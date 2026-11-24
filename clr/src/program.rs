//! Loading of a .NET assembly (dotnetdll) -> flat tables (types, fields, methods).
use crate::code::Code;
use crate::value::*;
use dotnetdll::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Bool, Char, I8, U8, I16, U16, I32, U32, I64, U64, F32, F64, IntPtr, UIntPtr,
    Object, String,
    Array(Box<Ty>),
    /// User type (id in Vm.types).
    User(u32),
    /// External type (full name), `true` if value type.
    Ext(String, bool),
    Generic,
    Pointer,
}

pub struct TypeInfo {
    pub name: String,
    pub user: Option<TypeIndex>,
    pub base: Option<u32>,
    pub ifaces: Vec<u32>,
    pub is_value: bool,
    pub is_enum: bool,
    pub is_iface: bool,
    /// Instance field names (inheritance included), index = slot.
    pub fields: Vec<String>,
    pub field_tys: Vec<Ty>,
    pub field_elems: Vec<Option<Ty>>,
    pub field_skip: Vec<bool>,
    pub statics: HashMap<String, u32>,
    pub static_tys: Vec<Ty>,
    /// Methods defined by this type: name -> ids.
    pub methods: HashMap<String, Vec<u32>>,
    pub cctor: Option<u32>,
    pub cctor_state: u8, // 0 = not run, 1 = running/done
}

pub struct MethodInfo {
    pub name: String,
    pub owner: u32,
    pub mi: Option<MethodIndex>,
    pub is_static: bool,
    pub argc: usize,
    pub has_ret: bool,
    pub virt: bool,
    pub abstract_: bool,
    pub sig: String,
    pub code: std::cell::OnceCell<Rc<Code>>,
    pub ctor_or_static: bool,
    /// targeted optimisations (see Vm::guard_skip), 0 = none
    pub guard: u8,
}

pub struct Program {
    pub res: Resolution<'static>,
    pub tid: HashMap<TypeIndex, u32>,
    pub mid: HashMap<MethodIndex, u32>,
    pub fslot: HashMap<FieldIndex, (u32, u32, bool)>,
    pub methods: Vec<MethodInfo>,
    /// (declaration, implementation) of explicit MethodImpls.
    pub overrides: HashMap<u32, Vec<(u32, u32)>>, // type id -> [(decl, impl)]
}

pub fn typeref_name(res: &Resolution, r: TypeRefIndex) -> String {
    let t = &res[r];
    match &t.scope {
        ResolutionScope::Nested(p) => format!("{}/{}", typeref_name(res, *p), t.name),
        _ => match &t.namespace {
            Some(ns) if !ns.is_empty() => format!("{}.{}", ns, t.name),
            _ => t.name.to_string(),
        },
    }
}

pub fn typedef_name(res: &Resolution, i: TypeIndex) -> String {
    let t = &res[i];
    if let Some(e) = t.encloser {
        return format!("{}/{}", typedef_name(res, e), t.name);
    }
    match &t.namespace {
        Some(ns) if !ns.is_empty() => format!("{}.{}", ns, t.name),
        _ => t.name.to_string(),
    }
}

impl Program {
    pub fn load(path: &str) -> (Program, Vec<TypeInfo>) {
        Program::load_bytes(std::fs::read(path).expect("dll read"))
    }

    pub fn load_bytes(data: Vec<u8>) -> (Program, Vec<TypeInfo>) {
        let bytes: &'static [u8] = Box::leak(data.into_boxed_slice());
        let res = Resolution::parse(bytes, ReadOptions::default()).expect("dll parse");
        let mut p = Program {
            res,
            tid: HashMap::new(),
            mid: HashMap::new(),
            fslot: HashMap::new(),
            methods: vec![],
            overrides: HashMap::new(),
        };
        let types = p.build();
        (p, types)
    }

