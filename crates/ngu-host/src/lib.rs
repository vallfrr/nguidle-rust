//! Bridge between the generated Rust (typed state) and the engine (`clr::Value`).
//! Game objects live in Rust structs (`Gc<Cn>`); the engine sees them as `Value::Obj` "shells"
//! (one per instance, created on demand or by the scene loader) linked by a `__h` handle.
use clr::code::NativeFn;
use clr::program::Ty;
use clr::value::{Ext, Obj as VObj, Struct, Value};
use clr::Vm;
use ngu_rt::*;
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

pub use clr;

pub trait ToV {
    fn to_v(&self) -> Value;
}
pub trait FromV {
    fn from_v(v: &Value) -> Self;
}

/// Generated game class: native construction + import of the scene's serialized fields.
pub trait UserClass: Sized + 'static {
    const CID: u32;
    fn construct() -> Gc<Self>;
    fn import(g: &Gc<Self>, f: &[Value], all: bool);
    /// instance without constructor (deserialized object)
    fn blank() -> Gc<Self>;
    /// writes the instance's current state into its shell's fields (serialization by the engine)
    fn export(g: &Gc<Self>, f: &mut Vec<Value>);
    fn hget(&self) -> u32;
    fn hset(&mut self, h: u32);
}

pub struct Host {
    pub vm: *mut Vm,
    shells: Vec<Rc<VObj>>,
    twins: Vec<Box<dyn Any>>,
    exporters: Vec<fn(&dyn Any, &mut Vec<Value>)>,
    by_ptr: HashMap<usize, u32>,
    natives: HashMap<String, Option<NativeFn>>,
    pub native_calls: u64,
}
static mut HOST: Option<Host> = None;

#[allow(static_mut_refs)]
pub fn host() -> &'static mut Host {
    unsafe { HOST.get_or_insert_with(|| Host { vm: std::ptr::null_mut(), shells: vec![], twins: vec![], exporters: vec![], by_ptr: HashMap::new(), natives: HashMap::new(), native_calls: 0 }) }
}
pub fn set_vm(vm: *mut Vm) {
    host().vm = vm;
}
pub fn vm() -> &'static mut Vm {
    unsafe { &mut *host().vm }
}

/// Read a typed object's current fields without walking or mutating its graph.
pub fn live_fields(value: &Value) -> Option<Vec<Value>> {
    let Value::Obj(o) = value else { return None };
    let key = Rc::as_ptr(o) as usize;
    let &handle = host().by_ptr.get(&key)?;
    let index = handle as usize - 1;
    // Converting a field can register more objects, so the exporter must not
    // borrow the host's vectors while it runs.
    let twin = host().twins[index].as_ref() as *const dyn Any;
    let export = host().exporters[index];
    let mut fields = o.f.borrow().clone();
    export(unsafe { &*twin }, &mut fields);
    Some(fields)
}

fn exporter<T: UserClass>(b: &dyn Any, f: &mut Vec<Value>) {
    if let Some(g) = b.downcast_ref::<Gc<T>>() {
        T::export(g, f);
    }
}

fn register<T: UserClass>(shell: Rc<VObj>, twin: Box<dyn Any>) -> u32 {
    let h = host();
    h.shells.push(shell.clone());
    h.twins.push(twin);
    h.exporters.push(exporter::<T>);
    let id = h.shells.len() as u32;
    h.by_ptr.insert(Rc::as_ptr(&shell) as *const u8 as usize, id);
    id
}

