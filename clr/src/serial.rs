//! BinaryFormatter serialization of the game's object graph (`PlayerData`, `SaveData`) in the real .NET wire format
//! (see `nrbf.rs`), so that saves are interchangeable with the original game.
//! The writer reproduces the structure of the .NET writer: referenced objects are queued and written after the object
//! that uses them (`MemberReference`), value types (enums, structs) are written inline with negative ids, strings inline.
use crate::nrbf::*;
use crate::program::Ty;
use crate::value::*;
use crate::Vm;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

const ASM_GAME: &str = "Assembly-CSharp, Version=0.0.0.0, Culture=neutral, PublicKeyToken=null";
const ASM_UNITY: &str = "UnityEngine.CoreModule, Version=0.0.0.0, Culture=neutral, PublicKeyToken=null";
const ASM_CORLIB: &str = "mscorlib, Version=4.0.0.0, Culture=neutral, PublicKeyToken=b77a5c561934e089";
const LIST: &str = "System.Collections.Generic.List`1";

fn fail<T>(vm: &mut Vm, msg: &str) -> R<T> {
    crate::elog!("[serial] {msg}");
    vm.throw("System.Runtime.Serialization.SerializationException", msg)
}

// ------------------------------------------------------------------ reading

struct Mat<'a> {
    s: &'a Stream,
    cache: HashMap<i32, Value>,
    depth: usize,
}

fn pv_value(p: &Pv) -> Value {
    match p {
        Pv::Bool(b) => Value::bool(*b),
        Pv::U8(x) => Value::I32(*x as i32),
        Pv::Char(c) => Value::I32(c.chars().next().map(|c| c as i32).unwrap_or(0)),
        Pv::Decimal(_) => Value::I32(0),
        Pv::F64(x) => Value::F64(*x),
        Pv::I16(x) => Value::I32(*x as i32),
        Pv::I32(x) => Value::I32(*x),
        Pv::I64(x) => Value::I64(*x),
        Pv::I8(x) => Value::I32(*x as i32),
        Pv::F32(x) => Value::F32(*x),
        Pv::Ts(x) => Value::I64(*x),
        Pv::Dt(x) => Value::I64(*x as i64),
        Pv::U16(x) => Value::I32(*x as i32),
        Pv::U32(x) => Value::I32(*x as i32),
        Pv::U64(x) => Value::I64(*x as i64),
    }
}

fn mat(vm: &mut Vm, m: &mut Mat, n: &N) -> R<Value> {
    m.depth += 1;
    if m.depth > 400 {
        return fail(vm, "graph too deep");
    }
    let r = mat_inner(vm, m, n);
    m.depth -= 1;
    r
}

