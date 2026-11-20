//! BCL/Unity functions called by generated code. Name = Type__Method__ParamTypes (see ngu-ilgen).
#![allow(non_snake_case)]
use crate::*;
pub use crate::unity::{UnityEngine_Color___ctor_3, UnityEngine_Color___ctor_4, UnityEngine_Vector2___ctor_2, UnityEngine_Vector3___ctor_2, UnityEngine_Vector3___ctor_3, UnityEngine_Color32___ctor_4};

pub fn System_String__Concat__Str_Str(a: Str, b: Str) -> Str {
    let mut s = String::from(str_ref(&a));
    s.push_str(str_ref(&b));
    str_of(&s)
}
pub fn System_Double__ToString__Ref_f64(v: f64) -> Str {
    str_of(&crate::fmt::fmt_float(v, 15))
}
pub fn System_Double__ToString__Ref_f64_Str(v: f64, f: Str) -> Str {
    str_of(&crate::fmt::format_number(v, false, str_ref(&f)))
}
pub fn System_Int32__ToString__Ref_i32(v: i32) -> Str {
    str_of(&v.to_string())
}
pub fn System_Int64__ToString__Ref_i64(v: i64) -> Str {
    str_of(&v.to_string())
}
pub fn System_Math__Floor__f64(v: f64) -> f64 {
    v.floor()
}
pub fn System_Math__Pow__f64_f64(a: f64, b: f64) -> f64 {
    a.powf(b)
}
pub fn System_Math__Log__f64_f64(a: f64, b: f64) -> f64 {
    a.ln() / b.ln()
}
pub fn System_Math__Log10__f64(a: f64) -> f64 {
    a.log10()
}
pub fn System_Math__Abs__f64(a: f64) -> f64 {
    a.abs()
}
pub fn System_Double__IsPositiveInfinity__f64(a: f64) -> bool {
    a == f64::INFINITY
}
pub fn System_Double__IsNegativeInfinity__f64(a: f64) -> bool {
    a == f64::NEG_INFINITY
}
pub fn System_Double__IsNaN__f64(a: f64) -> bool {
    a.is_nan()
}

pub fn System_String__Concat__Str_Str_f64(a: Str, b: Str, c: f64) -> Str {
    // unused: the C# compiler emits Concat(object[]); kept for direct overloads
    str_of(&format!("{}{}{}", str_ref(&a), str_ref(&b), crate::fmt::fmt_float(c, 15)))
}
pub fn System_Single__ToString__Ref_f32_Str(v: f32, f: Str) -> Str {
    str_of(&crate::fmt::format_number(v as f64, false, str_ref(&f)))
}
pub fn System_Int32__ToString__Ref_i32_Str(v: i32, f: Str) -> Str {
    str_of(&crate::fmt::format_number(v as f64, true, str_ref(&f)))
}
pub fn UnityEngine_Mathf__Floor__f32(a: f32) -> f32 { a.floor() }
pub fn UnityEngine_Mathf__FloorToInt__f32(a: f32) -> i32 { a.floor() as i32 }
pub fn System_Collections_Generic_Dictionary_2___ctor<K: PartialEq + Clone, V: Clone>() -> Dict<K, V> {
    Some(Rc::new(RefCell::new(DictImpl::new())))
}
pub fn System_Collections_Generic_Dictionary_2__Add<K: PartialEq + Clone, V: Clone>(d: Dict<K, V>, k: K, v: V) {
    d.expect("NullReferenceException").borrow_mut().set(k, v);
}
#[track_caller]
pub fn System_Collections_Generic_Dictionary_2__get_Item<K: PartialEq + Clone, V: Clone>(d: Dict<K, V>, k: K) -> V {
    d.expect("NullReferenceException").borrow().get(&k).expect("KeyNotFoundException")
}
pub fn System_Collections_Generic_List_1___ctor<T>() -> List<T> {
    Some(Rc::new(RefCell::new(GList(Vec::new()))))
}