impl<T: UserClass> ToV for Option<Gc<T>> {
    fn to_v(&self) -> Value {
        match self {
            None => Value::Null,
            Some(g) => {
                let h = g.get().hget();
                if h != 0 {
                    return Value::Obj(host().shells[h as usize - 1].clone());
                }
                let shell = vm().new_obj(T::CID);
                let h = register::<T>(shell.clone(), Box::new(g.clone()));
                g.get().hset(h);
                Value::Obj(shell)
            }
        }
    }
}
impl<T: UserClass> FromV for Option<Gc<T>> {
    fn from_v(v: &Value) -> Self {
        match v {
            Value::Obj(o) => {
                let key = Rc::as_ptr(o) as *const u8 as usize;
                if let Some(&h) = host().by_ptr.get(&key) {
                    return match host().twins[h as usize - 1].downcast_ref::<Gc<T>>() {
                        Some(g) => Some(g.clone()),
                        // twin type differs from the expected type (inheritance/interface not handled): never return null silently
                        None => panic!("invalid class conversion: shell of type {} read as {}", o.ty, T::CID),
                    };
                }
                // object from binary deserialization: no constructor, all serialized fields are taken over
                let blank = vm().eng.deser.remove(&key);
                let g = if blank { T::blank() } else { T::construct() };
                let h = register::<T>(o.clone(), Box::new(g.clone()));
                g.get().hset(h);
                let f = o.f.borrow().clone();
                T::import(&g, &f, blank);
                Some(g)
            }
            _ => None,
        }
    }
}

macro_rules! num {
    ($($t:ty => $to:expr, $from:expr);* $(;)?) => { $(
        impl ToV for $t { fn to_v(&self) -> Value { let f: fn($t) -> Value = $to; f(*self) } }
        impl FromV for $t { fn from_v(v: &Value) -> Self { let f: fn(&Value) -> $t = $from; f(&unb(v)) } }
    )* };
}
num! {
    i32 => |x| Value::I32(x), |v| v.as_i32();
    i64 => |x| Value::I64(x), |v| v.as_i64();
    u64 => |x| Value::I64(x as i64), |v| v.as_i64() as u64;
    u32 => |x| Value::I32(x as i32), |v| v.as_i32() as u32;
    i16 => |x| Value::I32(x as i32), |v| v.as_i32() as i16;
    u16 => |x| Value::I32(x as i32), |v| v.as_i32() as u16;
    i8 => |x| Value::I32(x as i32), |v| v.as_i32() as i8;
    u8 => |x| Value::I32(x as i32), |v| v.as_i32() as u8;
    f32 => |x| Value::F32(x), |v| v.as_f32();
    f64 => |x| Value::F64(x), |v| v.as_f64();
    bool => |x| Value::I32(x as i32), |v| v.as_bool();
}
impl ToV for () {
    fn to_v(&self) -> Value {
        Value::Null
    }
}
impl FromV for () {
    fn from_v(_: &Value) -> Self {}
}
impl ToV for Str {
    fn to_v(&self) -> Value {
        match self {
            Some(s) => Value::Str(s.clone()),
            None => Value::Null,
        }
    }
}
impl FromV for Str {
    fn from_v(v: &Value) -> Self {
        match v {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        }
    }
}
/// Opaque engine object: the `Value` is kept as is.
impl ToV for Obj {
    fn to_v(&self) -> Value {
        let o = match self {
            Some(o) => o,
            None => return Value::Null,
        };
        if let Some(v) = o.downcast_ref::<Value>() {
            return v.clone();
        }
        // value boxed by generated code: conversion of common types
        macro_rules! try_as {
            ($($t:ty),*) => { $( if let Some(x) = o.downcast_ref::<$t>() { return x.to_v(); } )* };
        }
        try_as!(i32, i64, u64, u32, i16, u16, i8, u8, f32, f64, bool, Str, Color, Vector2, Vector3, Vector4, Quaternion);
        eprintln!("[host] boxed object of a type not convertible to the engine (null value passed)");
        Value::Null
    }
}
impl FromV for Obj {
    fn from_v(v: &Value) -> Self {
        if v.is_null() {
            None
        } else {
            box_val(v.clone())
        }
    }
}
impl<T: ToV + Clone> ToV for Option<std::rc::Rc<std::cell::RefCell<Vec<T>>>> {
    fn to_v(&self) -> Value {
        match self {
            None => Value::Null,
            Some(r) => {
                let items: Vec<Value> = r.borrow().iter().map(|x| x.to_v()).collect();
                clr::bcl_core::new_array(vm(), items)
            }
        }
    }
}
impl<T: FromV> FromV for Option<std::rc::Rc<std::cell::RefCell<Vec<T>>>> {
    fn from_v(v: &Value) -> Self {
        if let Value::Obj(o) = v {
            if let Ext::Arr(a) | Ext::List(a) = &*o.x.borrow() {
                return Some(Rc::new(std::cell::RefCell::new(a.iter().map(T::from_v).collect())));
            }
        }
        None
    }
}
impl<K: ToV + PartialEq + Clone, V: ToV + Clone> ToV for Dict<K, V> {
    fn to_v(&self) -> Value {
        match self {
            None => Value::Null,
            Some(d) => {
                let id = vm().type_id("System.Collections.Generic.Dictionary`2");
                let mut dd = clr::value::Dict::default();
                let d = d.borrow();
                for (k, v) in d.keys.iter().zip(d.vals.iter()) {
                    dd.set(k.to_v(), v.to_v());
                }
                Value::Obj(VObj::new(id, 0, Ext::Dict(dd)))
            }
        }
    }
}
impl<K: FromV + PartialEq + Clone, V: FromV + Clone> FromV for Dict<K, V> {
    fn from_v(v: &Value) -> Self {
        if let Value::Obj(o) = v {
            if let Ext::Dict(d) = &*o.x.borrow() {
                let mut out = DictImpl::new();
                for (k, v) in d.iter() {
                    out.set(K::from_v(k), V::from_v(v));
                }
                return Some(Rc::new(std::cell::RefCell::new(out)));
            }
        }
        None
    }
}
macro_rules! vstruct {
    ($t:ident, $name:expr, $($f:ident),+) => {
        impl ToV for $t { fn to_v(&self) -> Value { let ty = vm().type_id($name); Value::Struct(Box::new(Struct { ty, f: vec![$(Value::F32(self.$f)),+] })) } }
        impl FromV for $t { fn from_v(v: &Value) -> Self {
            let mut r = $t::default();
            if let Value::Struct(s) = v { let mut i = 0; $( if let Some(x) = s.f.get(i) { r.$f = x.as_f32(); } i += 1; )+ let _ = i; }
            r } }
    };
}
vstruct!(Color, "UnityEngine.Color", r, g, b, a);
vstruct!(Vector2, "UnityEngine.Vector2", x, y);
vstruct!(Vector3, "UnityEngine.Vector3", x, y, z);
vstruct!(Vector4, "UnityEngine.Vector4", x, y, z, w);
vstruct!(Quaternion, "UnityEngine.Quaternion", x, y, z, w);

