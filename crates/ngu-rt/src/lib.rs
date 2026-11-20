//! Open runtime of the NGU Idle port: minimal .NET types + BCL functions called by generated code.
pub mod ext;
pub mod fmt;
pub mod unity;
pub use unity::*;
pub use ext::ListEnum;

use std::cell::RefCell;
use std::rc::Rc;

/// generic .NET object (nullable).
pub type Obj = Option<Rc<dyn std::any::Any>>;
/// .NET string (nullable).
pub type Str = Option<Rc<str>>;
/// .NET array (nullable).
pub type Arr<T> = Option<Rc<RefCell<Vec<T>>>>;
/// List<T> (nullable).
pub type List<T> = Option<Rc<RefCell<GList<T>>>>;
/// Content of a `List<T>`: distinct from `Vec` so the bridge tells list from array (different serialization formats).
pub struct GList<T>(pub Vec<T>);
impl<T> std::ops::Deref for GList<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}
impl<T> std::ops::DerefMut for GList<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}
impl<T> Default for GList<T> {
    fn default() -> Self {
        GList(Vec::new())
    }
}
/// Dictionary<K,V> (nullable), insertion order kept like .NET without removals.
pub type Dict<K, V> = Option<Rc<RefCell<DictImpl<K, V>>>>;

/// Game class reference: shared, mutable, single-threaded (like the .NET GC).
pub struct Gc<T>(pub Rc<GcCell<T>>);
pub struct GcCell<T>(std::cell::UnsafeCell<T>);
impl<T> Gc<T> {
    pub fn new(v: T) -> Gc<T> {
        Gc(Rc::new(GcCell(std::cell::UnsafeCell::new(v))))
    }
    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    pub fn get(&self) -> &mut T {
        unsafe { &mut *self.0 .0.get() }
    }
}
impl<T> Clone for Gc<T> {
    fn clone(&self) -> Self {
        Gc(self.0.clone())
    }
}

pub fn str_of(x: &str) -> Str {
    Some(Rc::from(x))
}
pub fn str_ref(s: &Str) -> &str {
    s.as_deref().unwrap_or("")
}

pub fn new_arr<T: Clone + Default>(n: i32) -> Arr<T> {
    Some(Rc::new(RefCell::new(vec![T::default(); n.max(0) as usize])))
}

pub struct DictImpl<K, V> {
    pub keys: Vec<K>,
    pub vals: Vec<V>,
}
impl<K: PartialEq + Clone, V: Clone> DictImpl<K, V> {
    pub fn new() -> Self {
        DictImpl { keys: vec![], vals: vec![] }
    }
    pub fn set(&mut self, k: K, v: V) {
        match self.keys.iter().position(|x| *x == k) {
            Some(i) => self.vals[i] = v,
            None => {
                self.keys.push(k);
                self.vals.push(v);
            }
        }
    }
    pub fn get(&self, k: &K) -> Option<V> {
        self.keys.iter().position(|x| x == k).map(|i| self.vals[i].clone())
    }
}
impl<K: PartialEq + Clone, V: Clone> Default for DictImpl<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Reference equality (`ceq` on references).
pub trait RefId {
    fn rid(&self) -> usize;
}
impl<T: ?Sized> RefId for Option<Rc<T>> {
    fn rid(&self) -> usize {
        self.as_ref().map(|r| Rc::as_ptr(r) as *const u8 as usize).unwrap_or(0)
    }
}
impl<T> RefId for Option<Gc<T>> {
    fn rid(&self) -> usize {
        self.as_ref().map(|r| Rc::as_ptr(&r.0) as *const u8 as usize).unwrap_or(0)
    }
}
pub fn ref_eq<A: RefId, B: RefId>(a: &A, b: &B) -> bool {
    a.rid() == b.rid()
}

/// .NET boxing: every value becomes an `Obj` (downcast for formatting / casts).
pub fn box_val<T: 'static>(v: T) -> Obj {
    Some(Rc::new(v))
}

impl<T> PartialEq for Gc<T> {
    fn eq(&self, o: &Self) -> bool {
        Rc::ptr_eq(&self.0, &o.0)
    }
}

/// `as`/cast from an `Obj` (value boxed by `box_val`).
pub fn cast_obj<T: Clone + 'static>(o: &Obj) -> Option<T> {
    o.as_ref().and_then(|r| r.downcast_ref::<T>().cloned())
}

/// Little-endian decoding of array initializers (`RuntimeHelpers.InitializeArray`).
pub trait FromLe: Sized {
    const N: usize;
    fn from_le(b: &[u8]) -> Self;
}
macro_rules! from_le {
    ($($t:ty),*) => { $( impl FromLe for $t {
        const N: usize = std::mem::size_of::<$t>();
        fn from_le(b: &[u8]) -> Self { let mut a = [0u8; std::mem::size_of::<$t>()]; a.copy_from_slice(&b[..Self::N]); <$t>::from_le_bytes(a) }
    } )* };
}
from_le!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);
impl FromLe for bool {
    const N: usize = 1;
    fn from_le(b: &[u8]) -> Self { b[0] != 0 }
}
pub fn init_array<T: FromLe>(a: &Arr<T>, data: &[u8]) {
    let a = a.as_ref().expect("NullReferenceException");
    let mut v = a.borrow_mut();
    for (i, c) in data.chunks_exact(T::N).enumerate() {
        if i < v.len() {
            v[i] = T::from_le(c);
        }
    }
}

/// `conv.i4` from a float, like x86-64 (cvttsd2si): NaN / out of range -> `i32::MIN`.
pub fn f2i32(x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 { i32::MIN } else { x as i32 }
}
/// `conv.i8` from a float: NaN / out of range -> `i64::MIN`.
pub fn f2i64(x: f64) -> i64 {
    if x.is_nan() || x >= 9223372036854775808.0 || x < -9223372036854775808.0 { i64::MIN } else { x as i64 }
}

/// Exit of a `try` region (generated code): jump to an outer block or return from the method.
pub enum Flow<R> {
    Goto(usize),
    Ret(R),
}
/// Action pending while a `finally` block runs.
pub enum Pend<R> {
    None,
    Goto(usize),
    Ret(R),
    Throw(Box<dyn std::any::Any + Send>),
}