    pub fn ty_of_source(&self, s: &TypeSource<MethodType>) -> Ty {
        let u = match s {
            TypeSource::User(u) => u,
            TypeSource::Generic { base, .. } => base,
        };
        match u {
            UserType::Definition(i) => Ty::User(self.tid[i]),
            UserType::Reference(r) => Ty::Ext(typeref_name(&self.res, *r), false),
        }
    }

    pub fn ty_of(&self, t: &MethodType) -> Ty {
        match t {
            MethodType::Base(b) => match &**b {
                BaseType::Type { value_kind, source } => {
                    let mut ty = self.ty_of_source(source);
                    if let Ty::Ext(n, v) = &mut ty {
                        *v = matches!(value_kind, Some(ValueKind::ValueType)) || crate::bcl::is_native_struct(n);
                    }
                    ty
                }
                BaseType::Boolean => Ty::Bool,
                BaseType::Char => Ty::Char,
                BaseType::Int8 => Ty::I8,
                BaseType::UInt8 => Ty::U8,
                BaseType::Int16 => Ty::I16,
                BaseType::UInt16 => Ty::U16,
                BaseType::Int32 => Ty::I32,
                BaseType::UInt32 => Ty::U32,
                BaseType::Int64 => Ty::I64,
                BaseType::UInt64 => Ty::U64,
                BaseType::Float32 => Ty::F32,
                BaseType::Float64 => Ty::F64,
                BaseType::IntPtr => Ty::IntPtr,
                BaseType::UIntPtr => Ty::UIntPtr,
                BaseType::Object => Ty::Object,
                BaseType::String => Ty::String,
                BaseType::Vector(_, e) => Ty::Array(Box::new(self.ty_of(e))),
                BaseType::Array(e, _) => Ty::Array(Box::new(self.ty_of(e))),
                BaseType::ValuePointer(..) | BaseType::FunctionPointer(..) => Ty::Pointer,
            },
            _ => Ty::Generic,
        }
    }

    /// Element type of a 1-parameter generic field (List<T>) or array.
    pub fn elem_of_member(&self, t: &MemberType) -> Option<Ty> {
        if let MemberType::Base(b) = t {
            match &**b {
                BaseType::Type { source: TypeSource::Generic { parameters, .. }, .. } if !parameters.is_empty() => {
                    return Some(self.ty_of_member(&parameters[0]));
                }
                BaseType::Vector(_, e) => return Some(self.ty_of_member(e)),
                _ => {}
            }
        }
        None
    }

    pub fn ty_of_member(&self, t: &MemberType) -> Ty {
        match t {
            MemberType::Base(b) => {
                // reuses ty_of through manual conversion
                match &**b {
                    BaseType::Type { value_kind, source } => {
                        let src = match source {
                            TypeSource::User(u) => TypeSource::User(*u),
                            TypeSource::Generic { base, .. } => TypeSource::User(*base),
                        };
                        let mut ty = self.ty_of_source(&src);
                        if let Ty::Ext(n, v) = &mut ty {
                            *v = matches!(value_kind, Some(ValueKind::ValueType)) || crate::bcl::is_native_struct(n);
                        }
                        ty
                    }
                    BaseType::Boolean => Ty::Bool,
                    BaseType::Char => Ty::Char,
                    BaseType::Int8 => Ty::I8,
                    BaseType::UInt8 => Ty::U8,
                    BaseType::Int16 => Ty::I16,
                    BaseType::UInt16 => Ty::U16,
                    BaseType::Int32 => Ty::I32,
                    BaseType::UInt32 => Ty::U32,
                    BaseType::Int64 => Ty::I64,
                    BaseType::UInt64 => Ty::U64,
                    BaseType::Float32 => Ty::F32,
                    BaseType::Float64 => Ty::F64,
                    BaseType::IntPtr => Ty::IntPtr,
                    BaseType::UIntPtr => Ty::UIntPtr,
                    BaseType::Object => Ty::Object,
                    BaseType::String => Ty::String,
                    BaseType::Vector(_, e) => Ty::Array(Box::new(self.ty_of_member(e))),
                    BaseType::Array(e, _) => Ty::Array(Box::new(self.ty_of_member(e))),
                    _ => Ty::Pointer,
                }
            }
            _ => Ty::Generic,
        }
    }