/// Call of an engine native function (`clr::bcl` table), key `Type::Method`.
pub fn native(key: &str, targs: Option<Vec<Ty>>, args: Vec<Value>) -> Value {
    let h = host();
    h.native_calls += 1;
    if trace() {
        let d: Vec<String> = args.iter().map(|v| vm().describe(v)).collect();
        eprintln!("[native] {key} {}", d.join(", "));
    }
    let f = *h.natives.entry(key.to_string()).or_insert_with(|| clr::bcl::lookup(key));
    let f = match f {
        Some(f) => f,
        None => {
            // Report unimplemented engine APIs once. A missing numerical API must not silently save a zero.
            static mut SEEN: Vec<String> = Vec::new();
            #[allow(static_mut_refs)]
            unsafe {
                if !SEEN.iter().any(|k| k == key) {
                    SEEN.push(key.to_string());
                    eprintln!("[host] missing native function: {key}");
                }
            }
            if matches!(key.split_once("::").map(|(ty, _)| ty),
                Some("System.Decimal" | "System.Math" | "System.MathF" | "System.Convert" | "UnityEngine.Mathf"
                    | "System.Int32" | "System.Int64" | "System.Single" | "System.Double")) {
                throw(vm().new_exc("System.NotImplementedException", key));
            }
            return Value::Null;
        }
    };
    let vm = vm();
    vm.targs = targs.map(Rc::new);
    let mut a = args;
    match f(vm, &mut a) {
        Ok(v) => v,
        Err(e) => throw(e),
    }
}

impl<K: ToV, V: ToV> ToV for (K, V) {
    fn to_v(&self) -> Value {
        let ty = vm().type_id("System.Collections.Generic.KeyValuePair`2");
        Value::Struct(Box::new(Struct { ty, f: vec![self.0.to_v(), self.1.to_v()] }))
    }
}
impl<K: FromV, V: FromV> FromV for (K, V) {
    fn from_v(v: &Value) -> Self {
        match v {
            Value::Struct(s) if s.f.len() >= 2 => (K::from_v(&s.f[0]), V::from_v(&s.f[1])),
            _ => (K::from_v(&Value::Null), V::from_v(&Value::Null)),
        }
    }
}

