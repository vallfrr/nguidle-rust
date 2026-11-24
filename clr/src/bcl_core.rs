//! BCL intrinsics (System.*): strings, numbers, Math, collections, I/O.
use crate::code::NativeFn;
use crate::value::*;
use crate::Vm;
use std::rc::Rc;

// ------------------------------------------------------------------ helpers
pub fn rv(vm: &mut Vm, v: &Value) -> Value {
    vm.deref(v.clone())
}

fn st(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        _ => String::new(),
    }
}

pub fn new_list(vm: &mut Vm, items: Vec<Value>) -> Value {
    let id = vm.type_id("System.Collections.Generic.List`1");
    Value::Obj(Obj::new(id, 0, Ext::List(items)))
}

pub fn new_array(vm: &mut Vm, items: Vec<Value>) -> Value {
    let id = vm.type_id("System.Array");
    Value::Obj(Obj::new(id, 0, Ext::Arr(items)))
}

pub fn new_enum(vm: &mut Vm, items: Vec<Value>) -> Value {
    let id = vm.type_id("System.Collections.IEnumerator");
    Value::Obj(Obj::new(id, 0, Ext::Native(Box::new(NatEnum { items, idx: -1 }))))
}

pub struct NatEnum {
    pub items: Vec<Value>,
    pub idx: i64,
}

struct OrderedByDouble {
    source: Value,
    selector: Rc<Obj>,
}

fn linq_selector(vm: &mut Vm, value: &Value) -> R<Rc<Obj>> {
    match value {
        Value::Obj(o) if matches!(&*o.x.borrow(), Ext::Delegate(_)) => Ok(o.clone()),
        Value::Null => vm.throw("System.ArgumentNullException", "selector"),
        _ => vm.throw("System.InvalidCastException", "Expected a LINQ selector delegate."),
    }
}

pub fn kvp(vm: &mut Vm, k: Value, v: Value) -> Value {
    let id = vm.type_id("System.Collections.Generic.KeyValuePair`2");
    Value::Struct(Box::new(Struct { ty: id, f: vec![k, v] }))
}

/// Bytes of a byte[] (or list) without cloning the element vector first; same result as mapping `iter_values`.
pub fn bytes_of_value(vm: &mut Vm, v: &Value) -> R<Vec<u8>> {
    if let Value::Obj(o) = v {
        if let Ext::Arr(a) | Ext::List(a) = &*o.x.borrow() {
            return Ok(a.iter().map(|x| x.as_i32() as u8).collect());
        }
    }
    Ok(iter_values(vm, v)?.iter().map(|x| x.as_i32() as u8).collect())
}

/// Content of a collection (array, list, dictionary, user enumerator).
pub fn iter_values(vm: &mut Vm, v: &Value) -> R<Vec<Value>> {
    let v = vm.deref(v.clone());
    match &v {
        Value::Obj(o) => {
            let ordered = match &*o.x.borrow() {
                Ext::Native(b) => b.downcast_ref::<OrderedByDouble>().map(|o| (o.source.clone(), o.selector.clone())),
                _ => None,
            };
            if let Some((source, selector)) = ordered {
                let items = iter_values(vm, &source)?;
                let mut keyed = Vec::with_capacity(items.len());
                for item in items {
                    let Value::F64(cost) = vm.call_delegate(&selector, vec![item.clone()])? else {
                        return vm.throw("System.NotImplementedException", "OrderBy currently supports Double keys.");
                    };
                    keyed.push((cost, item));
                }
                // Double.CompareTo sorts NaN first and treats signed zeros as equal. Keep equal keys in source order.
                keyed.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or_else(|| b.is_nan().cmp(&a.is_nan())));
                return Ok(keyed.into_iter().map(|(_, item)| item).collect());
            }
            let direct = match &*o.x.borrow() {
                Ext::Arr(a) | Ext::List(a) => Some(a.clone()),
                Ext::Dict(d) => Some(d.iter().map(|(k, v)| (k.clone(), v.clone())).flat_map(|(k, v)| [k, v]).collect::<Vec<_>>()),
                _ => None,
            };
            if let Some(d) = direct {
                if matches!(&*o.x.borrow(), Ext::Dict(_)) {
                    let mut out = vec![];
                    for ch in d.chunks(2) {
                        out.push(kvp(vm, ch[0].clone(), ch[1].clone()));
                    }
                    return Ok(out);
                }
                return Ok(d);
            }
            // user enumerable
            if let Some(m) = vm.find_virtual_by_name(o.ty, "GetEnumerator", 1) {
                let e = vm.invoke(m, vec![v.clone()])?;
                return drain_enumerator(vm, e);
            }
            if matches!(&*o.x.borrow(), Ext::Native(_)) {
                return drain_enumerator(vm, v.clone());
            }
            Ok(vec![])
        }
        Value::Str(s) => Ok(s.encode_utf16().map(|c| Value::I32(c as i32)).collect()),
        Value::Null => vm.throw("System.ArgumentNullException", "source"),
        _ => Ok(vec![]),
    }
}