fn mat_inner(vm: &mut Vm, m: &mut Mat, n: &N) -> R<Value> {
    match n {
        N::Null => Ok(Value::Null),
        N::P(p) => Ok(pv_value(p)),
        N::Str(_, s) => Ok(Value::str(s)),
        N::Ref(id) => {
            if let Some(v) = m.cache.get(id) {
                return Ok(v.clone());
            }
            match m.s.index.get(id).cloned() {
                Some(t) => mat(vm, m, &t),
                None => fail(vm, &format!("unresolved object reference {id}")),
            }
        }
        N::PArr(a) => {
            if let Some(v) = m.cache.get(&a.id) {
                return Ok(v.clone());
            }
            let items = a.vals.iter().map(pv_value).collect();
            let v = crate::bcl_core::new_array(vm, items);
            m.cache.insert(a.id, v.clone());
            Ok(v)
        }
        N::Arr(a) => {
            if let Some(v) = m.cache.get(&a.id) {
                return Ok(v.clone());
            }
            let mut items = Vec::with_capacity(a.items.len());
            for it in &a.items {
                items.push(mat(vm, m, it)?);
            }
            let v = crate::bcl_core::new_array(vm, items);
            m.cache.insert(a.id, v.clone());
            Ok(v)
        }
        N::Obj(o) => {
            if let Some(v) = m.cache.get(&o.id) {
                return Ok(v.clone());
            }
            let name = o.meta.name.replace('+', "/");
            if name.starts_with(LIST) {
                let slot = |f: &str| o.meta.names.iter().position(|x| x == f);
                let (Some(si), Some(sz)) = (slot("_items"), slot("_size")) else { return fail(vm, "malformed List") };
                let size = match &o.vals[sz] {
                    N::P(Pv::I32(x)) => (*x).max(0) as usize,
                    _ => 0,
                };
                let items = match mat(vm, m, &o.vals[si])? {
                    Value::Obj(a) => match &*a.x.borrow() {
                        Ext::Arr(v) => v.iter().take(size).cloned().collect::<Vec<_>>(),
                        _ => vec![],
                    },
                    _ => vec![],
                };
                let v = crate::bcl_core::new_list(vm, items);
                m.cache.insert(o.id, v.clone());
                return Ok(v);
            }
            let Some(&tid) = vm.type_ids.get(&name) else { return fail(vm, &format!("unknown type in save: {}", o.meta.name)) };
            if vm.types[tid as usize].is_enum {
                let v = match o.meta.names.iter().position(|x| x == "value__").map(|i| &o.vals[i]) {
                    Some(N::P(p)) => pv_value(p),
                    _ => Value::I32(0),
                };
                m.cache.insert(o.id, v.clone());
                return Ok(v);
            }
            if tid >= vm.n_user {
                // native value type (e.g. UnityEngine.Random.State): built by value, no sharing
                let mut sv = vm.default_struct(tid);
                for (i, fname) in o.meta.names.iter().enumerate() {
                    let v = mat(vm, m, &o.vals[i])?;
                    if let Value::Struct(s) = &mut sv {
                        if let Some(slot) = vm.types[tid as usize].fields.iter().position(|f| f == fname) {
                            let fty = vm.types[tid as usize].field_tys[slot].clone();
                            s.f[slot] = crate::bcl_sys::coerce(v, &fty);
                        }
                    }
                }
                return Ok(sv);
            }
            let obj = vm.new_obj(tid); // without constructor, like BinaryFormatter
            vm.eng.deser.insert(Rc::as_ptr(&obj) as *const u8 as usize);
            let v = Value::Obj(obj.clone());
            m.cache.insert(o.id, v.clone());
            for (i, fname) in o.meta.names.iter().enumerate() {
                let val = mat(vm, m, &o.vals[i])?;
                if let Some(slot) = vm.types[tid as usize].fields.iter().position(|f| f == fname) {
                    let fty = vm.types[tid as usize].field_tys[slot].clone();
                    obj.f.borrow_mut()[slot] = crate::bcl_sys::coerce(val, &fty);
                }
            }
            Ok(v)
        }
    }
}

/// `BinaryFormatter.Deserialize`: stream bytes -> engine value.
pub fn deserialize(vm: &mut Vm, bytes: &[u8]) -> R<Value> {
    let s = match parse(bytes) {
        Ok(s) => s,
        Err(e) => return fail(vm, &format!("invalid save data: {e}")),
    };
    let mut m = Mat { s: &s, cache: HashMap::new(), depth: 0 };
    mat(vm, &mut m, &s.root.clone())
}

// ------------------------------------------------------------------ writing

/// Declared type of a member / array element, derived from the field types of the game's classes.
#[derive(Clone, Debug)]
enum Decl {
    Prim(u8),
    Str,
    Obj,
    /// user class or enum (type id)
    Class(u32),
    /// native value type (engine type name with `/` nesting)
    Native(String),
    List(Box<Decl>),
    Arr(Box<Decl>),
}

fn prim_of(ty: &Ty) -> Option<u8> {
    Some(match ty {
        Ty::Bool => 1,
        Ty::U8 => 2,
        Ty::Char => 3,
        Ty::F64 => 6,
        Ty::I16 => 7,
        Ty::I32 => 8,
        Ty::I64 => 9,
        Ty::I8 => 10,
        Ty::F32 => 11,
        Ty::U16 => 14,
        Ty::U32 => 15,
        Ty::U64 => 16,
        _ => return None,
    })
}