// ---- List<T> ----
#[track_caller]
fn nre<T>(x: Option<T>) -> T { x.expect("NullReferenceException") }
pub fn System_Collections_Generic_List_1___ctor_1<T>(_cap: i32) -> List<T> { Some(Rc::new(RefCell::new(GList(Vec::new())))) }
pub fn System_Collections_Generic_List_1__Add<T>(l: List<T>, v: T) { nre(l).borrow_mut().push(v) }
pub fn System_Collections_Generic_List_1__Clear<T>(l: List<T>) { nre(l).borrow_mut().clear() }
pub fn System_Collections_Generic_List_1__get_Count<T>(l: List<T>) -> i32 { nre(l).borrow().len() as i32 }
#[track_caller]
pub fn System_Collections_Generic_List_1__get_Item<T: Clone>(l: List<T>, i: i32) -> T { nre(l).borrow()[i as usize].clone() }
#[track_caller]
pub fn System_Collections_Generic_List_1__set_Item<T>(l: List<T>, i: i32, v: T) { nre(l).borrow_mut()[i as usize] = v }
pub fn System_Collections_Generic_List_1__Contains<T: PartialEq>(l: List<T>, v: T) -> bool { nre(l).borrow().contains(&v) }
pub fn System_Collections_Generic_List_1__IndexOf<T: PartialEq>(l: List<T>, v: T) -> i32 { nre(l).borrow().iter().position(|x| *x == v).map(|i| i as i32).unwrap_or(-1) }
pub fn System_Collections_Generic_List_1__Remove<T: PartialEq>(l: List<T>, v: T) -> bool {
    let l = nre(l);
    let mut b = l.borrow_mut();
    match b.iter().position(|x| *x == v) { Some(i) => { b.remove(i); true } None => false }
}
#[track_caller]
pub fn System_Collections_Generic_List_1__RemoveAt<T>(l: List<T>, i: i32) { nre(l).borrow_mut().remove(i as usize); }
#[track_caller]
pub fn System_Collections_Generic_List_1__Insert<T>(l: List<T>, i: i32, v: T) { nre(l).borrow_mut().insert(i as usize, v) }
pub fn System_Collections_Generic_List_1__ToArray<T: Clone>(l: List<T>) -> Arr<T> { Some(Rc::new(RefCell::new(nre(l).borrow().0.clone()))) }

// ---- Dictionary<K,V> ----
pub fn System_Collections_Generic_Dictionary_2__Clear<K, V>(d: Dict<K, V>) { let d = nre(d); let mut d = d.borrow_mut(); d.keys.clear(); d.vals.clear(); }
pub fn System_Collections_Generic_Dictionary_2__get_Count<K, V>(d: Dict<K, V>) -> i32 { nre(d).borrow().keys.len() as i32 }
pub fn System_Collections_Generic_Dictionary_2__ContainsKey<K: PartialEq + Clone, V: Clone>(d: Dict<K, V>, k: K) -> bool { nre(d).borrow().get(&k).is_some() }
pub fn System_Collections_Generic_Dictionary_2__TryGetValue<K: PartialEq + Clone, V: Clone>(d: Dict<K, V>, k: K, out: *mut V) -> bool {
    match nre(d).borrow().get(&k) { Some(v) => { unsafe { *out = v }; true } None => false }
}
pub fn System_Collections_Generic_Dictionary_2__set_Item<K: PartialEq + Clone, V: Clone>(d: Dict<K, V>, k: K, v: V) { nre(d).borrow_mut().set(k, v) }
pub fn System_Collections_Generic_Dictionary_2__Remove<K: PartialEq + Clone, V: Clone>(d: Dict<K, V>, k: K) -> bool {
    let d = nre(d);
    let mut d = d.borrow_mut();
    match d.keys.iter().position(|x| *x == k) { Some(i) => { d.keys.remove(i); d.vals.remove(i); true } None => false }
}
pub fn System_String___ctor_1(a: Arr<u16>) -> Str {
    str_of(&String::from_utf16_lossy(&nre(a).borrow()))
}
pub fn System_Collections_Generic_KeyValuePair_2__get_Key<K, V>(kv: (K, V)) -> K { kv.0 }
pub fn System_Collections_Generic_KeyValuePair_2__get_Value<K, V>(kv: (K, V)) -> V { kv.1 }

// ---- List<T>.Enumerator (foreach): value struct, passed by pointer to methods ----
pub struct ListEnum<T> { pub list: List<T>, pub i: i32 }
impl<T> Clone for ListEnum<T> { fn clone(&self) -> Self { ListEnum { list: self.list.clone(), i: self.i } } }
impl<T> Default for ListEnum<T> { fn default() -> Self { ListEnum { list: None, i: 0 } } }
pub fn System_Collections_Generic_List_1__GetEnumerator<T>(l: List<T>) -> ListEnum<T> { ListEnum { list: l, i: 0 } }
pub fn System_Collections_Generic_List_1_Enumerator__MoveNext<T>(e: *mut ListEnum<T>) -> bool {
    unsafe {
        let n = (*e).list.as_ref().map(|l| l.borrow().len()).unwrap_or(0) as i32;
        if (*e).i < n { (*e).i += 1; true } else { false }
    }
}
pub fn System_Collections_Generic_List_1_Enumerator__get_Current<T: Clone>(e: *mut ListEnum<T>) -> T {
    unsafe { (*e).list.as_ref().expect("NullReferenceException").borrow()[((*e).i - 1) as usize].clone() }
}
pub fn System_Collections_Generic_List_1_Enumerator__Dispose<T>(_e: *mut ListEnum<T>) {}