fn drain_enumerator(vm: &mut Vm, e: Value) -> R<Vec<Value>> {
    if let Value::Obj(o) = &e {
        let items = match &mut *o.x.borrow_mut() {
            Ext::Native(b) => b.downcast_mut::<NatEnum>().map(|n| {
                let r = n.items[(n.idx + 1) as usize..].to_vec();
                n.idx = n.items.len() as i64;
                r
            }),
            _ => None,
        };
        if let Some(i) = items {
            return Ok(i);
        }
        // user enumerator
        let mn = vm.find_virtual_by_name(o.ty, "MoveNext", 1);
        let cur = vm.find_virtual_by_name(o.ty, "get_Current", 1);
        if let (Some(mn), Some(cur)) = (mn, cur) {
            let mut out = vec![];
            while vm.invoke(mn, vec![e.clone()])?.as_bool() {
                out.push(vm.invoke(cur, vec![e.clone()])?);
            }
            return Ok(out);
        }
    }
    Ok(vec![])
}

fn list_ref(v: &Value) -> Option<Rc<Obj>> {
    match v {
        Value::Obj(o) => Some(o.clone()),
        _ => None,
    }
}

macro_rules! with_vec {
    ($vm:expr, $v:expr, $f:expr) => {{
        let t = $vm.deref($v.clone());
        match &t {
            Value::Obj(o) => match &mut *o.x.borrow_mut() {
                Ext::List(l) | Ext::Arr(l) => Ok($f(l)),
                _ => $vm.throw("System.InvalidProgramException", "not a list"),
            },
            _ => $vm.throw("System.NullReferenceException", "liste null"),
        }
    }};
}

// ------------------------------------------------------------------ string conversions