fn decl_of(vm: &mut Vm, ty: &Ty, elem: Option<&Ty>) -> R<Decl> {
    if let Some(p) = prim_of(ty) {
        return Ok(Decl::Prim(p));
    }
    Ok(match ty {
        Ty::String => Decl::Str,
        Ty::Object => Decl::Obj,
        Ty::User(id) => Decl::Class(*id),
        Ty::Array(e) => Decl::Arr(Box::new(decl_of(vm, e, None)?)),
        Ty::Ext(n, _) if n.starts_with(LIST) => {
            let Some(e) = elem else { return fail(vm, "List field without element type") };
            Decl::List(Box::new(decl_of(vm, e, None)?))
        }
        Ty::Ext(n, true) if vm.type_ids.contains_key(n) => Decl::Native(n.clone()),
        t => return fail(vm, &format!("cannot serialize a field of type {t:?}")),
    })
}

fn prim_clr(p: u8) -> &'static str {
    match p {
        1 => "System.Boolean",
        2 => "System.Byte",
        3 => "System.Char",
        6 => "System.Double",
        7 => "System.Int16",
        8 => "System.Int32",
        9 => "System.Int64",
        10 => "System.SByte",
        11 => "System.Single",
        14 => "System.UInt16",
        15 => "System.UInt32",
        16 => "System.UInt64",
        _ => "System.Object",
    }
}

struct W {
    out: Vec<u8>,
    next: i32,
    libs: HashMap<String, i32>,
    libs_written: HashSet<i32>,
    meta: HashMap<String, i32>,
    ids: HashMap<usize, i32>,
    strs: HashMap<usize, i32>,
    queue: VecDeque<(Value, Decl, i32)>,
    /// every queued object stays alive until the end: ids are keyed by address, which must not be reused
    keep: Vec<Value>,
}

impl W {
    fn id(&mut self) -> i32 {
        let i = self.next;
        self.next += 1;
        i
    }
    fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn lps(&mut self, s: &str) {
        let mut n = s.len();
        loop {
            let b = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                self.out.push(b);
                break;
            }
            self.out.push(b | 0x80);
        }
        self.out.extend_from_slice(s.as_bytes());
    }
    fn lib(&mut self, asm: &str) -> i32 {
        if let Some(i) = self.libs.get(asm) {
            return *i;
        }
        let i = self.id();
        self.libs.insert(asm.to_string(), i);
        i
    }
    fn lib_record(&mut self, lib: i32) {
        if self.libs_written.insert(lib) {
            let name = self.libs.iter().find(|(_, v)| **v == lib).map(|(k, _)| k.clone()).unwrap_or_default();
            self.out.push(12);
            self.i32(lib);
            self.lps(&name);
        }
    }
}

/// Type name as written in the stream: engine nesting `/` becomes `+`.
fn stream_name(n: &str) -> String {
    n.replace('/', "+")
}

fn asm_of_native(n: &str) -> &'static str {
    if n.starts_with("UnityEngine.") { ASM_UNITY } else { ASM_CORLIB }
}

fn type_label(vm: &Vm, d: &Decl) -> String {
    match d {
        Decl::Prim(p) => format!("{}, {}", prim_clr(*p), ASM_CORLIB),
        Decl::Str => format!("System.String, {ASM_CORLIB}"),
        Decl::Obj => format!("System.Object, {ASM_CORLIB}"),
        Decl::Class(id) => format!("{}, {}", stream_name(&vm.types[*id as usize].name), ASM_GAME),
        Decl::Native(n) => format!("{}, {}", stream_name(n), asm_of_native(n)),
        Decl::List(e) => format!("{LIST}[[{}]], {}", type_label(vm, e), ASM_CORLIB),
        Decl::Arr(e) => format!("{}[]", type_label(vm, e).split(", ").next().unwrap_or("")),
    }
}

