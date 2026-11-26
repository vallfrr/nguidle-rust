//! Builds the serialized layout of script classes from the game's assemblies,
//! following Unity's field serialization rules.

use super::typetree::Node;
use dotnetdll::prelude::*;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

type Result<T> = std::result::Result<T, String>;
type Asm = Rc<Resolution<'static>>;

const ALIGN: u64 = 0x4000;

/// Classes whose fields Unity serializes itself: no fields to add.
const ENGINE_ROOTS: [&str; 6] = [
    "UnityEngine.Object",
    "UnityEngine.Component",
    "UnityEngine.Behaviour",
    "UnityEngine.MonoBehaviour",
    "UnityEngine.ScriptableObject",
    "System.Object",
];

/// A class definition inside one of the loaded assemblies.
#[derive(Clone)]
struct Def {
    res: Asm,
    index: TypeIndex,
}

enum Base {
    None,
    Def(Def),
    Engine(String),
}

/// Layout builder over the assemblies in a `Managed` folder.
pub struct Layouts {
    managed: PathBuf,
    loaded: RefCell<HashMap<String, Option<Asm>>>,
    /// Engine types with fixed layouts, by full name: a node whose name is
    /// replaced by the field name.
    engine: HashMap<String, Node>,
    /// Engine base classes that bring serialized fields along.
    engine_bases: HashMap<String, Vec<Node>>,
}

fn node(ty: &str, name: &str, flags: u64, children: Vec<Node>) -> Node {
    Node {
        ty: ty.into(),
        name: name.into(),
        flags,
        children,
    }
}

fn pptr(target: &str, name: &str, flags: u64) -> Node {
    node(
        &format!("PPtr<{target}>"),
        name,
        flags,
        vec![
            node("int", "m_FileID", 0, vec![]),
            node("SInt64", "m_PathID", 0, vec![]),
        ],
    )
}

fn string(name: &str) -> Node {
    let array = node(
        "Array",
        "Array",
        ALIGN,
        vec![
            node("int", "size", 0, vec![]),
            node("char", "data", 0, vec![]),
        ],
    );
    node("string", name, 0, vec![array])
}

fn full_name(res: &Resolution, index: TypeIndex) -> String {
    let def = &res[index];
    if let Some(outer) = def.encloser {
        return format!("{}/{}", full_name(res, outer), def.name);
    }
    match &def.namespace {
        Some(ns) if !ns.is_empty() => format!("{ns}.{}", def.name),
        _ => def.name.to_string(),
    }
}

fn ref_name(res: &Resolution, index: TypeRefIndex) -> String {
    let r = &res[index];
    if let ResolutionScope::Nested(parent) = &r.scope {
        return format!("{}/{}", ref_name(res, *parent), r.name);
    }
    match &r.namespace {
        Some(ns) if !ns.is_empty() => format!("{ns}.{}", r.name),
        _ => r.name.to_string(),
    }
}

/// Name of the assembly that holds a referenced type.
fn ref_assembly(res: &Resolution, index: TypeRefIndex) -> Option<String> {
    match &res[index].scope {
        ResolutionScope::Assembly(a) => Some(res[*a].name.to_string()),
        ResolutionScope::Nested(parent) => ref_assembly(res, *parent),
        _ => None,
    }
}

impl Layouts {
    pub fn new(managed: PathBuf) -> Self {
        let json =
            serde_json::from_str(include_str!("engine_types.json")).expect("engine_types.json");
        let (engine, engine_bases) = engine_tables(&json);
        Layouts {
            managed,
            loaded: RefCell::new(HashMap::new()),
            engine,
            engine_bases,
        }
    }

    fn assembly(&self, name: &str) -> Option<Asm> {
        let file = if name.ends_with(".dll") {
            name.to_owned()
        } else {
            format!("{name}.dll")
        };
        if let Some(found) = self.loaded.borrow().get(&file) {
            return found.clone();
        }
        let loaded = std::fs::read(self.managed.join(&file))
            .ok()
            .and_then(|bytes| {
                let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
                Resolution::parse(bytes, ReadOptions::default())
                    .ok()
                    .map(Rc::new)
            });
        self.loaded.borrow_mut().insert(file, loaded.clone());
        loaded
    }

    fn find(&self, assembly: &str, name: &str) -> Option<Def> {
        let res = self.assembly(assembly)?;
        let index = res
            .enumerate_type_definitions()
            .find(|(i, _)| full_name(&res, *i) == name)?
            .0;
        Some(Def { res, index })
    }

    /// Layout of a script class, `Assembly.dll` plus the class's full name.
    pub fn script(&self, assembly: &str, class: &str) -> Result<Node> {
        let def = self
            .find(assembly, class)
            .ok_or_else(|| format!("{assembly}: no class {class}"))?;
        let mut children = vec![
            pptr("GameObject", "m_GameObject", ALIGN),
            node("UInt8", "m_Enabled", 0, vec![]),
            pptr("MonoScript", "m_Script", ALIGN),
            { node("string", "m_Name", ALIGN, vec![]) },
        ];
        children.extend(self.fields(&def, 0)?);
        let short = class.rsplit('.').next().unwrap_or(class);
        Ok(node(short, "Base", 0, children))
    }