// ------------------------------------------------------------------ number display culture
// Like Mono in the original game: separators of the system language (Windows in French: `50 000`, `48,9`).
// NGU_CULTURE=fr|en|de|invariant forces the culture (tests). ponytail: separator table per language (no CLDR data);
// identical copy in clr/src/bcl_core.rs and crates/ngu-rt/src/fmt.rs (oracle and native must stay equal).
pub fn culture() -> (char, char) {
    static C: std::sync::OnceLock<(char, char)> = std::sync::OnceLock::new();
    *C.get_or_init(|| {
        let l = std::env::var("NGU_CULTURE").ok().or_else(sys_lang).unwrap_or_default().to_ascii_lowercase();
        match &l[..l.len().min(2)] {
            "fr" | "ru" | "pl" | "cs" | "sk" | "sv" | "fi" | "nb" | "no" | "uk" | "hu" | "bg" => ('\u{a0}', ','),
            "de" | "es" | "it" | "pt" | "nl" | "tr" | "id" | "da" | "el" | "ro" | "hr" | "sl" | "sr" | "vi" => ('.', ','),
            _ => (',', '.'),
        }
    })
}
#[cfg(windows)]
fn sys_lang() -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultLocaleName(buf: *mut u16, n: i32) -> i32;
    }
    let mut b = [0u16; 85];
    let n = unsafe { GetUserDefaultLocaleName(b.as_mut_ptr(), 85) };
    (n > 1).then(|| String::from_utf16_lossy(&b[..n as usize - 1]))
}
#[cfg(not(windows))]
fn sys_lang() -> Option<String> {
    ["LC_ALL", "LC_NUMERIC", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}
/// Invariant text (`1,234.5`) -> display culture.
pub fn loc(s: String) -> String {
    let (g, d) = culture();
    if (g, d) == (',', '.') {
        return s;
    }
    s.chars().map(|c| match c { ',' => g, '.' => d, c => c }).collect()
}
pub fn fmt_float(x: f64, digits: usize) -> String {
    loc(fmt_float_inv(x, digits))
}
/// .NET number format (standard and custom), in the display culture.
pub fn format_number(x: f64, is_int: bool, fmt: &str) -> String {
    loc(format_number_inv(x, is_int, fmt))
}
fn fmt_float_inv(x: f64, digits: usize) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let s = format!("{:.*e}", digits - 1, x);
    let (m, e) = s.split_once('e').unwrap();
    let exp: i32 = e.parse().unwrap();
    let neg = m.starts_with('-');
    let m = m.trim_start_matches('-');
    let mut digs: String = m.chars().filter(|c| *c != '.').collect();
    while digs.len() > 1 && digs.ends_with('0') {
        digs.pop();
    }
    let sign = if neg { "-" } else { "" };
    if exp >= -5 && exp < digits as i32 {
        if exp >= 0 {
            let e = exp as usize;
            if digs.len() <= e + 1 {
                format!("{}{}{}", sign, digs, "0".repeat(e + 1 - digs.len()))
            } else {
                format!("{}{}.{}", sign, &digs[..e + 1], &digs[e + 1..])
            }
        } else {
            format!("{}0.{}{}", sign, "0".repeat((-exp - 1) as usize), digs)
        }
    } else {
        let mant = if digs.len() > 1 { format!("{}.{}", &digs[..1], &digs[1..]) } else { digs };
        format!("{}{}E{}{:02}", sign, mant, if exp < 0 { "-" } else { "+" }, exp.abs())
    }
}

fn group3(int: &str) -> String {
    let b = int.as_bytes();
    let mut out = String::new();
    for (i, c) in b.iter().enumerate() {
        if i > 0 && (b.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c as char);
    }
    out
}

/// .NET number format (standard and custom).
fn format_number_inv(x: f64, is_int: bool, fmt: &str) -> String {
    let f = fmt.trim();
    let first = f.chars().next().unwrap_or('G');
    let rest = &f[first.len_utf8().min(f.len())..];
    if f.is_empty() {
        return if is_int { format!("{}", x as i64) } else { fmt_float_inv(x, 15) };
    }
    if first.is_ascii_alphabetic() && rest.chars().all(|c| c.is_ascii_digit()) {
        let prec: Option<usize> = if rest.is_empty() { None } else { rest.parse().ok() };
        match first.to_ascii_uppercase() {
            'X' => {
                let s = format!("{:X}", x as i64);
                return if let Some(p) = prec { format!("{:0>1$}", s, p) } else { s };
            }
            'D' => {
                let neg = x < 0.0;
                let s = format!("{}", (x as i64).abs());
                let s = if let Some(p) = prec { format!("{:0>1$}", s, p) } else { s };
                return if neg { format!("-{}", s) } else { s };
            }
            'F' => return format!("{:.*}", prec.unwrap_or(2), x),
            'N' => {
                let s = format!("{:.*}", prec.unwrap_or(2), x.abs());
                let (i, fr) = s.split_once('.').map(|(a, b)| (a.to_string(), Some(b.to_string()))).unwrap_or((s, None));
                let body = match fr { Some(fr) => format!("{}.{}", group3(&i), fr), None => group3(&i) };
                return if x < 0.0 { format!("-{}", body) } else { body };
            }
            'E' => {
                let p = prec.unwrap_or(6);
                let s = format!("{:.*e}", p, x);
                let (m, e) = s.split_once('e').unwrap();
                let ev: i32 = e.parse().unwrap();
                return format!("{}E{}{:03}", m, if ev < 0 { '-' } else { '+' }, ev.abs());
            }
            'G' => return if is_int { format!("{}", x as i64) } else { fmt_float_inv(x, prec.unwrap_or(15)) },
            _ => {}
        }
    }
    // custom format
    let (int_pat, frac_pat) = match f.split_once('.') {
        Some((a, b)) => (a, b),
        None => (f, ""),
    };
    let grouping = int_pat.contains(',');
    let int_zeros = int_pat.chars().filter(|c| *c == '0').count();
    let frac_max = frac_pat.chars().filter(|c| *c == '0' || *c == '#').count();
    let frac_min = frac_pat.chars().filter(|c| *c == '0').count();
    let neg = x < 0.0 && (x.abs() * 10f64.powi(frac_max as i32)).round() != 0.0;
    let s = format!("{:.*}", frac_max, x.abs());
    let (ip, fp) = match s.split_once('.') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (s.clone(), String::new()),
    };
    let mut ip = ip.trim_start_matches('0').to_string();
    while ip.len() < int_zeros {
        ip.insert(0, '0');
    }
    let ip = if grouping { group3(&ip) } else { ip };
    let mut fp = fp;
    while fp.len() > frac_min && fp.ends_with('0') {
        fp.pop();
    }
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&ip);
    if !fp.is_empty() {
        out.push('.');
        out.push_str(&fp);
    }
    if out.is_empty() || out == "-" {
        out.push('0');
    }
    out
}

