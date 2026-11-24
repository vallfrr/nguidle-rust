use super::*;

pub(super) fn lookup(key: &str) -> Option<NativeFn> {
    let function: NativeFn = match key {
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
        _ => return None,
    };
    Some(function)
}
