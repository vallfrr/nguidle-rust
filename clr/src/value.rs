//! Value model of the CIL interpreter (dynamic typing).
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Result: `Err` carries the CLR exception object (Value::Obj).
pub type R<T> = Result<T, Value>;

#[derive(Clone)]
pub enum Value {
    Null,
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Str(Rc<str>),
    Obj(Rc<Obj>),
    /// Type valeur (copie profonde a chaque clone).
    Struct(Box<Struct>),
    /// Managed pointer (ref/out/ldloca/ldflda...).
    Ptr(Rc<Ptr>),
}

#[derive(Clone)]
pub struct Struct {
    pub ty: u32,
    pub f: Vec<Value>,
}

pub struct Obj {
    pub ty: u32,
    pub f: RefCell<Vec<Value>>,
    pub x: RefCell<Ext>,
}

pub enum Ext {
    None,
    /// Boxed user value type (struct/enum).
    Boxed(Value),
    Arr(Vec<Value>),
    List(Vec<Value>),
    Dict(Dict),
    SB(String),
    Delegate(Vec<Deleg>),
    /// Native engine data (components, textures...).
    Native(Box<dyn Any>),
}

#[derive(Clone)]
pub struct Deleg {
    pub target: Value,
    pub method: u32,
}

pub enum Ptr {
    /// Absolute slot in the VM stack (argument or local).
    Slot(usize),
    Field(Rc<Obj>, u32),
    /// Field of a pointed-to struct.
    SField(Rc<Ptr>, u32),
    Elem(Rc<Obj>, u32),
    Static(u32, u32),
}

#[derive(Hash, PartialEq, Eq, Clone)]
pub enum HKey {
    I(i64),
    S(Rc<str>),
    P(usize),
    F(u64),
    T(Vec<HKey>),
    N,
}

#[derive(Default)]
pub struct Dict {
    pub entries: Vec<Option<(Value, Value)>>,
    pub index: HashMap<HKey, usize>,
    pub live: usize,
}

impl Dict {
    pub fn get(&self, k: &Value) -> Option<&Value> {
        let i = *self.index.get(&hkey(k))?;
        self.entries[i].as_ref().map(|e| &e.1)
    }
    pub fn set(&mut self, k: Value, v: Value) {
        let h = hkey(&k);
        if let Some(&i) = self.index.get(&h) {
            self.entries[i] = Some((k, v));
        } else {
            self.index.insert(h, self.entries.len());
            self.entries.push(Some((k, v)));
            self.live += 1;
        }
    }
    pub fn remove(&mut self, k: &Value) -> bool {
        if let Some(i) = self.index.remove(&hkey(k)) {
            self.entries[i] = None;
            self.live -= 1;
            true
        } else {
            false
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
        self.live = 0;
    }
    pub fn iter(&self) -> impl Iterator<Item = &(Value, Value)> {
        self.entries.iter().flatten()
    }
}

pub fn hkey(v: &Value) -> HKey {
    match v {
        Value::Null => HKey::N,
        Value::I32(i) => HKey::I(*i as i64),
        Value::I64(i) => HKey::I(*i),
        Value::F32(f) => HKey::F((*f as f64).to_bits()),
        Value::F64(f) => HKey::F(f.to_bits()),
        Value::Str(s) => HKey::S(s.clone()),
        Value::Obj(o) => {
            if let Ext::Boxed(b) = &*o.x.borrow() {
                return hkey(b);
            }
            HKey::P(Rc::as_ptr(o) as *const u8 as usize)
        }
        Value::Struct(s) => HKey::T(s.f.iter().map(hkey).collect()),
        Value::Ptr(p) => HKey::P(Rc::as_ptr(p) as *const u8 as usize),
    }
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn as_i32(&self) -> i32 {
        match self {
            Value::I32(i) => *i,
            Value::I64(i) => *i as i32,
            Value::F32(f) => *f as i32,
            Value::F64(f) => *f as i32,
            Value::Null => 0,
            _ => 0,
        }
    }
    pub fn as_i64(&self) -> i64 {
        match self {
            Value::I32(i) => *i as i64,
            Value::I64(i) => *i,
            Value::F32(f) => *f as i64,
            Value::F64(f) => *f as i64,
            _ => 0,
        }
    }
    pub fn as_f64(&self) -> f64 {
        match self {
            Value::I32(i) => *i as f64,
            Value::I64(i) => *i as f64,
            Value::F32(f) => *f as f64,
            Value::F64(f) => *f,
            _ => 0.0,
        }
    }
    pub fn as_f32(&self) -> f32 {
        self.as_f64() as f32
    }
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Null => false,
            Value::I32(i) => *i != 0,
            Value::I64(i) => *i != 0,
            _ => true,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_obj(&self) -> Option<&Rc<Obj>> {
        match self {
            Value::Obj(o) => Some(o),
            _ => None,
        }
    }
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }
    pub fn bool(b: bool) -> Value {
        Value::I32(b as i32)
    }
}

impl Obj {
    pub fn new(ty: u32, nfields: usize, x: Ext) -> Rc<Obj> {
        Rc::new(Obj { ty, f: RefCell::new(vec![Value::Null; nfields]), x: RefCell::new(x) })
    }
}