pub fn enum_name(vm: &Vm, ty: u32, v: i32) -> Option<String> {
    let ti = vm.types[ty as usize].user?;
    let res = &vm.prog.res;
    for (_, f) in res.enumerate_fields(ti) {
        if f.literal {
            if let Some(c) = &f.default {
                let cv = match c {
                    dotnetdll::resolved::members::Constant::Int32(x) => *x as i64,
                    dotnetdll::resolved::members::Constant::UInt8(x) => *x as i64,
                    dotnetdll::resolved::members::Constant::Int16(x) => *x as i64,
                    dotnetdll::resolved::members::Constant::UInt16(x) => *x as i64,
                    dotnetdll::resolved::members::Constant::UInt32(x) => *x as i64,
                    dotnetdll::resolved::members::Constant::Int64(x) => *x,
                    dotnetdll::resolved::members::Constant::Int8(x) => *x as i64,
                    _ => continue,
                };
                if cv == v as i64 {
                    return Some(f.name.to_string());
                }
            }
        }
    }
    None
}

/// Equivalent of v.ToString() (calls the user ToString if present).
pub fn to_string(vm: &mut Vm, v: &Value) -> R<String> {
    let v = vm.deref(v.clone());
    Ok(match &v {
        Value::Null => String::new(),
        Value::I32(i) => i.to_string(),
        Value::I64(i) => i.to_string(),
        Value::F32(f) => fmt_float(*f as f64, 7),
        Value::F64(f) => fmt_float(*f, 15),
        Value::Str(s) => s.to_string(),
        Value::Obj(o) => {
            if let Ext::Boxed(b) = &*o.x.borrow() {
                if vm.types[o.ty as usize].is_enum {
                    if let Value::I32(i) = b {
                        return Ok(enum_name(vm, o.ty, *i).unwrap_or_else(|| i.to_string()));
                    }
                }
            }
            if let Some(m) = vm.find_virtual_by_name(o.ty, "ToString", 1) {
                let r = vm.invoke(m, vec![v.clone()])?;
                return Ok(st(&r));
            }
            if let Ext::SB(s) = &*o.x.borrow() {
                return Ok(s.clone());
            }
            vm.types[o.ty as usize].name.clone()
        }
        Value::Struct(s) => {
            if let Some(m) = vm.find_virtual_by_name(s.ty, "ToString", 1) {
                let r = vm.invoke(m, vec![v.clone()])?;
                return Ok(st(&r));
            }
            vm.types[s.ty as usize].name.clone()
        }
        Value::Ptr(_) => String::new(),
    })
}

fn tostring_fn(vm: &mut Vm, a: &mut Vec<Value>) -> R<Value> {
    let v = rv(vm, &a[0]);
    if a.len() > 1 {
        if let Value::Str(f) = &a[1] {
            let s = match &v {
                Value::I32(i) => format_number(*i as f64, true, f),
                Value::I64(i) => format_number(*i as f64, true, f),
                Value::F32(x) => format_number(*x as f64, false, f),
                Value::F64(x) => format_number(*x, false, f),
                _ => to_string(vm, &v)?,
            };
            return Ok(Value::str(&s));
        }
    }
    let s = to_string(vm, &v)?;
    Ok(Value::str(&s))
}