    fn base(&self, def: &Def) -> Base {
        let res = &def.res;
        let Some(extends) = &res[def.index].extends else {
            return Base::None;
        };
        let source = match extends {
            TypeSource::User(user) => user,
            TypeSource::Generic { base, .. } => base,
        };
        match source {
            UserType::Definition(i) => Base::Def(Def {
                res: res.clone(),
                index: *i,
            }),
            UserType::Reference(r) => {
                let name = ref_name(res, *r);
                let found = ref_assembly(res, *r)
                    .filter(|a| resolvable(a))
                    .and_then(|asm| self.find(&asm, &name));
                match found {
                    Some(def) => Base::Def(def),
                    None => Base::Engine(name),
                }
            }
        }
    }

    fn is_engine_object(&self, def: &Def) -> bool {
        match self.base(def) {
            Base::Def(parent) => self.is_engine_object(&parent),
            Base::Engine(name) => ENGINE_ROOTS[..5].contains(&name.as_str()),
            Base::None => false,
        }
    }

    fn is_enum(&self, def: &Def) -> bool {
        matches!(self.base(def), Base::Engine(name) if name == "System.Enum")
    }

    fn attribute_named(res: &Resolution, attributes: &[Attribute], wanted: &str) -> bool {
        attributes.iter().any(|a| match &a.constructor {
            UserMethod::Reference(r) => match &res[*r].parent {
                MethodReferenceParent::Type(MethodType::Base(base)) => match &**base {
                    BaseType::Type {
                        source: TypeSource::User(UserType::Reference(t)),
                        ..
                    } => ref_name(res, *t) == wanted,
                    _ => false,
                },
                _ => false,
            },
            UserMethod::Definition(_) => false,
        })
    }

    /// Serialized fields of a class, base class first.
    fn fields(&self, def: &Def, depth: usize) -> Result<Vec<Node>> {
        if depth > 7 {
            return Ok(vec![]);
        }
        let mut out = match self.base(def) {
            Base::Def(parent) => self.fields(&parent, depth)?,
            Base::Engine(name) if ENGINE_ROOTS.contains(&name.as_str()) => vec![],
            Base::Engine(name) if name == "System.ValueType" => vec![],
            Base::Engine(name) => match self.engine_bases.get(&name) {
                Some(fields) => fields.clone(),
                None => return Err(format!("Unknown engine base class {name}")),
            },
            Base::None => vec![],
        };
        for (_, field) in def.res.enumerate_fields(def.index) {
            let public = matches!(
                field.accessibility,
                dotnetdll::resolved::members::Accessibility::Access(
                    dotnetdll::resolved::Accessibility::Public
                )
            );
            let marked =
                Self::attribute_named(&def.res, &field.attributes, "UnityEngine.SerializeField");
            if field.static_member
                || field.literal
                || field.init_only
                || field.not_serialized
                || !(public || marked)
            {
                continue;
            }
            if let Some(node) = self.field(&def.res, &field.return_type, &field.name, depth)? {
                out.push(node);
            }
        }
        Ok(out)
    }

    fn field(&self, res: &Asm, ty: &MemberType, name: &str, depth: usize) -> Result<Option<Node>> {
        let MemberType::Base(base) = ty else {
            return Ok(None);
        };
        let primitive =
            |t: &str, align: bool| Ok(Some(node(t, name, if align { ALIGN } else { 0 }, vec![])));
        match &**base {
            BaseType::Boolean => primitive("UInt8", true),
            BaseType::Int8 => primitive("SInt8", true),
            BaseType::UInt8 => primitive("UInt8", true),
            BaseType::Int16 => primitive("SInt16", true),
            BaseType::UInt16 => primitive("UInt16", true),
            BaseType::Char => primitive("UInt16", true),
            BaseType::Int32 => primitive("int", false),
            BaseType::UInt32 => primitive("unsigned int", false),
            BaseType::Int64 => primitive("SInt64", false),
            BaseType::UInt64 => primitive("UInt64", false),
            BaseType::Float32 => primitive("float", false),
            BaseType::Float64 => primitive("double", false),
            BaseType::String => Ok(Some(string(name))),
            BaseType::Vector(_, element) => {
                if is_list(element) {
                    return Ok(None); // Unity does not serialize nested lists
                }
                let Some(mut data) = self.field(res, element, "data", depth)? else {
                    return Ok(None);
                };
                if data.children.is_empty() {
                    data.flags = 0; // elements of an array are never aligned one by one
                }
                // Arrays of structures carry the structure's name, the rest are vectors.
                let plain =
                    data.children.is_empty() || data.ty.starts_with("PPtr<") || data.ty == "string";
                let ty = if plain {
                    "vector".to_owned()
                } else {
                    data.ty.clone()
                };
                let array = node(
                    "Array",
                    "Array",
                    ALIGN,
                    vec![node("int", "size", 0, vec![]), data],
                );
                Ok(Some(node(&ty, name, 0, vec![array])))
            }
            BaseType::Type { source, .. } => self.class_field(res, source, name, depth),
            _ => Ok(None),
        }
    }

