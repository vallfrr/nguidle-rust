use super::*;

pub(super) fn lookup(key: &str) -> Option<NativeFn> {
    let function: NativeFn = match key {
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
        _ => return None,
    };
    Some(function)
}