fn parse_err<T>(vm: &mut Vm) -> R<T> {
    vm.throw("System.FormatException", "Input string was not in a correct format.")
}

fn integer_overflow<T>(vm: &mut Vm) -> R<T> {
    vm.throw("System.OverflowException", "Value was outside the range of the destination integer type.")
}

fn numeric_text(s: &str) -> &str {
    s.trim_end_matches('\0').trim_matches(|c: char| c.is_ascii_whitespace() || c == '\u{000b}')
}

fn parse_integer<T: std::str::FromStr<Err = std::num::ParseIntError>>(vm: &mut Vm, arg: &Value) -> R<T> {
    let Value::Str(s) = arg else {
        return vm.throw("System.ArgumentNullException", "Input string was null.");
    };
    match numeric_text(s).parse() {
        Ok(v) => Ok(v),
        Err(e) if matches!(e.kind(), std::num::IntErrorKind::PosOverflow | std::num::IntErrorKind::NegOverflow) => integer_overflow(vm),
        Err(_) => parse_err(vm),
    }
}

fn parse_float(vm: &mut Vm, arg: &Value, single: bool) -> R<Value> {
    let Value::Str(s) = arg else {
        return vm.throw("System.ArgumentNullException", "Input string was null.");
    };
    let input = numeric_text(s);
    let symbol = match input {
        "NaN" => Some(f64::NAN),
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        _ => None,
    };
    if let Some(value) = symbol {
        return Ok(if single { Value::F32(value as f32) } else { Value::F64(value) });
    }

    let (group, decimal) = culture();
    let mut normalized = String::with_capacity(input.len());
    let mut group_allowed = false;
    let mut integer_part = true;
    for c in input.chars() {
        match c {
            '0'..='9' => {
                group_allowed = integer_part;
                normalized.push(c);
            }
            c if c == decimal => {
                integer_part = false;
                group_allowed = false;
                normalized.push('.');
            }
            c if group_allowed && (c == group || (group == '\u{a0}' && c == ' ')) => {}
            'e' | 'E' => {
                integer_part = false;
                group_allowed = false;
                normalized.push(c);
            }
            '+' | '-' => normalized.push(c),
            _ => return parse_err(vm),
        }
    }
    let Ok(value) = normalized.parse::<f64>() else { return parse_err(vm) };
    if value.is_infinite() || (single && (value as f32).is_infinite()) {
        return vm.throw("System.OverflowException", "Value was outside the range of the destination floating point type.");
    }
    let value = if value == 0.0 { 0.0 } else { value };
    Ok(if single { Value::F32(value as f32) } else { Value::F64(value) })
}

fn rounded_integer(vm: &mut Vm, value: f64, exclusive_max: f64) -> R<i64> {
    let rounded = value.round_ties_even();
    if rounded >= -exclusive_max && rounded < exclusive_max {
        Ok(rounded as i64)
    } else {
        integer_overflow(vm)
    }
}

fn is_integral_decimal(vm: &Vm, value: &Value) -> bool {
    matches!(value, Value::Struct(s) if vm.types[s.ty as usize].name == "System.Decimal" && matches!(s.f.as_slice(), [Value::I64(_)]))
}

fn prefers<T: PartialOrd>(x: T, y: T, minimum: bool) -> bool {
    if minimum { x < y } else { x > y }
}

fn math_extreme(a: &[Value], minimum: bool, unity: bool) -> Value {
    let choose_i64 = |x, y| if prefers(x, y, minimum) { x } else { y };
    let choose_float = |x: f64, y: f64| if (!unity && x.is_nan()) || prefers(x, y, minimum) { x } else { y };
    match (&a[0], &a[1]) {
        (Value::I32(x), Value::I32(y)) => Value::I32(choose_i64(*x as i64, *y as i64) as i32),
        (Value::I64(_), _) | (_, Value::I64(_)) => Value::I64(choose_i64(a[0].as_i64(), a[1].as_i64())),
        (Value::F32(x), Value::F32(y)) => Value::F32(choose_float(*x as f64, *y as f64) as f32),
        _ => Value::F64(choose_float(a[0].as_f64(), a[1].as_f64())),
    }
}