    fn class_field(
        &self,
        res: &Asm,
        source: &TypeSource<MemberType>,
        name: &str,
        depth: usize,
    ) -> Result<Option<Node>> {
        let (user, arguments) = match source {
            TypeSource::User(user) => (user, &[][..]),
            TypeSource::Generic { base, parameters } => (base, &parameters[..]),
        };
        let def = match user {
            UserType::Definition(i) => Def {
                res: res.clone(),
                index: *i,
            },
            UserType::Reference(r) => {
                let type_name = ref_name(res, *r);
                if type_name == "System.Collections.Generic.List`1" {
                    let [element] = arguments else {
                        return Ok(None);
                    };
                    if is_list(element) {
                        return Ok(None);
                    }
                    return self.field(
                        res,
                        &MemberType::Base(Box::new(BaseType::Vector(vec![], element.clone()))),
                        name,
                        depth,
                    );
                }
                if let Some(table) = self.engine.get(&type_name) {
                    let mut copy = table.clone();
                    copy.name = name.into();
                    return Ok(Some(copy));
                }
                let target = ref_assembly(res, *r);
                match target
                    .as_deref()
                    .filter(|a| resolvable(a))
                    .and_then(|asm| self.find(asm, &type_name))
                {
                    Some(def) => def,
                    None if type_name.starts_with("UnityEngine.") => {
                        let enumeration = target
                            .as_deref()
                            .and_then(|asm| self.find(asm, &type_name))
                            .is_some_and(|def| self.is_enum(&def));
                        if enumeration {
                            return Ok(Some(node("int", name, 0, vec![])));
                        }
                        let short = type_name.rsplit('.').next().unwrap();
                        return Ok(Some(pptr(&format!("${short}"), name, 0)));
                    }
                    None => return Ok(None),
                }
            }
        };
        if !arguments.is_empty() {
            return Ok(None);
        }
        if let Some(table) = self.engine.get(&full_name(&def.res, def.index)) {
            let mut copy = table.clone();
            copy.name = name.into();
            return Ok(Some(copy));
        }
        let own_name = res_short(&def);
        if self.is_enum(&def) {
            return Ok(Some(node("int", name, 0, vec![])));
        }
        if self.is_engine_object(&def) {
            return Ok(Some(pptr(&format!("${own_name}"), name, 0)));
        }
        if def.res[def.index].flags.serializable {
            let children = self.fields(&def, depth + 1)?;
            return Ok(Some(node(&own_name, name, 0, children)));
        }
        Ok(None)
    }
}

/// Engine and framework assemblies are described by the engine tables, not
/// read: only the game's own code and the UI package are.
fn resolvable(assembly: &str) -> bool {
    assembly == "UnityEngine.UI"
        || !(assembly.starts_with("UnityEngine")
            || assembly.starts_with("System")
            || assembly == "mscorlib"
            || assembly == "netstandard")
}

fn is_list(element: &MemberType) -> bool {
    match element {
        MemberType::Base(base) => match &**base {
            BaseType::Vector(..) => true,
            BaseType::Type {
                source: TypeSource::Generic { parameters, .. },
                ..
            } => !parameters.is_empty(),
            _ => false,
        },
        _ => false,
    }
}

fn res_short(def: &Def) -> String {
    def.res[def.index].name.to_string()
}

/// Loads engine type tables from JSON: `{ "types": {name: rows}, "bases": {name: [rows]} }`
/// where rows are the flat `(level, type, name, flags)` lists.
pub fn engine_tables(json: &Value) -> (HashMap<String, Node>, HashMap<String, Vec<Node>>) {
    fn rows(value: &Value) -> Vec<(usize, String, String, u64)> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r[0].as_u64().unwrap() as usize,
                    r[1].as_str().unwrap().into(),
                    r[2].as_str().unwrap().into(),
                    r[3].as_u64().unwrap(),
                )
            })
            .collect()
    }
    let types = json["types"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), Node::from_flat(&rows(v))))
        .collect();
    let bases = json["bases"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| {
            let all = rows(v);
            let mut fields = Vec::new();
            let mut at = 0;
            while at < all.len() {
                let end = (at + 1..all.len())
                    .find(|i| all[*i].0 <= all[at].0)
                    .unwrap_or(all.len());
                fields.push(Node::from_flat(&all[at..end]));
                at = end;
            }
            (k.clone(), fields)
        })
        .collect();
    (types, bases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_tables_describe_value_types_and_event_bases() {
        let layouts = Layouts::new(PathBuf::new());
        let color = &layouts.engine["UnityEngine.Color"];
        assert_eq!(
            color
                .children
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["r", "g", "b", "a"]
        );
        assert!(!layouts.engine_bases["UnityEngine.Events.UnityEvent"].is_empty());
        assert!(layouts.script("Missing.dll", "Nothing").is_err());
    }
}
