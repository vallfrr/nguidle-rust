use super::*;

pub(super) fn lookup(key: &str) -> Option<NativeFn> {
    let function: NativeFn = match key {
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
        // The interpreter executes managed code on one owner thread.
        "System.Environment::get_CurrentManagedThreadId" => |_vm, _a| Ok(Value::I32(1)),
        // ---------------------------------------------------------- System.Random
        "System.Random::.ctor" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                let seed = a.get(1).map(|v| v.as_i32()).unwrap_or(12345);
                *o.x.borrow_mut() = Ext::Native(Box::new(crate::dotnet_random::DotNetRandom::new(seed)));
            }
            Ok(Value::Null)
        },
        "System.Random::Next" => |vm, a| {
            let bounds = match a.len() {
                2 => Some((0, a[1].as_i32())),
                3 => Some((a[1].as_i32(), a[2].as_i32())),
                _ => None,
            };
            if bounds.is_some_and(|(min, max)| min > max) {
                return vm.throw("System.ArgumentOutOfRangeException", "minValue must not exceed maxValue");
            }
            if let Value::Obj(o) = &a[0] {
                if let Ext::Native(b) = &mut *o.x.borrow_mut() {
                    if let Some(random) = b.downcast_mut::<crate::dotnet_random::DotNetRandom>() {
                        return Ok(Value::I32(match bounds {
                            Some((min, max)) => random.range(min, max),
                            None => random.next(),
                        }));
                    }
                }
            }
            Ok(Value::I32(0))
        },
        "System.Random::NextDouble" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                if let Ext::Native(b) = &mut *o.x.borrow_mut() {
                    if let Some(random) = b.downcast_mut::<crate::dotnet_random::DotNetRandom>() {
                        return Ok(Value::F64(random.sample()));
                    }
                }
            }
            Ok(Value::F64(0.0))
        },
        // ---------------------------------------------------------- IO
        "System.IO.File::Exists" => |vm, a| {
            let filename = st(&a[0]);
            let path = std::path::Path::new(&filename);
            crate::save_slots::before_managed_read(vm, path);
            Ok(Value::bool(path.is_file()))
        },
        "System.IO.File::ReadAllText" => |vm, a| {
            let filename = st(&a[0]);
            let path = std::path::Path::new(&filename);
            crate::save_slots::before_managed_read(vm, path);
            match std::fs::read_to_string(path) {
                Ok(s) => Ok(Value::str(&s)),
                Err(_) => vm.throw("System.IO.FileNotFoundException", &filename),
            }
        },
        "System.IO.File::WriteAllText" => |vm, a| {
            crate::save_slots::write_managed_file(vm, st(&a[0]), st(&a[1]));
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
        _ => return None,
    };
    Some(function)
}