fn mathf_extreme(vm: &mut Vm, a: &[Value], minimum: bool) -> R<Value> {
    if a.len() == 2 {
        return Ok(math_extreme(a, minimum, true));
    }
    let items = iter_values(vm, &a[0])?;
    let mut it = items.into_iter();
    let first = it.next().unwrap_or(Value::F32(0.0));
    Ok(it.fold(first, |best, current| {
        if prefers(current.as_f32(), best.as_f32(), minimum) { current } else { best }
    }))
}

fn mathf_clamp<T: PartialOrd>(value: T, min: T, max: T) -> T {
    if value < min { min } else if value > max { max } else { value }
}

// ------------------------------------------------------------------ registry
pub fn norm_type(t: &str) -> &str {
    match t {
        "System.Collections.Generic.IList`1" | "System.Collections.Generic.ICollection`1" | "System.Collections.IList"
        | "System.Collections.Generic.IReadOnlyList`1" | "System.Collections.Generic.IReadOnlyCollection`1" | "System.Collections.ICollection" => "System.Collections.Generic.List`1",
        "System.Collections.Generic.IDictionary`2" | "System.Collections.Generic.IReadOnlyDictionary`2" => "System.Collections.Generic.Dictionary`2",
        "System.Collections.Generic.IEnumerable`1" | "System.Collections.IEnumerable" => "System.Collections.IEnumerable",
        "System.Collections.Generic.IEnumerator`1" | "System.Collections.IEnumerator"
        | "System.Collections.Generic.List`1/Enumerator" | "System.Collections.Generic.Dictionary`2/Enumerator" => "System.Collections.IEnumerator",
        "System.Object" => "System.Object",
        other => other,
    }
}

mod collections;
mod foundation;
mod numbers;
mod services;

pub fn lookup(key: &str) -> Option<NativeFn> {
    let (t, m) = key.split_once("::")?;
    let nk = format!("{}::{}", norm_type(t), m);
    let key = nk.as_str();
    foundation::lookup(key)
        .or_else(|| numbers::lookup(key))
        .or_else(|| collections::lookup(key))
        .or_else(|| services::lookup(key))
        .or_else(|| crate::bcl_sys::lookup(key))
}

pub fn b64_enc(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(T[(n >> 18) as usize & 63] as char);
        o.push(T[(n >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        o.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    o
}

pub fn b64_dec(s: &str) -> Vec<u8> {
    let mut out = vec![];
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        } as u32;
        buf = buf << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    out
}

#[allow(dead_code)]
fn unused(_: Option<Rc<Obj>>) {
    let _ = list_ref;
}

fn enum_tid(vm: &Vm, t: &Value) -> Option<u32> {
    if let Value::Obj(o) = t {
        if let Ext::Native(b) = &*o.x.borrow() {
            if let Some(n) = b.downcast_ref::<String>() {
                return vm.type_ids.get(n).copied();
            }
        }
    }
    None
}

/// Literal members (name, value) of a user enum, in declaration order.
pub fn enum_members(vm: &Vm, t: &Value) -> Vec<(String, i64)> {
    let mut out = vec![];
    if let Some(id) = enum_tid(vm, t) {
        if let Some(ti) = vm.types[id as usize].user {
            for (_, f) in vm.prog.res.enumerate_fields(ti) {
                if f.literal {
                    use dotnetdll::resolved::members::Constant as C;
                    let v = match &f.default {
                        Some(C::Int32(x)) => *x as i64,
                        Some(C::UInt8(x)) => *x as i64,
                        Some(C::Int16(x)) => *x as i64,
                        Some(C::UInt16(x)) => *x as i64,
                        Some(C::UInt32(x)) => *x as i64,
                        Some(C::Int64(x)) => *x,
                        Some(C::Int8(x)) => *x as i64,
                        _ => continue,
                    };
                    out.push((f.name.to_string(), v));
                }
            }
        }
    }
    out
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}
