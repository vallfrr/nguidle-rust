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

pub fn lookup(key: &str) -> Option<NativeFn> {
    let (t, m) = key.split_once("::")?;
    let nk = format!("{}::{}", norm_type(t), m);
    let key = nk.as_str();
    let f: NativeFn = match key {
        "System.Linq.Enumerable::OrderBy" => |vm, a| {
            if a[0].is_null() {
                return vm.throw("System.ArgumentNullException", "source");
            }
            let selector = linq_selector(vm, &a[1])?;
            let ty = vm.type_id("System.Linq.IOrderedEnumerable`1");
            let ordered = OrderedByDouble { source: a[0].clone(), selector };
            Ok(Value::Obj(Obj::new(ty, 0, Ext::Native(Box::new(ordered)))))
        },
        "System.Linq.Enumerable::ToDictionary" => |vm, a| {
            let keys = linq_selector(vm, &a[1])?;
            let values = linq_selector(vm, &a[2])?;
            let items = iter_values(vm, &a[0])?;
            let mut result = Dict::default();
            for item in items {
                let key = vm.call_delegate(&keys, vec![item.clone()])?;
                let value = vm.call_delegate(&values, vec![item])?;
                if key.is_null() {
                    return vm.throw("System.ArgumentNullException", "key");
                }
                if result.get(&key).is_some() {
                    return vm.throw("System.ArgumentException", "An item with the same key has already been added.");
                }
                result.set(key, value);
            }
            let ty = vm.type_id("System.Collections.Generic.Dictionary`2");
            Ok(Value::Obj(Obj::new(ty, 0, Ext::Dict(result))))
        },
        "System.Linq.Enumerable::ElementAt" => |vm, a| {
            let index = a[1].as_i32() as usize;
            match iter_values(vm, &a[0])?.into_iter().nth(index) {
                Some(item) => Ok(item),
                None => vm.throw("System.ArgumentOutOfRangeException", "index"),
            }
        },
        // The game uses Sum(IEnumerable<int>) for card mana costs.
        "System.Linq.Enumerable::Sum" => |vm, a| {
            let mut sum = 0i32;
            for item in iter_values(vm, &a[0])? {
                let Some(next) = sum.checked_add(item.as_i32()) else {
                    return vm.throw("System.OverflowException", "Sum exceeds the range of Int32.");
                };
                sum = next;
            }
            Ok(Value::I32(sum))
        },
        // ---------------------------------------------------------- Object
        "System.Object::ToString" => tostring_fn,
        "System.Object::Equals" | "System.Object::ReferenceEquals" => |vm, a| {
            let (x, y) = (rv(vm, &a[0]), rv(vm, &a[1]));
            Ok(Value::bool(crate::vm::values_eq(&x, &y)))
        },
        "System.Object::GetHashCode" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::I32(match hkey(&v) {
                HKey::I(i) => i as i32,
                HKey::S(s) => s.bytes().fold(5381i32, |h, b| h.wrapping_mul(33).wrapping_add(b as i32)),
                HKey::P(p) => (p >> 4) as i32,
                _ => 0,
            }))
        },
        "System.Object::GetType" => |vm, a| {
            let v = rv(vm, &a[0]);
            let n = vm.type_name_of(&v);
            let id = vm.type_id("System.Type");
            Ok(Value::Obj(Obj::new(id, 0, Ext::Native(Box::new(n)))))
        },
        "System.Type::GetTypeFromHandle" => |_vm, a| Ok(a[0].clone()),
        "System.Type::get_Name" | "System.Type::get_FullName" | "System.Type::ToString" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                if let Ext::Native(b) = &*o.x.borrow() {
                    if let Some(s) = b.downcast_ref::<String>() {
                        return Ok(Value::str(s));
                    }
                }
            }
            Ok(Value::Null)
        },
        // ---------------------------------------------------------- Exceptions
        "System.Exception::.ctor" | "System.SystemException::.ctor" | "System.NotSupportedException::.ctor"
        | "System.NotImplementedException::.ctor" | "System.InvalidOperationException::.ctor"
        | "System.ArgumentException::.ctor" | "System.FormatException::.ctor" | "System.ApplicationException::.ctor" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                let mut f = o.f.borrow_mut();
                if a.len() > 1 && !f.is_empty() {
                    f[0] = a[1].clone();
                }
                if a.len() > 2 && f.len() > 1 {
                    f[1] = a[2].clone();
                }
            }
            Ok(Value::Null)
        },
        "System.Exception::get_Message" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                return Ok(o.f.borrow().first().cloned().unwrap_or(Value::Null));
            }
            Ok(Value::Null)
        },
        "System.Exception::get_InnerException" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                return Ok(o.f.borrow().get(1).cloned().unwrap_or(Value::Null));
            }
            Ok(Value::Null)
        },
        "System.IDisposable::Dispose" | "System.ComponentModel.Component::Dispose" => |_vm, _a| Ok(Value::Null),
        // ---------------------------------------------------------- String
        "System.String::Concat" => |vm, a| {
            let mut s = String::new();
            let items: Vec<Value> = if a.len() == 1 {
                let v = rv(vm, &a[0]);
                match &v {
                    Value::Obj(_) => iter_values(vm, &v)?,
                    other => vec![other.clone()],
                }
            } else {
                a.clone()
            };
            for v in &items {
                s.push_str(&to_string(vm, v)?);
            }
            Ok(Value::str(&s))
        },
        "System.String::get_Length" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::I32(match &v { Value::Str(s) => (if s.is_ascii() { s.len() } else { s.encode_utf16().count() }) as i32, _ => 0 }))
        },
        "System.String::get_Chars" => |vm, a| {
            let v = rv(vm, &a[0]);
            let i = a[1].as_i32();
            match &v {
                Value::Str(s) => match if s.is_ascii() { s.as_bytes().get(i as usize).map(|b| *b as u16) } else { s.encode_utf16().nth(i as usize) } {
                    Some(c) if i >= 0 => Ok(Value::I32(c as i32)),
                    _ => vm.throw("System.IndexOutOfRangeException", "Index was outside the bounds of the array."),
                },
                _ => vm.throw("System.NullReferenceException", "string null"),
            }
        },
        "System.String::op_Equality" => |_vm, a| Ok(Value::bool(match (&a[0], &a[1]) {
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::Null, Value::Null) => true,
            _ => false,
        })),
        "System.String::op_Inequality" => |_vm, a| Ok(Value::bool(!match (&a[0], &a[1]) {
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::Null, Value::Null) => true,
            _ => false,
        })),
        "System.String::Equals" => |vm, a| {
            let (x, y) = (rv(vm, &a[0]), rv(vm, &a[1]));
            Ok(Value::bool(match (&x, &y) { (Value::Str(p), Value::Str(q)) => p == q, _ => false }))
        },
        "System.String::IsNullOrEmpty" => |_vm, a| Ok(Value::bool(match &a[0] { Value::Str(s) => s.is_empty(), _ => true })),
        "System.String::Substring" => |vm, a| {
            let s = st(&rv(vm, &a[0]));
            let u: Vec<u16> = s.encode_utf16().collect();
            let start = a[1].as_i32();
            let len = if a.len() > 2 { a[2].as_i32() } else { u.len() as i32 - start };
            if start < 0 || len < 0 || (start + len) as usize > u.len() {
                return vm.throw("System.ArgumentOutOfRangeException", "Substring");
            }
            Ok(Value::str(&String::from_utf16_lossy(&u[start as usize..(start + len) as usize])))
        },
        "System.String::IndexOf" => |vm, a| {
            let s = st(&rv(vm, &a[0]));
            let r = match &a[1] {
                Value::Str(p) => s.find(&**p),
                v => char::from_u32(v.as_i32() as u32).and_then(|c| s.find(c)),
            };
            Ok(Value::I32(r.map(|i| s[..i].encode_utf16().count() as i32).unwrap_or(-1)))
        },
        "System.String::LastIndexOf" => |vm, a| {
            let s = st(&rv(vm, &a[0]));
            let r = match &a[1] {
                Value::Str(p) => s.rfind(&**p),
                v => char::from_u32(v.as_i32() as u32).and_then(|c| s.rfind(c)),
            };
            Ok(Value::I32(r.map(|i| s[..i].encode_utf16().count() as i32).unwrap_or(-1)))
        },
        "System.String::Contains" => |vm, a| Ok(Value::bool(st(&rv(vm, &a[0])).contains(&st(&a[1])))),
        "System.String::StartsWith" => |vm, a| Ok(Value::bool(st(&rv(vm, &a[0])).starts_with(&st(&a[1])))),
        "System.String::EndsWith" => |vm, a| Ok(Value::bool(st(&rv(vm, &a[0])).ends_with(&st(&a[1])))),
        "System.String::Replace" => |vm, a| {
            let s = st(&rv(vm, &a[0]));
            let (from, to) = match (&a[1], &a[2]) {
                (Value::Str(f), Value::Str(t)) => (f.to_string(), t.to_string()),
                (f, t) => (char::from_u32(f.as_i32() as u32).unwrap_or(' ').to_string(), char::from_u32(t.as_i32() as u32).unwrap_or(' ').to_string()),
            };
            Ok(Value::str(&s.replace(&from, &to)))
        },
        "System.String::ToUpper" | "System.String::ToUpperInvariant" => |vm, a| Ok(Value::str(&st(&rv(vm, &a[0])).to_uppercase())),
        "System.String::ToLower" | "System.String::ToLowerInvariant" => |vm, a| Ok(Value::str(&st(&rv(vm, &a[0])).to_lowercase())),
        "System.String::Trim" => |vm, a| Ok(Value::str(st(&rv(vm, &a[0])).trim())),
        "System.String::Remove" => |vm, a| {
            let u: Vec<u16> = st(&rv(vm, &a[0])).encode_utf16().collect();
            let start = a[1].as_i32() as usize;
            let end = if a.len() > 2 { start + a[2].as_i32() as usize } else { u.len() };
            let mut o = u[..start.min(u.len())].to_vec();
            o.extend_from_slice(&u[end.min(u.len())..]);
            Ok(Value::str(&String::from_utf16_lossy(&o)))
        },
        "System.String::Insert" => |vm, a| {
            let u: Vec<u16> = st(&rv(vm, &a[0])).encode_utf16().collect();
            let i = (a[1].as_i32() as usize).min(u.len());
            let mut o = u[..i].to_vec();
            o.extend(st(&a[2]).encode_utf16());
            o.extend_from_slice(&u[i..]);
            Ok(Value::str(&String::from_utf16_lossy(&o)))
        },
        "System.String::PadLeft" => |vm, a| {
            let s = st(&rv(vm, &a[0]));
            let n = a[1].as_i32() as usize;
            let c = if a.len() > 2 { char::from_u32(a[2].as_i32() as u32).unwrap_or(' ') } else { ' ' };
            let l = s.encode_utf16().count();
            Ok(Value::str(&if l >= n { s } else { format!("{}{}", c.to_string().repeat(n - l), s) }))
        },
        "System.String::PadRight" => |vm, a| {
            let s = st(&rv(vm, &a[0]));
            let n = a[1].as_i32() as usize;
            let c = if a.len() > 2 { char::from_u32(a[2].as_i32() as u32).unwrap_or(' ') } else { ' ' };
            let l = s.encode_utf16().count();
            Ok(Value::str(&if l >= n { s } else { format!("{}{}", s, c.to_string().repeat(n - l)) }))
        },
        "System.String::Split" => |vm, a| {
            let s = st(&rv(vm, &a[0]));
            let seps: Vec<String> = match &a[1] {
                Value::Obj(_) => iter_values(vm, &a[1])?.iter().map(|v| match v {
                    Value::Str(x) => x.to_string(),
                    o => char::from_u32(o.as_i32() as u32).unwrap_or(' ').to_string(),
                }).collect(),
                _ => vec![],
            };
            let parts: Vec<Value> = if seps.is_empty() {
                s.split_whitespace().map(Value::str).collect()
            } else {
                let mut out = vec![];
                let mut cur = String::new();
                let mut rest = s.as_str();
                'o: while !rest.is_empty() {
                    for sp in &seps {
                        if !sp.is_empty() && rest.starts_with(sp.as_str()) {
                            out.push(Value::str(&cur));
                            cur.clear();
                            rest = &rest[sp.len()..];
                            continue 'o;
                        }
                    }
                    let c = rest.chars().next().unwrap();
                    cur.push(c);
                    rest = &rest[c.len_utf8()..];
                }
                out.push(Value::str(&cur));
                out
            };
            Ok(new_array(vm, parts))
        },
        "System.String::Format" => |vm, a| {
            let fmt = st(&a[0]);
            let args: Vec<Value> = if a.len() == 2 && matches!(&a[1], Value::Obj(o) if matches!(&*o.x.borrow(), Ext::Arr(_))) {
                iter_values(vm, &a[1])?
            } else {
                a[1..].to_vec()
            };
            let mut out = String::new();
            let cs: Vec<char> = fmt.chars().collect();
            let mut i = 0;
            while i < cs.len() {
                if cs[i] == '{' {
                    if i + 1 < cs.len() && cs[i + 1] == '{' { out.push('{'); i += 2; continue; }
                    let j = cs[i..].iter().position(|c| *c == '}').map(|p| p + i).unwrap_or(cs.len() - 1);
                    let inner: String = cs[i + 1..j].iter().collect();
                    let (idx, f) = inner.split_once(':').map(|(x, y)| (x.to_string(), Some(y.to_string()))).unwrap_or((inner.clone(), None));
                    let n: usize = idx.split(',').next().unwrap().trim().parse().unwrap_or(0);
                    if let Some(v) = args.get(n) {
                        let v = vm.deref(v.clone());
                        let s = match (&v, &f) {
                            (Value::I32(x), Some(f)) => format_number(*x as f64, true, f),
                            (Value::I64(x), Some(f)) => format_number(*x as f64, true, f),
                            (Value::F32(x), Some(f)) => format_number(*x as f64, false, f),
                            (Value::F64(x), Some(f)) => format_number(*x, false, f),
                            _ => to_string(vm, &v)?,
                        };
                        out.push_str(&s);
                    }
                    i = j + 1;
                } else if cs[i] == '}' && i + 1 < cs.len() && cs[i + 1] == '}' {
                    out.push('}');
                    i += 2;
                } else {
                    out.push(cs[i]);
                    i += 1;
                }
            }
            Ok(Value::str(&out))
        },
        "System.String::.ctor" => |vm, a| {
            // new string(char[]) / new string(c, n): returned through NewObjExt, unsupported (reference) -> placeholder
            let _ = (vm, a);
            Ok(Value::Null)
        },
        "System.String::Join" => |vm, a| {
            let sep = st(&a[0]);
            let items = if a.len() == 2 { iter_values(vm, &a[1])? } else { a[1..].to_vec() };
            let mut parts = vec![];
            for v in &items {
                parts.push(to_string(vm, v)?);
            }
            Ok(Value::str(&parts.join(&sep)))
        },
        "System.String::ToCharArray" => |vm, a| {
            let s = st(&rv(vm, &a[0]));
            let v = s.encode_utf16().map(|c| Value::I32(c as i32)).collect();
            Ok(new_array(vm, v))
        },
        "System.String::CompareOrdinal" | "System.String::Compare" | "System.String::CompareTo" => |vm, a| {
            let (x, y) = (st(&rv(vm, &a[0])), st(&a[1]));
            Ok(Value::I32(match x.cmp(&y) { std::cmp::Ordering::Less => -1, std::cmp::Ordering::Equal => 0, _ => 1 }))
        },
        "System.String::ToString" => |vm, a| Ok(rv(vm, &a[0])),
        // ---------------------------------------------------------- numbers -> string
        "System.Int32::ToString" | "System.Int64::ToString" | "System.Single::ToString" | "System.Double::ToString"
        | "System.Boolean::ToString" | "System.Byte::ToString" | "System.UInt64::ToString" | "System.UInt32::ToString"
        | "System.Char::ToString" | "System.Int16::ToString" | "System.Enum::ToString"
        | "System.Decimal::ToString" => |vm, a| {
            let v = rv(vm, &a[0]);
            if let (Value::I32(c), true) = (&v, a.is_empty()) { let _ = c; }
            if let Value::I32(i) = &v {
                // bool -> True/False not distinguishable here: handled by the method type
                let _ = i;
            }
            tostring_fn(vm, a)
        },
        "System.Int32::Parse" => |vm, a| Ok(Value::I32(parse_integer::<i32>(vm, &a[0])?)),
        "System.Int64::Parse" => |vm, a| Ok(Value::I64(parse_integer::<i64>(vm, &a[0])?)),
        "System.Int16::Parse" => |vm, a| Ok(Value::I32(parse_integer::<i16>(vm, &a[0])? as i32)),
        "System.Byte::Parse" => |vm, a| Ok(Value::I32(parse_integer::<u8>(vm, &a[0])? as i32)),
        "System.UInt32::Parse" => |vm, a| Ok(Value::I32(parse_integer::<u32>(vm, &a[0])? as i32)),
        "System.UInt64::Parse" => |vm, a| Ok(Value::I64(parse_integer::<u64>(vm, &a[0])? as i64)),
        "System.Single::Parse" => |vm, a| parse_float(vm, &a[0], true),
        "System.Double::Parse" => |vm, a| parse_float(vm, &a[0], false),
        "System.Single::IsNaN" | "System.Double::IsNaN" => |_vm, a| Ok(Value::bool(a[0].as_f64().is_nan())),
        "System.Single::IsInfinity" | "System.Double::IsInfinity" => |_vm, a| Ok(Value::bool(a[0].as_f64().is_infinite())),
        "System.Double::IsPositiveInfinity" => |_vm, a| Ok(Value::bool(a[0].as_f64() == f64::INFINITY)),
        "System.Double::IsNegativeInfinity" => |_vm, a| Ok(Value::bool(a[0].as_f64() == f64::NEG_INFINITY)),
        "System.Convert::ToInt32" => |vm, a| Ok(Value::I32(match &a[0] {
            Value::Str(_) => parse_integer::<i32>(vm, &a[0])?,
            Value::F32(_) | Value::F64(_) => rounded_integer(vm, a[0].as_f64(), 2_147_483_648.0)? as i32,
            Value::I64(v) => match i32::try_from(*v) { Ok(x) => x, Err(_) => return integer_overflow(vm) },
            v => v.as_i32(),
        })),
        "System.Convert::ToInt64" => |vm, a| Ok(Value::I64(match &a[0] {
            Value::Str(_) => parse_integer::<i64>(vm, &a[0])?,
            Value::F32(_) | Value::F64(_) => rounded_integer(vm, a[0].as_f64(), 9_223_372_036_854_775_808.0)?,
            v => v.as_i64(),
        })),
        "System.Convert::ToDouble" => |vm, a| match &a[0] {
            Value::Str(_) => parse_float(vm, &a[0], false),
            v => Ok(Value::F64(v.as_f64())),
        },
        "System.Convert::ToBase64String" => |vm, a| {
            let b: Vec<u8> = iter_values(vm, &a[0])?.iter().map(|v| v.as_i32() as u8).collect();
            Ok(Value::str(&b64_enc(&b)))
        },
        "System.Convert::FromBase64String" => |vm, a| {
            let b = b64_dec(&st(&a[0])).iter().map(|x| Value::I32(*x as i32)).collect();
            Ok(new_array(vm, b))
        },
        // The game only constructs Decimal from Int64. Keep that value exact through Floor.
        "System.Decimal::op_Implicit" => |vm, a| {
            let Value::I64(value) = a[0] else {
                return vm.throw("System.NotImplementedException", "Unsupported conversion to System.Decimal.");
            };
            let ty = vm.type_id("System.Decimal");
            Ok(Value::Struct(Box::new(Struct { ty, f: vec![Value::I64(value)] })))
        },
        "System.Decimal::op_Explicit" => |vm, a| {
            let value = rv(vm, &a[0]);
            if is_integral_decimal(vm, &value) {
                if let Value::Struct(s) = value {
                    return Ok(s.f[0].clone());
                }
            }
            vm.throw("System.NotImplementedException", "Unsupported conversion from System.Decimal.")
        },
        // ---------------------------------------------------------- Math
        "System.Math::Min" => |_vm, a| Ok(math_extreme(a, true, false)),
        "System.Math::Max" => |_vm, a| Ok(math_extreme(a, false, false)),
        "UnityEngine.Mathf::Min" => |vm, a| mathf_extreme(vm, a, true),
        "UnityEngine.Mathf::Max" => |vm, a| mathf_extreme(vm, a, false),
        "System.Math::Abs" | "UnityEngine.Mathf::Abs" => |vm, a| Ok(match &a[0] {
            Value::I32(x) => match x.checked_abs() { Some(v) => Value::I32(v), None => return integer_overflow(vm) },
            Value::I64(x) => match x.checked_abs() { Some(v) => Value::I64(v), None => return integer_overflow(vm) },
            Value::F32(x) => Value::F32(x.abs()),
            v => Value::F64(v.as_f64().abs()),
        }),
        "System.Math::Floor" => |vm, a| {
            let value = rv(vm, &a[0]);
            if is_integral_decimal(vm, &value) {
                return Ok(value);
            }
            Ok(Value::F64(value.as_f64().floor()))
        },
        "System.Math::Ceiling" => |_vm, a| Ok(Value::F64(a[0].as_f64().ceil())),
        "System.Math::Round" => |vm, a| {
            let value = a[0].as_f64();
            let Some(digits) = a.get(1).map(Value::as_i32) else {
                return Ok(Value::F64(value.round_ties_even()));
            };
            if !(0..=15).contains(&digits) {
                return vm.throw("System.ArgumentOutOfRangeException", "Rounding digits must be between 0 and 15.");
            }
            if value.abs() >= 1e16 || !value.is_finite() {
                return Ok(Value::F64(value));
            }
            let scale = 10_f64.powi(digits);
            Ok(Value::F64((value * scale).round_ties_even() / scale))
        },
        "System.Math::Sqrt" => |_vm, a| Ok(Value::F64(a[0].as_f64().sqrt())),
        "System.Math::Pow" => |_vm, a| Ok(Value::F64(ngu_rt::ext::System_Math__Pow__f64_f64(a[0].as_f64(), a[1].as_f64()))),
        "System.Math::Log" => |_vm, a| Ok(Value::F64(if a.len() > 1 {
            ngu_rt::ext::System_Math__Log__f64_f64(a[0].as_f64(), a[1].as_f64())
        } else {
            a[0].as_f64().ln()
        })),
        "System.Math::Log10" => |_vm, a| Ok(Value::F64(a[0].as_f64().log10())),
        "UnityEngine.Mathf::Floor" => |_vm, a| Ok(Value::F32(a[0].as_f32().floor())),
        "UnityEngine.Mathf::Ceil" => |_vm, a| Ok(Value::F32(a[0].as_f32().ceil())),
        "UnityEngine.Mathf::FloorToInt" => |_vm, a| Ok(Value::I32(ngu_rt::ext::UnityEngine_Mathf__FloorToInt__f32(a[0].as_f32()))),
        "UnityEngine.Mathf::CeilToInt" => |_vm, a| Ok(Value::I32(ngu_rt::f2i32(a[0].as_f64().ceil()))),
        "UnityEngine.Mathf::Sqrt" => |_vm, a| Ok(Value::F32(a[0].as_f32().sqrt())),
        "UnityEngine.Mathf::Pow" => |_vm, a| Ok(Value::F32(ngu_rt::ext::System_Math__Pow__f64_f64(a[0].as_f64(), a[1].as_f64()) as f32)),
        "UnityEngine.Mathf::Log10" => |_vm, a| Ok(Value::F32(a[0].as_f64().log10() as f32)),
        "UnityEngine.Mathf::Repeat" => |_vm, a| {
            let (t, l) = (a[0].as_f32(), a[1].as_f32());
            // Mathf.Floor receives a Single, but Mono retains the following intermediate product as a Double.
            let wrapped = f64::from(t) - f64::from((t / l).floor()) * f64::from(l);
            Ok(Value::F32(mathf_clamp(wrapped as f32, 0.0, l)))
        },
        "UnityEngine.Mathf::Clamp" => |_vm, a| Ok(match (&a[0], &a[1], &a[2]) {
            (Value::I32(x), Value::I32(lo), Value::I32(hi)) => Value::I32(mathf_clamp(*x, *lo, *hi)),
            _ => Value::F32(mathf_clamp(a[0].as_f32(), a[1].as_f32(), a[2].as_f32())),
        }),
        // ---------------------------------------------------------- Array / List
        "System.Array::get_Length" => |vm, a| {
            let n = with_vec!(vm, a[0], |l: &mut Vec<Value>| l.len())?;
            Ok(Value::I32(n as i32))
        },
        "System.Array::Empty" => |vm, _a| Ok(new_array(vm, vec![])),
        "System.Array::GetEnumerator" | "System.Collections.IEnumerable::GetEnumerator" => |vm, a| {
            let items = iter_values(vm, &a[0])?;
            Ok(new_enum(vm, items))
        },
        "System.Array::CopyTo" => |vm, a| {
            let src = iter_values(vm, &a[0])?;
            let off = a[2].as_i32() as usize;
            with_vec!(vm, a[1], |l: &mut Vec<Value>| {
                for (i, v) in src.into_iter().enumerate() {
                    if off + i < l.len() { l[off + i] = v; }
                }
            })?;
            Ok(Value::Null)
        },
        "System.Runtime.CompilerServices.RuntimeHelpers::InitializeArray" => |_vm, _a| Ok(Value::Null),
        "System.Collections.Generic.List`1::.ctor" => |vm, a| {
            let items = if a.len() > 1 && matches!(&a[1], Value::Obj(_)) { iter_values(vm, &a[1])? } else { vec![] };
            if let Value::Obj(o) = &a[0] {
                *o.x.borrow_mut() = Ext::List(items);
            }
            Ok(Value::Null)
        },
        "System.Collections.Generic.List`1::get_Count" | "System.Collections.Generic.Dictionary`2::get_Count" => |vm, a| {
            let t = vm.deref(a[0].clone());
            if let Value::Obj(o) = &t {
                match &*o.x.borrow() {
                    Ext::List(l) | Ext::Arr(l) => return Ok(Value::I32(l.len() as i32)),
                    Ext::Dict(d) => return Ok(Value::I32(d.live as i32)),
                    _ => {}
                }
            }
            vm.throw("System.NullReferenceException", "Count")
        },
        "System.Collections.Generic.List`1::get_Item" => |vm, a| {
            let i = a[1].as_i32();
            let r = with_vec!(vm, a[0], |l: &mut Vec<Value>| if i >= 0 { l.get(i as usize).cloned() } else { None })?;
            match r {
                Some(v) => Ok(v),
                None => vm.throw("System.ArgumentOutOfRangeException", "Index was out of range."),
            }
        },
        "System.Collections.Generic.List`1::set_Item" => |vm, a| {
            let i = a[1].as_i32();
            let v = a[2].clone();
            let ok = with_vec!(vm, a[0], |l: &mut Vec<Value>| if i >= 0 && (i as usize) < l.len() { l[i as usize] = v; true } else { false })?;
            if ok { Ok(Value::Null) } else { vm.throw("System.ArgumentOutOfRangeException", "Index was out of range.") }
        },
        "System.Collections.Generic.List`1::Add" => |vm, a| {
            let v = a[1].clone();
            with_vec!(vm, a[0], |l: &mut Vec<Value>| l.push(v))?;
            Ok(Value::Null)
        },
        "System.Collections.Generic.List`1::AddRange" => |vm, a| {
            let items = iter_values(vm, &a[1])?;
            with_vec!(vm, a[0], |l: &mut Vec<Value>| l.extend(items))?;
            Ok(Value::Null)
        },
        "System.Collections.Generic.List`1::Clear" | "System.Collections.Generic.Dictionary`2::Clear" => |vm, a| {
            let t = vm.deref(a[0].clone());
            if let Value::Obj(o) = &t {
                match &mut *o.x.borrow_mut() {
                    Ext::List(l) => l.clear(),
                    Ext::Dict(d) => d.clear(),
                    _ => {}
                }
            }
            Ok(Value::Null)
        },
        "System.Collections.Generic.List`1::RemoveAt" => |vm, a| {
            let i = a[1].as_i32();
            let ok = with_vec!(vm, a[0], |l: &mut Vec<Value>| if i >= 0 && (i as usize) < l.len() { l.remove(i as usize); true } else { false })?;
            if ok { Ok(Value::Null) } else { vm.throw("System.ArgumentOutOfRangeException", "Index was out of range.") }
        },
        "System.Collections.Generic.List`1::Remove" => |vm, a| {
            let v = vm.deref(a[1].clone());
            let r = with_vec!(vm, a[0], |l: &mut Vec<Value>| match l.iter().position(|x| crate::vm::values_eq(x, &v)) { Some(i) => { l.remove(i); true } None => false })?;
            Ok(Value::bool(r))
        },
        "System.Collections.Generic.List`1::Contains" => |vm, a| {
            let v = vm.deref(a[1].clone());
            let r = with_vec!(vm, a[0], |l: &mut Vec<Value>| l.iter().any(|x| crate::vm::values_eq(x, &v)))?;
            Ok(Value::bool(r))
        },
        "System.Collections.Generic.List`1::IndexOf" => |vm, a| {
            let v = vm.deref(a[1].clone());
            let r = with_vec!(vm, a[0], |l: &mut Vec<Value>| l.iter().position(|x| crate::vm::values_eq(x, &v)).map(|i| i as i32).unwrap_or(-1))?;
            Ok(Value::I32(r))
        },
        "System.Collections.Generic.List`1::ToArray" => |vm, a| {
            let items = iter_values(vm, &a[0])?;
            Ok(new_array(vm, items))
        },
        "System.Collections.Generic.List`1::GetEnumerator" | "System.Collections.Generic.Dictionary`2::GetEnumerator" => |vm, a| {
            let items = iter_values(vm, &a[0])?;
            Ok(new_enum(vm, items))
        },
        "System.Collections.IEnumerator::MoveNext" => |vm, a| {
            let e = vm.deref(a[0].clone());
            if let Value::Obj(o) = &e {
                if let Ext::Native(b) = &mut *o.x.borrow_mut() {
                    if let Some(n) = b.downcast_mut::<NatEnum>() {
                        n.idx += 1;
                        return Ok(Value::bool((n.idx as usize) < n.items.len()));
                    }
                }
            }
            Ok(Value::bool(false))
        },
        "System.Collections.IEnumerator::get_Current" => |vm, a| {
            let e = vm.deref(a[0].clone());
            if let Value::Obj(o) = &e {
                if let Ext::Native(b) = &*o.x.borrow() {
                    if let Some(n) = b.downcast_ref::<NatEnum>() {
                        return Ok(n.items.get(n.idx as usize).cloned().unwrap_or(Value::Null));
                    }
                }
            }
            Ok(Value::Null)
        },
        "System.Collections.IEnumerator::Reset" => |_vm, _a| Ok(Value::Null),
        // ---------------------------------------------------------- Dictionary
        "System.Collections.Generic.Dictionary`2::.ctor" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                *o.x.borrow_mut() = Ext::Dict(Dict::default());
            }
            Ok(Value::Null)
        },
        "System.Collections.Generic.Dictionary`2::get_Item" => |vm, a| {
            let k = vm.deref(a[1].clone());
            let t = vm.deref(a[0].clone());
            if let Value::Obj(o) = &t {
                if let Ext::Dict(d) = &*o.x.borrow() {
                    if let Some(v) = d.get(&k) {
                        return Ok(v.clone());
                    }
                    drop(d);
                }
            }
            vm.throw("System.Collections.Generic.KeyNotFoundException", "The given key was not present in the dictionary.")
        },
        "System.Collections.Generic.Dictionary`2::set_Item" | "System.Collections.Generic.Dictionary`2::Add" => |vm, a| {
            let k = vm.deref(a[1].clone());
            let t = vm.deref(a[0].clone());
            if let Value::Obj(o) = &t {
                if let Ext::Dict(d) = &mut *o.x.borrow_mut() {
                    d.set(k, a[2].clone());
                    return Ok(Value::Null);
                }
            }
            vm.throw("System.NullReferenceException", "dict")
        },
        "System.Collections.Generic.Dictionary`2::ContainsKey" => |vm, a| {
            let k = vm.deref(a[1].clone());
            let t = vm.deref(a[0].clone());
            if let Value::Obj(o) = &t {
                if let Ext::Dict(d) = &*o.x.borrow() {
                    return Ok(Value::bool(d.get(&k).is_some()));
                }
            }
            Ok(Value::bool(false))
        },
        "System.Collections.Generic.Dictionary`2::TryGetValue" => |vm, a| {
            let k = vm.deref(a[1].clone());
            let t = vm.deref(a[0].clone());
            let mut found = None;
            if let Value::Obj(o) = &t {
                if let Ext::Dict(d) = &*o.x.borrow() {
                    found = d.get(&k).cloned();
                }
            }
            let ok = found.is_some();
            if let Value::Ptr(p) = &a[2] {
                let v = found.unwrap_or(Value::Null);
                vm.store_ptr(p, v);
            }
            Ok(Value::bool(ok))
        },
        "System.Collections.Generic.Dictionary`2::Remove" => |vm, a| {
            let k = vm.deref(a[1].clone());
            let t = vm.deref(a[0].clone());
            if let Value::Obj(o) = &t {
                if let Ext::Dict(d) = &mut *o.x.borrow_mut() {
                    return Ok(Value::bool(d.remove(&k)));
                }
            }
            Ok(Value::bool(false))
        },
        "System.Collections.Generic.Dictionary`2::get_Keys" => |vm, a| {
            let t = vm.deref(a[0].clone());
            let mut ks = vec![];
            if let Value::Obj(o) = &t {
                if let Ext::Dict(d) = &*o.x.borrow() {
                    ks = d.iter().map(|(k, _)| k.clone()).collect();
                }
            }
            Ok(new_list(vm, ks))
        },
        "System.Collections.Generic.Dictionary`2::get_Values" => |vm, a| {
            let t = vm.deref(a[0].clone());
            let mut ks = vec![];
            if let Value::Obj(o) = &t {
                if let Ext::Dict(d) = &*o.x.borrow() {
                    ks = d.iter().map(|(_, v)| v.clone()).collect();
                }
            }
            Ok(new_list(vm, ks))
        },
        "System.Collections.Generic.KeyValuePair`2::get_Key" => |vm, a| {
            let t = vm.deref(a[0].clone());
            Ok(if let Value::Struct(s) = &t { s.f[0].clone() } else { Value::Null })
        },
        "System.Collections.Generic.KeyValuePair`2::get_Value" => |vm, a| {
            let t = vm.deref(a[0].clone());
            Ok(if let Value::Struct(s) = &t { s.f[1].clone() } else { Value::Null })
        },
        // ---------------------------------------------------------- StringBuilder
        "System.Text.StringBuilder::.ctor" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                *o.x.borrow_mut() = Ext::SB(match a.get(1) { Some(Value::Str(s)) => s.to_string(), _ => String::new() });
            }
            Ok(Value::Null)
        },
        "System.Text.StringBuilder::Append" => |vm, a| {
            let s = to_string(vm, &a[1])?;
            if let Value::Obj(o) = &a[0] {
                if let Ext::SB(b) = &mut *o.x.borrow_mut() {
                    b.push_str(&s);
                }
            }
            Ok(a[0].clone())
        },
        "System.Text.StringBuilder::ToString" => |vm, a| {
            let t = vm.deref(a[0].clone());
            if let Value::Obj(o) = &t {
                if let Ext::SB(b) = &*o.x.borrow() {
                    return Ok(Value::str(b));
                }
            }
            Ok(Value::str(""))
        },
        // ---------------------------------------------------------- Delegate
        "System.Delegate::Combine" | "System.MulticastDelegate::Combine" => |vm, a| {
            let mut out = vec![];
            for v in a.iter() {
                if let Value::Obj(o) = v {
                    if let Ext::Delegate(l) = &*o.x.borrow() {
                        out.extend(l.iter().cloned());
                    }
                }
            }
            if out.is_empty() {
                return Ok(Value::Null);
            }
            let id = vm.type_id("System.Delegate");
            Ok(Value::Obj(Obj::new(id, 0, Ext::Delegate(out))))
        },
        "System.Delegate::Remove" | "System.MulticastDelegate::Remove" => |vm, a| {
            let (mut src, rem) = (vec![], vec![]);
            let mut rem = rem;
            if let Value::Obj(o) = &a[0] { if let Ext::Delegate(l) = &*o.x.borrow() { src = l.clone(); } }
            if let Value::Obj(o) = &a[1] { if let Ext::Delegate(l) = &*o.x.borrow() { rem = l.clone(); } }
            for r in &rem {
                if let Some(p) = src.iter().rposition(|d| d.method == r.method && crate::vm::values_eq(&d.target, &r.target)) {
                    src.remove(p);
                }
            }
            if src.is_empty() {
                return Ok(Value::Null);
            }
            let id = vm.type_id("System.Delegate");
            Ok(Value::Obj(Obj::new(id, 0, Ext::Delegate(src))))
        },
        // ---------------------------------------------------------- DateTime / TimeSpan
        "System.DateTime::get_Now" | "System.DateTime::get_UtcNow" => |vm, _a| {
            let d = match std::env::var("NGU_FIXED_NOW").ok().and_then(|v| v.parse::<u64>().ok()) {
                Some(secs) => std::time::Duration::from_secs(secs),
                None => std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default(),
            };
            let ticks = 621_355_968_000_000_000i64 + (d.as_nanos() / 100) as i64;
            let id = vm.type_id("System.DateTime");
            Ok(Value::Struct(Box::new(Struct { ty: id, f: vec![Value::I64(ticks)] })))
        },
        "System.DateTime::.ctor" => |vm, a| {
            let g = |i: usize| a.get(i).map(|v| v.as_i64()).unwrap_or(0);
            let ticks = if a.len() == 1 {
                g(0)
            } else {
                // (year, month, day [, h, min, s [, kind]])
                let days = days_from_civil(g(0), g(1), g(2));
                (719_162 + days) * 864_000_000_000 + ((g(3) * 60 + g(4)) * 60 + g(5)) * 10_000_000
            };
            let id = vm.type_id("System.DateTime");
            Ok(Value::Struct(Box::new(Struct { ty: id, f: vec![Value::I64(ticks)] })))
        },
        "System.DateTime::ToString" => |vm, a| {
            let t = vm.deref(a[0].clone());
            let ticks = if let Value::Struct(s) = &t { s.f[0].as_i64() } else { 0 };
            let secs = ticks / 10_000_000 - 62_135_596_800;
            let days = secs.div_euclid(86_400);
            let rem = secs.rem_euclid(86_400);
            let (y, m, d) = civil_from_days(days);
            let (hh, mi, ss) = (rem / 3600, rem % 3600 / 60, rem % 60);
            const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
            let fmt = match a.get(1) { Some(Value::Str(s)) => s.to_string(), _ => "MM/dd/yyyy HH:mm:ss".to_string() };
            let mut out = String::new();
            let cs: Vec<char> = fmt.chars().collect();
            let mut i = 0;
            while i < cs.len() {
                let run = cs[i..].iter().take_while(|c| **c == cs[i]).count();
                match (cs[i], run) {
                    ('y', 4) => out.push_str(&format!("{:04}", y)),
                    ('y', _) => out.push_str(&format!("{:02}", y % 100)),
                    ('M', 4) => out.push_str(MONTHS[(m - 1) as usize]),
                    ('M', 3) => out.push_str(&MONTHS[(m - 1) as usize][..3]),
                    ('M', _) => out.push_str(&format!("{:02}", m)),
                    ('d', _) => out.push_str(&format!("{:02}", d)),
                    ('H', _) => out.push_str(&format!("{:02}", hh)),
                    ('m', _) => out.push_str(&format!("{:02}", mi)),
                    ('s', _) => out.push_str(&format!("{:02}", ss)),
                    (c, n) => out.push_str(&c.to_string().repeat(n)),
                }
                i += run;
            }
            Ok(Value::str(&out))
        },
        "System.DateTime::get_Ticks" => |vm, a| {
            let t = vm.deref(a[0].clone());
            Ok(if let Value::Struct(s) = &t { s.f[0].clone() } else { Value::I64(0) })
        },
        "System.DateTime::op_Subtraction" => |vm, a| {
            let (x, y) = (vm.deref(a[0].clone()), vm.deref(a[1].clone()));
            let tk = |v: &Value| if let Value::Struct(s) = v { s.f[0].as_i64() } else { 0 };
            let id = vm.type_id("System.TimeSpan");
            Ok(Value::Struct(Box::new(Struct { ty: id, f: vec![Value::I64(tk(&x) - tk(&y))] })))
        },
        "System.TimeSpan::get_TotalSeconds" => |vm, a| {
            let t = vm.deref(a[0].clone());
            Ok(Value::F64(if let Value::Struct(s) = &t { s.f[0].as_i64() as f64 / 1e7 } else { 0.0 }))
        },
        "System.DateTime::get_Hour" => |vm, a| {
            let t = vm.deref(a[0].clone());
            let tk = if let Value::Struct(s) = &t { s.f[0].as_i64() } else { 0 };
            Ok(Value::I32(((tk / 36_000_000_000) % 24) as i32))
        },
        // ---------------------------------------------------------- System.Random
        "System.Random::.ctor" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                let seed = a.get(1).map(|v| v.as_i32() as u64).unwrap_or(12345);
                *o.x.borrow_mut() = Ext::Native(Box::new(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)));
            }
            Ok(Value::Null)
        },
        "System.Random::Next" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                if let Ext::Native(b) = &mut *o.x.borrow_mut() {
                    if let Some(s) = b.downcast_mut::<u64>() {
                        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        let r = (*s >> 33) as i64;
                        return Ok(Value::I32(match a.len() {
                            2 => (r % a[1].as_i32().max(1) as i64) as i32,
                            3 => a[1].as_i32() + (r % (a[2].as_i32() - a[1].as_i32()).max(1) as i64) as i32,
                            _ => r as i32,
                        }));
                    }
                }
            }
            Ok(Value::I32(0))
        },
        // ---------------------------------------------------------- IO
        "System.IO.File::Exists" => |_vm, a| Ok(Value::bool(std::path::Path::new(&st(&a[0])).is_file())),
        "System.IO.File::ReadAllText" => |vm, a| match std::fs::read_to_string(st(&a[0])) {
            Ok(s) => Ok(Value::str(&s)),
            Err(_) => vm.throw("System.IO.FileNotFoundException", &st(&a[0])),
        },
        "System.IO.File::WriteAllText" => |vm, a| {
            let filename = st(&a[0]);
            let path = std::path::Path::new(&filename);
            let data = st(&a[1]);
            if std::fs::write(path, data.as_bytes()).is_ok() {
                crate::save_slots::autosave_written(vm, path, data.as_bytes());
            }
            Ok(Value::Null)
        },
        "System.IO.File::WriteAllBytes" => |vm, a| {
            let b: Vec<u8> = iter_values(vm, &a[1])?.iter().map(|v| v.as_i32() as u8).collect();
            let _ = std::fs::write(st(&a[0]), b);
            Ok(Value::Null)
        },
        "System.IO.Path::GetDirectoryName" => |_vm, a| Ok(Value::str(std::path::Path::new(&st(&a[0])).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default().as_str())),
        "System.IO.Path::GetFullPath" => |_vm, a| Ok(a[0].clone()),
        "System.IO.Path::GetPathRoot" => |_vm, _a| Ok(Value::str("/")),
        "System.Text.Encoding::get_UTF8" | "System.Text.Encoding::get_ASCII" | "System.Text.Encoding::get_Default" => |vm, _a| {
            let id = vm.type_id("System.Text.Encoding");
            Ok(Value::Obj(Obj::new(id, 0, Ext::None)))
        },
        "System.Text.Encoding::GetBytes" => |vm, a| {
            let b = st(&a[1]).bytes().map(|x| Value::I32(x as i32)).collect();
            Ok(new_array(vm, b))
        },
        "System.Text.Encoding::GetString" => |vm, a| {
            let b: Vec<u8> = iter_values(vm, &a[1])?.iter().map(|v| v.as_i32() as u8).collect();
            Ok(Value::str(&String::from_utf8_lossy(&b)))
        },
        // ---------------------------------------------------------- Enum
        "System.Enum::GetNames" => |vm, a| {
            let m = enum_members(vm, &a[0]);
            let v = m.into_iter().map(|(n, _)| Value::str(&n)).collect();
            Ok(new_array(vm, v))
        },
        "System.Enum::GetValues" => |vm, a| {
            let tid = enum_tid(vm, &a[0]);
            let m = enum_members(vm, &a[0]);
            let v = m
                .into_iter()
                .map(|(_, x)| match tid {
                    Some(t) => Value::Obj(Obj::new(t, 0, Ext::Boxed(Value::I32(x as i32)))),
                    None => Value::I32(x as i32),
                })
                .collect();
            Ok(new_array(vm, v))
        },
        _ => return crate::bcl_sys::lookup(key),
    };
    Some(f)
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