/// BinaryTypeEnum + additional info of a member / array element of declared type `d`.
fn bt_add(vm: &Vm, w: &mut W, d: &Decl) -> (u8, Add) {
    match d {
        Decl::Prim(p) => (0, Add::Prim(*p)),
        Decl::Str => (1, Add::None),
        Decl::Obj => (2, Add::None),
        Decl::Class(id) => {
            let l = w.lib(ASM_GAME);
            (4, Add::Class(stream_name(&vm.types[*id as usize].name), l))
        }
        Decl::Native(n) => {
            let l = w.lib(asm_of_native(n));
            (4, Add::Class(stream_name(n), l))
        }
        Decl::List(e) => (3, Add::Sys(format!("{LIST}[[{}]]", type_label(vm, e)))),
        Decl::Arr(e) => match &**e {
            Decl::Prim(p) => (7, Add::Prim(*p)),
            Decl::Str => (6, Add::None),
            Decl::Obj => (5, Add::None),
            Decl::Class(id) => {
                let l = w.lib(ASM_GAME);
                (4, Add::Class(format!("{}[]", stream_name(&vm.types[*id as usize].name)), l))
            }
            other => {
                let n = type_label(vm, other);
                let n = n.split(", ").next().unwrap_or("").to_string();
                let l = w.lib(ASM_CORLIB);
                (4, Add::Class(format!("{n}[]"), l))
            }
        },
    }
}

fn write_prim(vm: &mut Vm, w: &mut W, p: u8, v: &Value) -> R<()> {
    let v = match v {
        Value::Ptr(pp) => vm.load_ptr(pp),
        v => v.clone(),
    };
    match p {
        1 => w.out.push((v.as_i32() != 0) as u8),
        2 | 10 => w.out.push(v.as_i32() as u8),
        3 => {
            let c = char::from_u32(v.as_i32() as u32).unwrap_or('\0');
            let mut b = [0u8; 4];
            w.out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
        }
        6 => w.out.extend_from_slice(&v.as_f64().to_le_bytes()),
        7 | 14 => w.out.extend_from_slice(&(v.as_i32() as i16).to_le_bytes()),
        8 | 15 => w.out.extend_from_slice(&v.as_i32().to_le_bytes()),
        9 | 16 => w.out.extend_from_slice(&v.as_i64().to_le_bytes()),
        11 => w.out.extend_from_slice(&v.as_f32().to_le_bytes()),
        _ => return fail(vm, "unsupported primitive in save"),
    }
    Ok(())
}

fn write_null_run(w: &mut W, n: usize) {
    match n {
        0 => {}
        1..=3 => (0..n).for_each(|_| w.out.push(10)),
        4..=255 => {
            w.out.push(13);
            w.out.push(n as u8);
        }
        _ => {
            w.out.push(14);
            w.i32(n as i32);
        }
    }
}

fn ptr_key(v: &Value) -> Option<usize> {
    match v {
        Value::Obj(o) => Some(Rc::as_ptr(o) as *const u8 as usize),
        Value::Str(s) => Some(s.as_ptr() as usize),
        _ => None,
    }
}

/// Writes the value of a member / element of declared type `d` (records for reference types are queued).
fn write_slot(vm: &mut Vm, w: &mut W, d: &Decl, v: &Value) -> R<()> {
    let v = match v {
        Value::Ptr(p) => vm.load_ptr(p),
        v => v.clone(),
    };
    match d {
        Decl::Prim(p) => write_prim(vm, w, *p, &v),
        Decl::Str => write_string(vm, w, &v),
        Decl::Obj => match &v {
            Value::Null => {
                w.out.push(10);
                Ok(())
            }
            Value::Str(_) => write_string(vm, w, &v),
            Value::Obj(o) if matches!(&*o.x.borrow(), Ext::Boxed(_)) => fail(vm, "boxed values in saves are not supported"),
            _ => fail(vm, "object-typed members are not supported in saves"),
        },
        Decl::Class(id) if vm.types[*id as usize].is_enum => {
            let tid = *id;
            let name = stream_name(&vm.types[tid as usize].name);
            let oid = w.id();
            write_class(vm, w, &name, ASM_GAME, -oid, vec![("value__".to_string(), Decl::Prim(8), Value::I32(v.as_i32()))])
        }
        Decl::Native(n) => {
            // a struct field that was never assigned holds no value yet: serialize the default value
            let v = match &v {
                Value::Struct(_) => v.clone(),
                Value::Null => match vm.type_ids.get(n).copied() {
                    Some(t) => vm.default_struct(t),
                    None => return fail(vm, "unknown native struct"),
                },
                other => {
                    let d = vm.describe(other);
                    return fail(vm, &format!("native struct expected, found {d}"));
                }
            };
            let Value::Struct(s) = &v else { return fail(vm, "native struct expected") };
            let tid = s.ty;
            let names = vm.types[tid as usize].fields.clone();
            let tys = vm.types[tid as usize].field_tys.clone();
            let mut members = vec![];
            for (i, f) in s.f.iter().enumerate() {
                let Some(p) = tys.get(i).and_then(prim_of) else { return fail(vm, "native struct with non-primitive field") };
                members.push((names[i].clone(), Decl::Prim(p), f.clone()));
            }
            let oid = w.id();
            let name = stream_name(n);
            write_class(vm, w, &name, asm_of_native(n), -oid, members)
        }
        Decl::Class(_) | Decl::List(_) | Decl::Arr(_) => match ptr_key(&v) {
            None => {
                w.out.push(10);
                Ok(())
            }
            Some(k) => {
                let id = match w.ids.get(&k) {
                    Some(i) => *i,
                    None => {
                        let i = w.id();
                        w.ids.insert(k, i);
                        w.keep.push(v.clone());
                        w.queue.push_back((v.clone(), d.clone(), i));
                        i
                    }
                };
                w.out.push(9);
                w.i32(id);
                Ok(())
            }
        },
    }
}