/// `newobj` of an external type: structs return the native constructor's value, classes
/// receive a pre-allocated object as first argument (like the interpreter).
pub fn native_new(tname: &str, is_struct: bool, args: Vec<Value>) -> Value {
    let key = format!("{}::.ctor", tname);
    if is_struct {
        return native(&key, None, args);
    }
    let id = vm().type_id(tname);
    let o = Value::Obj(vm().new_obj(id));
    let mut a = vec![o.clone()];
    a.extend(args);
    native(&key, None, a);
    o
}

/// "Unset" value of a shell: the field keeps what the generated constructor initialised.
pub fn is_default(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::I32(0) | Value::I64(0) => true,
        Value::F32(x) => *x == 0.0,
        Value::F64(x) => *x == 0.0,
        _ => false,
    }
}

/// `ldftn` + `newobj delegate`: engine delegate (target, method).
pub fn new_delegate(target: Value, mid: u32) -> Obj {
    let id = vm().type_id("System.Delegate");
    box_val(Value::Obj(VObj::new(id, 0, Ext::Delegate(vec![clr::value::Deleg { target, method: mid }]))))
}
pub fn invoke_delegate(d: &Obj, args: Vec<Value>) -> Value {
    match d.to_v() {
        Value::Obj(o) => match vm().call_delegate(&o, args) {
            Ok(v) => v,
            Err(e) => throw(e),
        },
        _ => panic!("NullReferenceException (null delegate)"),
    }
}
/// External static field (`Vector3.zero`...), `clr::bcl` table.
pub fn static_field(key: &str) -> Value {
    match clr::bcl::static_field(vm(), key) {
        Ok(v) => v,
        Err(_) => panic!("external static field not found: {key}"),
    }
}

pub fn native_field(key: &str, object: Value) -> Obj {
    match clr::unity_api::native_field(vm(), &object, key) {
        Ok(value) => FromV::from_v(&value),
        Err(error) => throw(error),
    }
}

/// `isinst`/`castclass` from an `Obj`: engine value (engine type test) or boxed typed value.
/// Returns `Option<T>`; for `T = Obj` / `Option<Gc<_>>`, `None` = .NET `null`.
pub fn obj_cast<T: FromV + Clone + 'static>(o: &Obj, cid: u32) -> Option<T> {
    if trace() {
        eprintln!("[cast?] cid={} {} present={}", cid, vm().types[cid as usize].name, o.is_some());
    }
    let r = o.as_ref()?;
    if let Some(v) = r.downcast_ref::<Value>() {
        if vm().is_inst(v, &clr::code::TyOp::Class(cid)) {
            return Some(T::from_v(v));
        }
        if trace() {
            let vn = match v { Value::Obj(o) => vm().types[o.ty as usize].name.clone(), _ => "?".into() };
            eprintln!("[cast] {} is not a {}", vn, vm().types[cid as usize].name);
        }
        return None;
    }
    r.downcast_ref::<T>().cloned()
}

// ---------------------------------------------------------------- .NET exceptions (Rust stack unwinding)
/// Payload of a .NET exception raised from generated code or a native call.
pub struct Exc(pub Value);
unsafe impl Send for Exc {}

static mut CATCH_DEPTH: u32 = 0;
static mut LAST_LOC: String = String::new();
static mut CUR_EXC: Vec<Value> = Vec::new();

/// Installs a silent panic hook for .NET exceptions (and panics intercepted by a `try`).
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(l) = info.location() {
            unsafe { LAST_LOC = format!("{}:{}", l.file(), l.line()) };
        }
        let silent = unsafe { CATCH_DEPTH > 0 } || info.payload().is::<Exc>();
        if !silent {
            prev(info);
        }
    }));
}