    fn build(&mut self) -> Vec<TypeInfo> {
        // 1. type ids (= enumeration order)
        let order: Vec<TypeIndex> = self.res.enumerate_type_definitions().map(|(i, _)| i).collect();
        for (n, i) in order.iter().enumerate() {
            self.tid.insert(*i, n as u32);
        }
        // 2. skeletons
        let mut types: Vec<TypeInfo> = order
            .iter()
            .map(|&i| {
                let t = &self.res[i];
                TypeInfo {
                    name: typedef_name(&self.res, i),
                    user: Some(i),
                    base: None,
                    ifaces: vec![],
                    is_value: false,
                    is_enum: false,
                    is_iface: matches!(t.flags.kind, dotnetdll::resolved::types::Kind::Interface),
                    fields: vec![],
                    field_tys: vec![],
                    field_elems: vec![],
                    field_skip: vec![],
                    statics: HashMap::new(),
                    static_tys: vec![],
                    methods: HashMap::new(),
                    cctor: None,
                    cctor_state: 0,
                }
            })
            .collect();
        // 3. methods (including property/event accessors)
        for &ti in &order {
            let tid = self.tid[&ti];
            let mut list: Vec<MethodIndex> = vec![];
            let nm = self.res[ti].methods.len();
            for k in 0..nm {
                list.push(self.res.method_index(ti, k).unwrap());
            }
            for (pi, _) in self.res.enumerate_properties(ti) {
                if let Some(m) = self.res.property_getter_index(pi) { list.push(m); }
                if let Some(m) = self.res.property_setter_index(pi) { list.push(m); }
                let no = self.res[pi].other.len();
                for k in 0..no {
                    if let Some(m) = self.res.property_other_index(pi, k) { list.push(m); }
                }
            }
            for (ei, e) in self.res.enumerate_events(ti).map(|(i, e)| (i, e.raise_event.is_some())).collect::<Vec<_>>() {
                list.push(self.res.event_add_index(ei));
                list.push(self.res.event_remove_index(ei));
                if e {
                    if let Some(m) = self.res.event_raise_index(ei) { list.push(m); }
                }
            }
            for mi in list {
                let m = &self.res[mi];
                let sig = format!(
                    "{}({})",
                    m.signature.return_type.1.is_some() as u8,
                    m.signature.parameters.len()
                );
                let id = self.methods.len() as u32;
                self.methods.push(MethodInfo {
                    name: m.name.to_string(),
                    owner: tid,
                    mi: Some(mi),
                    is_static: m.is_static(),
                    argc: m.signature.parameters.len() + if m.signature.instance { 1 } else { 0 },
                    has_ret: m.signature.return_type.1.is_some(),
                    virt: m.virtual_member,
                    abstract_: m.abstract_member,
                    sig,
                    code: std::cell::OnceCell::new(),
                    ctor_or_static: m.is_static() || m.name == ".ctor",
                    guard: match m.name.as_ref() {
                        "progressPerTick" if typedef_name(&self.res, ti) == "WishesController" => 2,
                        "updateAllWishes" if typedef_name(&self.res, ti) == "WishesController" => 3,
                        "manageFight" if typedef_name(&self.res, ti) == "AdventureController" => 4,
                        "totalAdvAttack" if typedef_name(&self.res, ti) == "Character" => 5,
                        "totalAdvDefense" if typedef_name(&self.res, ti) == "Character" => 6,
                        name if name.starts_with("autokillTitan") && name.ends_with("Achieved")
                            && typedef_name(&self.res, ti) == "AdventureController" => 7,
                        "getOptimalScore" if typedef_name(&self.res, ti) == "CookingController" => 8,
                        "totalAdvHP" if typedef_name(&self.res, ti) == "Character" => 9,
                        "checkAchievements" if typedef_name(&self.res, ti) == "AllAchievementsController" => 10,
                        "achievementCondition" if typedef_name(&self.res, ti) == "AllAchievementsController" => 11,
                        "totalEnergyPower" if m.signature.parameters.is_empty() && typedef_name(&self.res, ti) == "Character" => 12,
                        "totalMagicPower" if m.signature.parameters.is_empty() && typedef_name(&self.res, ti) == "Character" => 13,
                        "totalCapEnergy" if m.signature.parameters.is_empty() && typedef_name(&self.res, ti) == "Character" => 14,
                        "totalCapMagic" if m.signature.parameters.is_empty() && typedef_name(&self.res, ti) == "Character" => 15,
                        "totalEnergyBar" if m.signature.parameters.is_empty() && typedef_name(&self.res, ti) == "Character" => 16,
                        "totalMagicBar" if m.signature.parameters.is_empty() && typedef_name(&self.res, ti) == "Character" => 17,
                        "effectiveBossID" if m.signature.parameters.is_empty()
                            && typedef_name(&self.res, ti) == "Character" => 18,
                        "Update" if m.signature.parameters.is_empty()
                            && typedef_name(&self.res, ti) == "LoadoutController" => 19,
                        "equipSpecBonus" if m.signature.parameters.len() == 2
                            && typedef_name(&self.res, ti) == "InventoryController" => 20,
                        "specBonus" if m.signature.parameters.len() == 1
                            && typedef_name(&self.res, ti) == "InventoryController" => 21,
                        _ => 0,
                    },
                });
                self.mid.insert(mi, id);
                let name = m.name.to_string();
                if name == ".cctor" {
                    types[tid as usize].cctor = Some(id);
                }
                types[tid as usize].methods.entry(name).or_default().push(id);
            }
        }
        // 4. inheritance + fields (in dependency order)
        let mut done = vec![false; types.len()];
        for n in 0..types.len() {
            self.layout(n as u32, &mut types, &mut done, &order);
        }
        // 5. explicit MethodImpls
        for &ti in &order {
            let tid = self.tid[&ti];
            for ov in &self.res[ti].overrides {
                let (decl, imp) = (&ov.declaration, &ov.implementation);
                if let (UserMethod::Definition(d), UserMethod::Definition(i)) = (decl, imp) {
                    self.overrides.entry(tid).or_default().push((self.mid[d], self.mid[i]));
                } else if let (UserMethod::Reference(_), UserMethod::Definition(i)) = (decl, imp) {
                    // external declaration (BCL interface): key by name
                    let _ = i;
                }
            }
        }
        types
    }