fn write_string(vm: &mut Vm, w: &mut W, v: &Value) -> R<()> {
    let s = match v {
        Value::Null => {
            w.out.push(10);
            return Ok(());
        }
        Value::Str(s) => s.to_string(),
        other => crate::bcl_core::to_string(vm, other)?,
    };
    let key = ptr_key(v);
    w.keep.push(v.clone());
    if let Some(i) = key.and_then(|k| w.strs.get(&k)).copied() {
        w.out.push(9);
        w.i32(i);
        return Ok(());
    }
    let id = w.id();
    if let Some(k) = key {
        w.strs.insert(k, id);
    }
    w.out.push(6);
    w.i32(id);
    w.lps(&s);
    Ok(())
}

/// Class record (full definition the first time, `ClassWithId` afterwards) followed by the member values.
fn write_class(vm: &mut Vm, w: &mut W, name: &str, asm: &str, id: i32, members: Vec<(String, Decl, Value)>) -> R<()> {
    let sys = asm == ASM_CORLIB;
    if let Some(mid) = w.meta.get(name).copied() {
        w.out.push(1);
        w.i32(id);
        w.i32(mid);
    } else {
        let lib = if sys { None } else { Some(w.lib(asm)) };
        let infos: Vec<(u8, Add)> = members.iter().map(|(_, d, _)| bt_add(vm, w, d)).collect();
        // every library mentioned by the class (its own, then those of its member types) is announced first
        let mut mentioned: Vec<i32> = lib.into_iter().collect();
        for (_, a) in &infos {
            if let Add::Class(_, l) = a {
                mentioned.push(*l);
            }
        }
        for l in mentioned {
            w.lib_record(l);
        }
        w.out.push(if sys { 4 } else { 5 });
        w.i32(id);
        w.lps(name);
        w.i32(members.len() as i32);
        for (n, _, _) in &members {
            w.lps(n);
        }
        for (b, _) in &infos {
            w.out.push(*b);
        }
        for (_, a) in &infos {
            match a {
                Add::Prim(p) => w.out.push(*p),
                Add::Sys(n) => w.lps(n),
                Add::Class(n, l) => {
                    w.lps(n);
                    w.i32(*l);
                }
                Add::None => {}
            }
        }
        if let Some(l) = lib {
            w.i32(l);
        }
        w.meta.insert(name.to_string(), id);
    }
    for (_, d, v) in &members {
        write_slot(vm, w, d, v)?;
    }
    Ok(())
}