pub fn try_catch<R>(f: impl FnOnce() -> R) -> Result<R, Box<dyn Any + Send>> {
    unsafe { CATCH_DEPTH += 1 };
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    unsafe { CATCH_DEPTH -= 1 };
    r
}
pub fn throw(v: Value) -> ! {
    std::panic::resume_unwind(Box::new(Exc(v)))
}
#[allow(static_mut_refs)]
pub fn rethrow() -> ! {
    let v = unsafe { CUR_EXC.last().cloned() }.unwrap_or(Value::Null);
    throw(v)
}
#[allow(static_mut_refs)]
pub fn set_cur(v: Value) {
    unsafe { CUR_EXC.push(v) };
}
/// .NET exception object of a payload (Rust panics are translated to usual CLR exceptions).
pub fn to_exc(p: &Box<dyn Any + Send>) -> Value {
    if let Some(e) = p.downcast_ref::<Exc>() {
        return e.0.clone();
    }
    let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
    let name = if msg.contains("NullReferenceException") {
        "System.NullReferenceException"
    } else if msg.contains("KeyNotFoundException") {
        "System.Collections.Generic.KeyNotFoundException"
    } else if msg.contains("InvalidCastException") {
        "System.InvalidCastException"
    } else if msg.contains("index out of bounds") || msg.contains("out of range") {
        "System.IndexOutOfRangeException"
    } else if msg.contains("divide by zero") || msg.contains("remainder with a divisor of zero") {
        "System.DivideByZeroException"
    } else {
        "System.Exception"
    };
    let loc = unsafe { LAST_LOC.clone() };
    vm().new_exc(name, &format!("{} @ {}", msg, loc))
}
pub fn exc_is(v: &Value, cid: u32) -> bool {
    vm().is_inst(v, &clr::code::TyOp::Class(cid))
}
pub fn exc_obj(v: &Value) -> Obj {
    box_val(v.clone())
}

/// `typeof(X)` / `ldtoken`: engine `System.Type` object.
pub fn type_token(name: &str) -> Obj {
    let id = vm().type_id("System.Type");
    box_val(Value::Obj(VObj::new(id, 0, Ext::Native(Box::new(name.to_string())))))
}

/// `unbox.any`: typed value boxed by generated code or engine `Value` (lenient numeric conversion).
pub fn unbox<T: FromV + Clone + 'static>(o: &Obj) -> T {
    let r = o.as_ref().expect("NullReferenceException");
    if let Some(x) = r.downcast_ref::<T>() {
        return x.clone();
    }
    let v = if let Some(v) = r.downcast_ref::<Value>() {
        v.clone()
    } else if let Some(x) = r.downcast_ref::<i32>() {
        Value::I32(*x)
    } else if let Some(x) = r.downcast_ref::<i64>() {
        Value::I64(*x)
    } else if let Some(x) = r.downcast_ref::<f32>() {
        Value::F32(*x)
    } else if let Some(x) = r.downcast_ref::<f64>() {
        Value::F64(*x)
    } else if let Some(x) = r.downcast_ref::<bool>() {
        Value::I32(*x as i32)
    } else {
        panic!("InvalidCastException: unbox")
    };
    T::from_v(&v)
}

fn trace() -> bool {
    static mut T: Option<bool> = None;
    unsafe { *T.get_or_insert_with(|| std::env::var("NGU_TRACE").is_ok()) }
}

/// (the engine -> game call trace is done by `Vm::invoke`, shared by the oracle and native mode: NGU_TRACE_CALLS=1)
pub fn trace_invoke(_vm: *mut Vm, _mid: u32) {}

/// Engine boxed primitive (`Ext::Boxed`) -> bare value.
pub fn unb(v: &Value) -> Value {
    if let Value::Obj(o) = v {
        if let Ext::Boxed(b) = &*o.x.borrow() {
            return b.clone();
        }
    }
    v.clone()
}

/// Type id of a native class (external type ids are not stable across processes).
pub fn cid_of(name: &str) -> u32 {
    vm().type_id(name)
}

/// Generated method entry (NGU_TRACE_M=1).
#[inline]
pub fn tm(mid: u32) {
    if clr::vm::trace_m() {
        let vm = vm();
        let mi = &vm.prog.methods[mid as usize];
        eprintln!("[m] {}::{}", vm.types[mi.owner as usize].name, mi.name);
    }
}