    /// Resolves (or creates) the id of the named native type. Also called by the VM at run time.
    pub fn base_name_of(&self, ti: TypeIndex) -> Option<TKey> {
        let t = &self.res[ti];
        let e = t.extends.as_ref()?;
        let u = match e {
            TypeSource::User(u) => u,
            TypeSource::Generic { base, .. } => base,
        };
        Some(match u {
            UserType::Definition(i) => TKey::User(self.tid[i]),
            UserType::Reference(r) => TKey::Ext(typeref_name(&self.res, *r)),
        })
    }

    fn layout(&self, n: u32, types: &mut Vec<TypeInfo>, done: &mut Vec<bool>, order: &[TypeIndex]) {
        if done[n as usize] {
            return;
        }
        done[n as usize] = true;
        let ti = order[n as usize];
        let mut fields: Vec<String> = vec![];
        let mut ftys: Vec<Ty> = vec![];
        let mut felems: Vec<Option<Ty>> = vec![];
        let mut fskip: Vec<bool> = vec![];
        let mut is_value = false;
        let mut is_enum = false;
        let mut base_id = None;
        if let Some(bk) = self.base_name_of(ti) {
            match bk {
                TKey::User(b) => {
                    self.layout(b, types, done, order);
                    fields = types[b as usize].fields.clone();
                    ftys = types[b as usize].field_tys.clone();
                    felems = types[b as usize].field_elems.clone();
                    fskip = types[b as usize].field_skip.clone();
                    is_value = types[b as usize].is_value;
                    base_id = Some(b);
                }
                TKey::Ext(name) => {
                    if name == "System.ValueType" { is_value = true; }
                    if name == "System.Enum" { is_value = true; is_enum = true; }
                    let (pf, pt) = crate::bcl::native_layout(&name);
                    for (f, t) in pf.iter().zip(pt) { fields.push(f.to_string()); ftys.push(t); }
                    if let Some(par) = crate::bcl::native_parent(&name) {
                        let (pf2, pt2) = crate::bcl::native_layout(par);
                        let mut f2: Vec<String> = pf2.iter().map(|s| s.to_string()).collect();
                        f2.extend(fields.drain(..));
                        let mut t2: Vec<Ty> = pt2;
                        t2.extend(ftys.drain(..));
                        fields = f2; ftys = t2;
                    }
                    felems.resize(fields.len(), None);
                }
            }
        }
        let mut statics = HashMap::new();
        let mut static_tys = vec![];
        for (fi, f) in self.res.enumerate_fields(ti) {
            if f.static_member {
                statics.insert(f.name.to_string(), static_tys.len() as u32);
                static_tys.push(self.ty_of_member(&f.return_type));
            } else {
                // enum fields (value__) are handled as an integer
                fields.push(f.name.to_string());
                ftys.push(self.ty_of_member(&f.return_type));
                felems.push(self.elem_of_member(&f.return_type));
                fskip.resize(fields.len() - 1, false);
                fskip.push(f.not_serialized);
            }
            let _ = fi;
        }
        let t = &mut types[n as usize];
        t.base = base_id;
        t.is_value = is_value;
        t.is_enum = is_enum;
        t.fields = fields;
        felems.resize(ftys.len(), None);
        fskip.resize(ftys.len(), false);
        t.field_skip = fskip;
        t.field_elems = felems;
        t.field_tys = ftys;
        t.statics = statics;
        t.static_tys = static_tys;
        // interfaces
        let mut ifs = vec![];
        for (_, s) in &self.res[ti].implements {
            let u = match s {
                TypeSource::User(u) => u,
                TypeSource::Generic { base, .. } => base,
            };
            if let UserType::Definition(i) = u {
                ifs.push(self.tid[i]);
            }
        }
        t.ifaces = ifs;
        // field -> (type, slot) table
        // (filled separately below)
        let mut inst = 0u32;
        let mut stat = 0u32;
        let base_n = base_id.map(|b| 0 + 0 * b as usize).unwrap_or(0);
        let _ = base_n;
        let own_start = t.fields.len() as u32 - self.res[ti].fields.iter().filter(|f| !f.static_member).count() as u32;
        inst += own_start;
        for (fi, f) in self.res.enumerate_fields(ti) {
            // mutable access impossible here (borrow of self.fslot) -> see fix_fslot
            let _ = (fi, f, &mut inst, &mut stat);
        }
    }

    /// Builds the field -> slot table (after layout, with mutable access to self).
    pub fn fix_fslot(&mut self, types: &[TypeInfo]) {
        let order: Vec<TypeIndex> = self.res.enumerate_type_definitions().map(|(i, _)| i).collect();
        for (n, &ti) in order.iter().enumerate() {
            let t = &types[n];
            let nown = self.res[ti].fields.iter().filter(|f| !f.static_member).count();
            let mut inst = (t.fields.len() - nown) as u32;
            let mut stat = 0u32;
            let items: Vec<(FieldIndex, bool)> = self
                .res
                .enumerate_fields(ti)
                .map(|(fi, f)| (fi, f.static_member))
                .collect();
            for (fi, is_static) in items {
                if is_static {
                    self.fslot.insert(fi, (n as u32, stat, true));
                    stat += 1;
                } else {
                    self.fslot.insert(fi, (n as u32, inst, false));
                    inst += 1;
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum TKey {
    User(u32),
    Ext(String),
}
