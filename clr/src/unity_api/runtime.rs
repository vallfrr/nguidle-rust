use super::*;

pub(super) fn lookup(key: &str) -> Option<NativeFn> {
    let function: NativeFn = match key {
        // ------------------------------------------------------- Debug / Application / Screen
        "UnityEngine.Debug::Log" | "UnityEngine.Debug::LogWarning" | "UnityEngine.Debug::LogError" => |vm, a| {
            let s = to_string(vm, &a[0])?;
            crate::elog!("[Debug] {}", s);
            Ok(Value::Null)
        },
        "UnityEngine.Application::get_platform" => |_vm, _a| Ok(Value::I32(2)),
        "UnityEngine.Application::get_persistentDataPath" => |vm, _a| Ok(Value::str(&vm.eng.save_dir.clone())),
        "UnityEngine.Application::get_dataPath" => |vm, _a| Ok(Value::str(&vm.eng.assets_dir.clone())),
        "UnityEngine.Application::OpenURL" | "UnityEngine.Application::Quit" | "UnityEngine.Application::ExternalCall" | "UnityEngine.Application::ExternalEval" => stub0!(),
        "UnityEngine.Screen::get_width" => |vm, _a| Ok(Value::I32(vm.eng.screen.0)),
        "UnityEngine.Screen::get_height" => |vm, _a| Ok(Value::I32(vm.eng.screen.1)),
        "UnityEngine.QualitySettings::set_vSyncCount" => stub0!(),
        "UnityEngine.Screen::SetResolution" => |vm, a| {
            // the shell resizes its window (ignored during the first frames: the game calls it at start-up with 960x600)
            vm.eng.resolution_req = Some((a[0].as_i32(), a[1].as_i32(), a.get(2).map(|v| v.as_i32() != 0).unwrap_or(false), vm.eng.frame));
            Ok(Value::Null)
        },
        "UnityEngine.Screen::get_currentResolution" => |vm, _a| {
            let (w, h) = vm.eng.screen;
            Ok(mk(vm, "UnityEngine.Resolution", vec![Value::I32(w), Value::I32(h), Value::I32(60)]))
        },
        "UnityEngine.Resolution::get_height" => |vm, a| Ok(Value::I32(rv(vm, &a[0]).as_struct_i(1))),
        "UnityEngine.Resolution::get_width" => |vm, a| Ok(Value::I32(rv(vm, &a[0]).as_struct_i(0))),
        "UnityEngine.Display::get_main" => |vm, _a| {
            let id = vm.type_id("UnityEngine.Display");
            Ok(Value::Obj(Obj::new(id, 0, Ext::None)))
        },
        "UnityEngine.Display::get_systemWidth" => |vm, _a| Ok(Value::I32(vm.eng.screen.0)),
        "UnityEngine.Display::get_systemHeight" => |vm, _a| Ok(Value::I32(vm.eng.screen.1)),
        // ------------------------------------------------------- Time / Random
        "UnityEngine.Time::get_deltaTime" => |vm, _a| Ok(Value::F32(vm.eng.dt)),
        "UnityEngine.Time::get_time" => |vm, _a| Ok(Value::F32(vm.eng.time as f32)),
        "UnityEngine.Random::get_value" => |vm, _a| Ok(Value::F32((unity_rand(vm) & 0x7FFFFF) as f32 / 8388607.0)),
        "UnityEngine.Random::Range" => |vm, a| {
            let r = unity_rand(vm);
            Ok(match (&a[0], &a[1]) {
                (Value::I32(lo), Value::I32(hi)) => {
                    Value::I32(unity_range_int(r, *lo, *hi))
                }
                _ => {
                    let (lo, hi) = (a[0].as_f32(), a[1].as_f32());
                    Value::F32(unity_range_float(r, lo, hi))
                }
            })
        },
        "UnityEngine.Random::InitState" => |vm, a| {
            init_state(vm, a[0].as_i32());
            Ok(Value::Null)
        },
        "UnityEngine.Random::get_state" => |vm, _a| {
            let s = vm.eng.rng;
            Ok(mk(vm, "UnityEngine.Random/State", s.iter().map(|x| Value::I32(*x as i32)).collect()))
        },
        "UnityEngine.Random::set_state" => |vm, a| {
            let v = rv(vm, &a[0]);
            if let Value::Struct(s) = &v {
                for i in 0..4 {
                    vm.eng.rng[i] = s.f[i].as_i32() as u32;
                }
            }
            Ok(Value::Null)
        },
        // ------------------------------------------------------- Color / Vector
        "UnityEngine.Color::.ctor" => |vm, a| {
            let al = if a.len() > 3 { a[3].as_f32() } else { 1.0 };
            Ok(col(vm, [a[0].as_f32(), a[1].as_f32(), a[2].as_f32(), al]))
        },
        "UnityEngine.Color32::.ctor" => |vm, a| Ok(mk(vm, "UnityEngine.Color32", a.iter().map(|x| Value::I32(x.as_i32())).collect())),
        "UnityEngine.Color32::op_Implicit" => |vm, a| {
            let v = rv(vm, &a[0]);
            if let Value::Struct(s) = &v {
                let n = vm.types[s.ty as usize].name.clone();
                if n == "UnityEngine.Color32" {
                    let c = [s.f[0].as_f32() / 255.0, s.f[1].as_f32() / 255.0, s.f[2].as_f32() / 255.0, s.f[3].as_f32() / 255.0];
                    return Ok(col(vm, c));
                }
            }
            Ok(v)
        },
        "UnityEngine.Color::get_white" => |vm, _a| Ok(col(vm, [1.0, 1.0, 1.0, 1.0])),
        "UnityEngine.Color::get_black" => |vm, _a| Ok(col(vm, [0.0, 0.0, 0.0, 1.0])),
        "UnityEngine.Color::get_clear" => |vm, _a| Ok(col(vm, [0.0, 0.0, 0.0, 0.0])),
        "UnityEngine.Color::get_red" => |vm, _a| Ok(col(vm, [1.0, 0.0, 0.0, 1.0])),
        "UnityEngine.Color::get_green" => |vm, _a| Ok(col(vm, [0.0, 1.0, 0.0, 1.0])),
        "UnityEngine.Color::get_blue" => |vm, _a| Ok(col(vm, [0.0, 0.0, 1.0, 1.0])),
        "UnityEngine.Color::get_yellow" => |vm, _a| Ok(col(vm, [1.0, 0.92156863, 0.015686275, 1.0])),
        "UnityEngine.Color::get_cyan" => |vm, _a| Ok(col(vm, [0.0, 1.0, 1.0, 1.0])),
        "UnityEngine.Color::get_magenta" => |vm, _a| Ok(col(vm, [1.0, 0.0, 1.0, 1.0])),
        "UnityEngine.Color::get_gray" | "UnityEngine.Color::get_grey" => |vm, _a| Ok(col(vm, [0.5, 0.5, 0.5, 1.0])),
        "UnityEngine.Color::Lerp" => |vm, a| {
            let (x, y) = (rv(vm, &a[0]), rv(vm, &a[1]));
            let t = a[2].as_f32().clamp(0.0, 1.0);
            let c = [0, 1, 2, 3].map(|i| sf(&x, i) + (sf(&y, i) - sf(&x, i)) * t);
            Ok(col(vm, c))
        },
        "UnityEngine.Color::op_Addition" => |vm, a| {
            let (x, y) = (rv(vm, &a[0]), rv(vm, &a[1]));
            let c = [0, 1, 2, 3].map(|i| sf(&x, i) + sf(&y, i));
            Ok(col(vm, c))
        },
        "UnityEngine.Color::op_Multiply" => |vm, a| {
            let x = rv(vm, &a[0]);
            let c = match &a[1] {
                Value::Struct(y) => [0, 1, 2, 3].map(|i| sf(&x, i) * y.f[i].as_f32()),
                o => [0, 1, 2, 3].map(|i| sf(&x, i) * o.as_f32()),
            };
            Ok(col(vm, c))
        },
        "UnityEngine.Color::op_Inequality" => |vm, a| {
            let (x, y) = (rv(vm, &a[0]), rv(vm, &a[1]));
            Ok(Value::bool((0..4).any(|i| (sf(&x, i) - sf(&y, i)).abs() > 1e-5)))
        },
        "UnityEngine.Vector2::.ctor" => |vm, a| Ok(vec2(vm, [a[0].as_f32(), a[1].as_f32()])),
        "UnityEngine.Vector3::.ctor" => |vm, a| Ok(vec3(vm, [a[0].as_f32(), a[1].as_f32(), if a.len() > 2 { a[2].as_f32() } else { 0.0 }])),
        "UnityEngine.Vector3::get_zero" => |vm, _a| Ok(vec3(vm, [0.0; 3])),
        "UnityEngine.Vector3::op_Equality" | "UnityEngine.Vector3::op_Inequality" => |vm, a| {
            let (x, y) = (rv(vm, &a[0]), rv(vm, &a[1]));
            Ok(Value::bool((0..3).all(|i| (sf(&x, i) - sf(&y, i)).abs() < 1e-5)))
        },
        "UnityEngine.Vector2::op_Implicit" => |vm, a| {
            let v = rv(vm, &a[0]);
            let n = if let Value::Struct(s) = &v { vm.types[s.ty as usize].name.clone() } else { String::new() };
            Ok(if n == "UnityEngine.Vector2" { vec3(vm, [sf(&v, 0), sf(&v, 1), 0.0]) } else { vec2(vm, [sf(&v, 0), sf(&v, 1)]) })
        },
        "UnityEngine.Quaternion::get_identity" => |vm, _a| Ok(mk(vm, "UnityEngine.Quaternion", vec![Value::F32(0.0), Value::F32(0.0), Value::F32(0.0), Value::F32(1.0)])),
        "UnityEngine.Rect::.ctor" => |vm, a| Ok(rect_val(vm, [a[0].as_f32(), a[1].as_f32(), a[2].as_f32(), a[3].as_f32()])),
        "UnityEngine.Rect::get_width" => |vm, a| Ok(Value::F32(sf(&rv(vm, &a[0]), 2))),
        "UnityEngine.Rect::get_height" => |vm, a| Ok(Value::F32(sf(&rv(vm, &a[0]), 3))),
        "UnityEngine.Rect::get_min" => |vm, a| {
            let r = rv(vm, &a[0]);
            Ok(vec2(vm, [sf(&r, 0), sf(&r, 1)]))
        },
        "UnityEngine.Rect::get_max" => |vm, a| {
            let r = rv(vm, &a[0]);
            Ok(vec2(vm, [sf(&r, 0) + sf(&r, 2), sf(&r, 1) + sf(&r, 3)]))
        },
        "UnityEngine.Rect::get_size" => |vm, a| {
            let r = rv(vm, &a[0]);
            Ok(vec2(vm, [sf(&r, 2), sf(&r, 3)]))
        },
        "UnityEngine.Rect::Contains" => |vm, a| {
            let (r, p) = (rv(vm, &a[0]), rv(vm, &a[1]));
            let (x, y) = (sf(&p, 0), sf(&p, 1));
            Ok(Value::bool(x >= sf(&r, 0) && x < sf(&r, 0) + sf(&r, 2) && y >= sf(&r, 1) && y < sf(&r, 1) + sf(&r, 3)))
        },
        _ => return None,
    };
    Some(function)
}