/// .NET boxing into an `Obj`: always an engine `Value` (immediate conversion), hence readable by native calls.
pub fn boxv<T: ToV>(v: T) -> Obj {
    let x = v.to_v();
    if x.is_null() { None } else { Some(Rc::new(x)) }
}
/// `as T` / cast on an `Obj` (engine value) to a convertible type (`FromV`), without a fine type test:
/// a `Str` requires `Value::Str`, a game class / engine object requires `Value::Obj`.
pub fn obj_cast_v<T: FromV>(o: &Obj) -> Option<T> {
    let v = o.as_ref()?.downcast_ref::<Value>()?;
    Some(T::from_v(v))
}

impl ToV for Color32 {
    fn to_v(&self) -> Value {
        let ty = vm().type_id("UnityEngine.Color32");
        Value::Struct(Box::new(Struct { ty, f: vec![Value::I32(self.r as i32), Value::I32(self.g as i32), Value::I32(self.b as i32), Value::I32(self.a as i32)] }))
    }
}
impl FromV for Color32 {
    fn from_v(v: &Value) -> Self {
        let mut r = Color32::default();
        if let Value::Struct(s) = v {
            let g = |i: usize| s.f.get(i).map(|x| x.as_i32() as u8).unwrap_or(0);
            r = Color32 { r: g(0), g: g(1), b: g(2), a: g(3) };
        }
        r
    }
}

/// "Deep" conversion to the engine: the current state of every reachable game object is copied into its shell
/// (needed by native functions that read fields, like `BinaryFormatter.Serialize`).
pub fn deep<T: ToV>(v: &T) -> Value {
    let x = v.to_v();
    let mut seen = std::collections::HashSet::new();
    deep_sync(&x, &mut seen);
    x
}
fn deep_sync(v: &Value, seen: &mut std::collections::HashSet<usize>) {
    let o = match v {
        Value::Obj(o) => o.clone(),
        Value::Struct(s) => {
            for x in &s.f {
                deep_sync(x, seen);
            }
            return;
        }
        _ => return,
    };
    let key = Rc::as_ptr(&o) as *const u8 as usize;
    if !seen.insert(key) {
        return;
    }
    if let Some(&h) = host().by_ptr.get(&key) {
        let mut f = o.f.borrow().clone();
        let exp = host().exporters[h as usize - 1];
        exp(&*host().twins[h as usize - 1], &mut f);
        *o.f.borrow_mut() = f;
    }
    let children: Vec<Value> = {
        let f = o.f.borrow();
        let mut c: Vec<Value> = f.iter().cloned().collect();
        match &*o.x.borrow() {
            Ext::Arr(a) | Ext::List(a) => c.extend(a.iter().cloned()),
            Ext::Dict(d) => {
                for (k, v) in d.iter() {
                    c.push(k.clone());
                    c.push(v.clone());
                }
            }
            Ext::Boxed(b) => c.push(b.clone()),
            _ => {}
        }
        c
    };
    for c in &children {
        deep_sync(c, seen);
    }
}

// List<T> (a type distinct from T[]): lists are exposed to the engine as `System.Collections.Generic.List`1`.
impl<T: ToV> ToV for Option<Rc<std::cell::RefCell<GList<T>>>> {
    fn to_v(&self) -> Value {
        match self {
            None => Value::Null,
            Some(r) => {
                let items: Vec<Value> = r.borrow().iter().map(|x| x.to_v()).collect();
                clr::bcl_core::new_list(vm(), items)
            }
        }
    }
}
impl<T: FromV> FromV for Option<Rc<std::cell::RefCell<GList<T>>>> {
    fn from_v(v: &Value) -> Self {
        if let Value::Obj(o) = v {
            if let Ext::Arr(a) | Ext::List(a) = &*o.x.borrow() {
                return Some(Rc::new(std::cell::RefCell::new(GList(a.iter().map(T::from_v).collect()))));
            }
        }
        None
    }
}

/// Native instance call: a null receiver raises `NullReferenceException` (like `callvirt`).
pub fn native_i(key: &str, targs: Option<Vec<Ty>>, args: Vec<Value>) -> Value {
    if matches!(args.first(), Some(Value::Null)) {
        let e = vm().new_exc("System.NullReferenceException", &format!("Object reference not set to an instance of an object ({key})"));
        throw(e);
    }
    native(key, targs, args)
}