fn write_queued(vm: &mut Vm, w: &mut W, v: &Value, d: &Decl, id: i32) -> R<()> {
    match d {
        Decl::Class(tid) => {
            let Value::Obj(o) = v else { return fail(vm, "object expected") };
            // runtime type wins over the declared type
            let rt = o.ty;
            let _ = tid;
            if rt >= vm.n_user || vm.types[rt as usize].user.is_none() {
                return fail(vm, &format!("type not serializable: {}", vm.types[rt as usize].name));
            }
            let name = stream_name(&vm.types[rt as usize].name);
            let names = vm.types[rt as usize].fields.clone();
            let tys = vm.types[rt as usize].field_tys.clone();
            let elems = vm.types[rt as usize].field_elems.clone();
            let skip = vm.types[rt as usize].field_skip.clone();
            let vals: Vec<Value> = o.f.borrow().clone();
            let mut members = vec![];
            for i in 0..names.len() {
                if skip.get(i).copied().unwrap_or(false) {
                    continue;
                }
                let d = decl_of(vm, &tys[i], elems.get(i).and_then(|e| e.as_ref()))?;
                members.push((names[i].clone(), d, vals[i].clone()));
            }
            write_class(vm, w, &name, ASM_GAME, id, members)
        }
        Decl::List(e) => {
            let Value::Obj(o) = v else { return fail(vm, "list expected") };
            let items = match &*o.x.borrow() {
                Ext::List(a) => a.clone(),
                _ => return fail(vm, "list expected"),
            };
            let name = format!("{LIST}[[{}]]", type_label(vm, e));
            let arr = crate::bcl_core::new_array(vm, items.clone());
            let members = vec![
                ("_items".to_string(), Decl::Arr(e.clone()), arr),
                ("_size".to_string(), Decl::Prim(8), Value::I32(items.len() as i32)),
                ("_version".to_string(), Decl::Prim(8), Value::I32(items.len() as i32)),
            ];
            write_class(vm, w, &name, ASM_CORLIB, id, members)
        }
        Decl::Arr(e) => {
            let Value::Obj(o) = v else { return fail(vm, "array expected") };
            let items = match &*o.x.borrow() {
                Ext::Arr(a) => a.clone(),
                _ => return fail(vm, "array expected"),
            };
            match &**e {
                Decl::Prim(p) => {
                    w.out.push(15);
                    w.i32(id);
                    w.i32(items.len() as i32);
                    w.out.push(*p);
                    for it in &items {
                        write_prim(vm, w, *p, it)?;
                    }
                    Ok(())
                }
                other => {
                    let (bt, add) = bt_add(vm, w, other);
                    if let Add::Class(_, l) = &add {
                        w.lib_record(*l);
                    }
                    match (bt, &add) {
                        (1, _) | (2, _) => {
                            w.out.push(if bt == 1 { 17 } else { 16 });
                            w.i32(id);
                            w.i32(items.len() as i32);
                        }
                        _ => {
                            w.out.push(7);
                            w.i32(id);
                            w.out.push(0);
                            w.i32(1);
                            w.i32(items.len() as i32);
                            w.out.push(bt);
                            match &add {
                                Add::Class(n, l) => {
                                    w.lps(n);
                                    w.i32(*l);
                                }
                                Add::Sys(n) => w.lps(n),
                                _ => {}
                            }
                        }
                    }
                    let mut run = 0;
                    for it in &items {
                        if matches!(it, Value::Null) {
                            run += 1;
                            continue;
                        }
                        write_null_run(w, run);
                        run = 0;
                        write_slot(vm, w, other, it)?;
                    }
                    write_null_run(w, run);
                    Ok(())
                }
            }
        }
        _ => fail(vm, "unexpected queued declaration"),
    }
}

/// `BinaryFormatter.Serialize`: engine value (a `PlayerData` / `SaveData` instance) -> stream bytes.
pub fn serialize(vm: &mut Vm, root: &Value) -> R<Vec<u8>> {
    let Value::Obj(o) = root else { return fail(vm, "cannot serialize this value") };
    let mut w = W { out: vec![], next: 1, libs: HashMap::new(), libs_written: HashSet::new(), meta: HashMap::new(), ids: HashMap::new(), strs: HashMap::new(), queue: VecDeque::new(), keep: vec![] };
    // header: root id 1, header id -1, version 1.0
    w.out.push(0);
    w.i32(1);
    w.i32(-1);
    w.i32(1);
    w.i32(0);
    let rid = w.id();
    w.ids.insert(Rc::as_ptr(o) as *const u8 as usize, rid);
    let d = Decl::Class(o.ty);
    write_queued(vm, &mut w, root, &d, rid)?;
    while let Some((v, d, id)) = w.queue.pop_front() {
        write_queued(vm, &mut w, &v, &d, id)?;
    }
    w.out.push(11);
    Ok(w.out)
}
