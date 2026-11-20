//! UnityEngine.* / UnityEngine.UI.* / Steam / Kartridge intrinsics (stubs).
use crate::bcl_core::{iter_values, new_array, to_string};
use crate::code::NativeFn;
use crate::unity::*;
use crate::value::*;
use crate::Vm;
use std::rc::Rc;

fn rv(vm: &mut Vm, v: &Value) -> Value {
    vm.deref(v.clone())
}

fn sf(v: &Value, i: usize) -> f32 {
    if let Value::Struct(s) = v {
        s.f.get(i).map(|x| x.as_f32()).unwrap_or(0.0)
    } else {
        0.0
    }
}

pub fn mk(vm: &mut Vm, name: &str, f: Vec<Value>) -> Value {
    let id = vm.type_id(name);
    Value::Struct(Box::new(Struct { ty: id, f }))
}

pub fn col(vm: &mut Vm, c: Col) -> Value {
    mk(vm, "UnityEngine.Color", c.iter().map(|x| Value::F32(*x)).collect())
}

pub fn vec3(vm: &mut Vm, v: [f32; 3]) -> Value {
    mk(vm, "UnityEngine.Vector3", v.iter().map(|x| Value::F32(*x)).collect())
}

pub fn vec2(vm: &mut Vm, v: [f32; 2]) -> Value {
    mk(vm, "UnityEngine.Vector2", v.iter().map(|x| Value::F32(*x)).collect())
}

pub fn to_col(v: &Value) -> Col {
    [sf(v, 0), sf(v, 1), sf(v, 2), sf(v, 3)]
}

fn rect_val(vm: &mut Vm, r: [f32; 4]) -> Value {
    mk(vm, "UnityEngine.Rect", r.iter().map(|x| Value::F32(*x)).collect())
}

fn this_go(v: &Value) -> Value {
    go_of(v)
}

fn comp_matches(vm: &mut Vm, c: &Value, t: &crate::code::TyOp) -> bool {
    vm.is_inst(c, t)
}

fn target_tyop(vm: &mut Vm, a: &[Value]) -> Option<crate::code::TyOp> {
    if let Some(t) = vm.targs.clone() {
        if let Some(t0) = t.first() {
            return Some(vm.tyop(t0));
        }
    }
    // GetComponent(Type)
    if let Some(Value::Obj(o)) = a.get(1) {
        if let Ext::Native(b) = &*o.x.borrow() {
            if let Some(n) = b.downcast_ref::<String>() {
                let n = n.clone();
                drop(b);
                let id = vm.type_id(&n);
                return Some(crate::code::TyOp::Class(id));
            }
        }
    }
    None
}

fn collect_comps(vm: &mut Vm, root: &Value, t: &crate::code::TyOp, children: bool, only_first: bool, include_inactive: bool) -> Vec<Value> {
    let mut out = vec![];
    let go = this_go(root);
    let mut stack = vec![go];
    while let Some(g) = stack.pop() {
        if !include_inactive && !with_go(&g, |x| x.active).unwrap_or(false) {
            continue;
        }
        let comps = with_go(&g, |x| x.comps.clone()).unwrap_or_default();
        for c in comps {
            if comp_matches(vm, &c, t) {
                out.push(c);
                if only_first {
                    return out;
                }
            }
        }
        if !children {
            break;
        }
        let tr = with_go(&g, |x| x.transform.clone()).unwrap_or(Value::Null);
        let mut ch = with_tr(&tr, |t| t.children.clone()).unwrap_or_default();
        ch.reverse();
        for c in ch {
            stack.push(go_of(&c));
        }
    }
    out
}

fn unity_rand(vm: &mut Vm) -> u32 {
    let s = &mut vm.eng.rng;
    let t = s[0] ^ (s[0] << 11);
    s[0] = s[1];
    s[1] = s[2];
    s[2] = s[3];
    s[3] = (s[3] ^ (s[3] >> 19)) ^ (t ^ (t >> 8));
    s[3]
}

fn init_state(vm: &mut Vm, seed: i32) {
    let s0 = seed as u32;
    let s1 = s0.wrapping_mul(1812433253).wrapping_add(1);
    let s2 = s1.wrapping_mul(1812433253).wrapping_add(1);
    let s3 = s2.wrapping_mul(1812433253).wrapping_add(1);
    vm.eng.rng = [s0, s1, s2, s3];
}

fn sel<R>(v: &Value, f: impl FnOnce(&mut SelD) -> R) -> Option<R> {
    with_comp(v, |c| match &mut c.k {
        CK::Sel(s) => Some(f(s)),
        _ => None,
    })
    .flatten()
}

fn set_text_comp(v: &Value, s: &str) {
    let changed = with_comp(v, |c| match &mut c.k {
        CK::Text(t) => {
            if t.text != s {
                t.text = s.to_string();
                true
            } else {
                false
            }
        }
        _ => false,
    })
    .unwrap_or(false);
    if changed {
        mark_dirty(v);
        bump_layout(v);
    }
}

fn load_text(_vm: &Vm, key: &str) -> String {
    crate::assets::read(&format!("text/{}.bin", key.replace(':', "_"))).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default()
}

fn save_prefs(vm: &mut Vm) {
    let mut m = serde_json::Map::new();
    for (k, v) in &vm.eng.prefs {
        m.insert(
            k.clone(),
            match v {
                Value::I32(i) => serde_json::json!({"i": i}),
                Value::Str(s) => serde_json::json!({"s": &**s}),
                _ => serde_json::Value::Null,
            },
        );
    }
    if !vm.eng.save_dir.is_empty() {
        let _ = std::fs::create_dir_all(&vm.eng.save_dir);
        let _ = std::fs::write(format!("{}/prefs.json", vm.eng.save_dir), serde_json::Value::Object(m).to_string());
    }
}

pub fn load_prefs(vm: &mut Vm) {
    let p = format!("{}/prefs.json", vm.eng.save_dir);
    if let Ok(t) = std::fs::read_to_string(p) {
        if let Ok(serde_json::Value::Object(m)) = serde_json::from_str(&t) {
            for (k, v) in m {
                if let Some(i) = v.get("i").and_then(|x| x.as_i64()) {
                    vm.eng.prefs.insert(k, Value::I32(i as i32));
                } else if let Some(s) = v.get("s").and_then(|x| x.as_str()) {
                    vm.eng.prefs.insert(k, Value::str(s));
                }
            }
        }
    }
}

macro_rules! stub0 {
    () => {
        |_vm, _a| Ok(Value::Null)
    };
}

pub fn lookup(key: &str) -> Option<NativeFn> {
    let f: NativeFn = match key {
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
        "UnityEngine.Screen::SetResolution" | "UnityEngine.QualitySettings::set_vSyncCount" => stub0!(),
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
                    let span = (*hi as i64 - *lo as i64).max(0) as u32;
                    Value::I32(if span == 0 { *lo } else { lo.wrapping_add((r % span) as i32) })
                }
                _ => {
                    let (lo, hi) = (a[0].as_f32(), a[1].as_f32());
                    Value::F32(lo + (r & 0x7FFFFF) as f32 / 8388607.0 * (hi - lo))
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
        // ------------------------------------------------------- Object / GameObject / Component
        "UnityEngine.Object::op_Equality" => |vm, a| {
            let (x, y) = (alive(vm, &a[0]), alive(vm, &a[1]));
            Ok(Value::bool(match (&x, &y) {
                (Value::Null, Value::Null) => true,
                (Value::Obj(p), Value::Obj(q)) => Rc::ptr_eq(p, q),
                _ => false,
            }))
        },
        "UnityEngine.Object::op_Inequality" => |vm, a| {
            let (x, y) = (alive(vm, &a[0]), alive(vm, &a[1]));
            Ok(Value::bool(!match (&x, &y) {
                (Value::Null, Value::Null) => true,
                (Value::Obj(p), Value::Obj(q)) => Rc::ptr_eq(p, q),
                _ => false,
            }))
        },
        "UnityEngine.Object::get_name" => |_vm, a| {
            let g = go_of(&a[0]);
            Ok(with_go(&g, |g| Value::str(&g.name)).unwrap_or(Value::Null))
        },
        "UnityEngine.Object::Destroy" => |vm, a| {
            let g = go_of(&a[0]);
            if !g.is_null() && is_go(&a[0]) {
                mark_dirty(&g);
                bump_layout(&g);
                with_go(&g, |x| x.destroyed = true);
                vm.set_active(&g, false);
                let tr = with_go(&g, |x| x.transform.clone()).unwrap_or(Value::Null);
                let parent = with_tr(&tr, |t| t.parent.clone()).unwrap_or(Value::Null);
                with_tr(&parent, |t| t.children.retain(|c| !same(c, &tr)));
                vm.eng.gos.retain(|x| !same(x, &g));
            }
            Ok(Value::Null)
        },
        "UnityEngine.Object::DontDestroyOnLoad" => stub0!(),
        "UnityEngine.GameObject::.ctor" => |vm, a| {
            let go = a[0].clone();
            if let Value::Obj(o) = &go {
                let name = a.get(1).and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_else(|| "GameObject".into());
                let tr = new_comp(vm, "UnityEngine.Transform", go.clone(), CK::Transform(Box::new(Tr { parent: Value::Null, children: vec![], pos: [0.0; 3], scale: [1.0; 3], rot_z: 0.0, amin: [0.5; 2], amax: [0.5; 2], pivot: [0.5; 2], apos: [0.0; 2], size: [0.0; 2], is_rect: false })));
                *o.x.borrow_mut() = Ext::Native(Box::new(GoData { name, active: true, layer: 0, tag: "Untagged".into(), comps: vec![tr.clone()], transform: tr, destroyed: false, dirty: true, cache: None, in_layout: false }));
                vm.eng.gos.push(go.clone());
            }
            Ok(Value::Null)
        },
        "UnityEngine.GameObject::SetActive" => |vm, a| {
            let g = rv(vm, &a[0]);
            vm.set_active(&g, a[1].as_bool());
            Ok(Value::Null)
        },
        "UnityEngine.GameObject::get_transform" | "UnityEngine.Component::get_transform" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(transform_of(&v))
        },
        "UnityEngine.GameObject::get_gameObject" | "UnityEngine.Component::get_gameObject" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(go_of(&v))
        },
        "UnityEngine.Component::get_tag" => |vm, a| {
            let v = rv(vm, &a[0]);
            let g = go_of(&v);
            Ok(with_go(&g, |g| Value::str(&g.tag)).unwrap_or(Value::Null))
        },
        "UnityEngine.GameObject::FindGameObjectsWithTag" => |vm, a| {
            let tag = a[0].as_str().unwrap_or("").to_string();
            let gos = vm.eng.gos.clone();
            let r: Vec<Value> = gos.into_iter().filter(|g| with_go(g, |x| x.tag == tag && x.active).unwrap_or(false)).collect();
            Ok(new_array(vm, r))
        },
        "UnityEngine.Component::GetComponent" | "UnityEngine.GameObject::GetComponent" => |vm, a| {
            let v = rv(vm, &a[0]);
            match target_tyop(vm, a) {
                Some(t) => Ok(collect_comps(vm, &v, &t, false, true, true).into_iter().next().unwrap_or(Value::Null)),
                None => Ok(Value::Null),
            }
        },
        "UnityEngine.Component::GetComponentInChildren" | "UnityEngine.GameObject::GetComponentInChildren" => |vm, a| {
            let v = rv(vm, &a[0]);
            let inc = a.get(1).map(|x| x.as_bool() && matches!(x, Value::I32(_))).unwrap_or(false);
            match target_tyop(vm, &a[..1]) {
                Some(t) => Ok(collect_comps(vm, &v, &t, true, true, inc).into_iter().next().unwrap_or(Value::Null)),
                None => Ok(Value::Null),
            }
        },
        "UnityEngine.Component::GetComponentsInChildren" | "UnityEngine.GameObject::GetComponentsInChildren" => |vm, a| {
            let v = rv(vm, &a[0]);
            let inc = a.get(1).map(|x| matches!(x, Value::I32(1))).unwrap_or(false);
            match target_tyop(vm, &a[..1]) {
                Some(t) => {
                    let r = collect_comps(vm, &v, &t, true, false, inc);
                    Ok(new_array(vm, r))
                }
                None => Ok(new_array(vm, vec![])),
            }
        },
        "UnityEngine.GameObject::AddComponent" => |_vm, _a| Ok(Value::Null),
        "UnityEngine.Behaviour::set_enabled" => |vm, a| {
            let v = rv(vm, &a[0]);
            let on = a[1].as_bool();
            let was = with_comp(&v, |c| std::mem::replace(&mut c.enabled, on)).unwrap_or(on);
            if was != on {
                mark_dirty(&v);
                let _ = vm.call_msg(&v, if on { "OnEnable" } else { "OnDisable" });
            }
            Ok(Value::Null)
        },
        "UnityEngine.MonoBehaviour::StartCoroutine" => |vm, a| {
            let owner = rv(vm, &a[0]);
            Ok(vm.start_coroutine(&owner, a[1].clone()))
        },
        "UnityEngine.MonoBehaviour::Invoke" => |vm, a| {
            let owner = rv(vm, &a[0]);
            let t = vm.eng.time + a[2].as_f32() as f64;
            vm.eng.invokes.push(Inv { owner, method: a[1].as_str().unwrap_or("").into(), next: t, repeat: 0.0 });
            Ok(Value::Null)
        },
        "UnityEngine.MonoBehaviour::InvokeRepeating" => |vm, a| {
            let owner = rv(vm, &a[0]);
            let t = vm.eng.time + a[2].as_f32() as f64;
            vm.eng.invokes.push(Inv { owner, method: a[1].as_str().unwrap_or("").into(), next: t, repeat: a[3].as_f32().max(0.0001) as f64 });
            Ok(Value::Null)
        },
        "UnityEngine.MonoBehaviour::CancelInvoke" => |vm, a| {
            let owner = rv(vm, &a[0]);
            let name = a.get(1).and_then(|x| x.as_str().map(|s| s.to_string()));
            vm.eng.invokes.retain(|i| !(same(&i.owner, &owner) && name.as_ref().map(|n| *n == i.method).unwrap_or(true)));
            Ok(Value::Null)
        },
        "UnityEngine.WaitForSeconds::.ctor" => |_vm, a| {
            if let Value::Obj(o) = &a[0] {
                *o.x.borrow_mut() = Ext::Native(Box::new(Comp { go: Value::Null, enabled: true, awoken: true, started: true, k: CK::Wait(a[1].as_f32()) }));
            }
            Ok(Value::Null)
        },
        // ------------------------------------------------------- Transform
        "UnityEngine.Transform::get_position" => |vm, a| {
            let t = rv(vm, &a[0]);
            let p = crate::ui::world_pos(vm, &t);
            Ok(vec3(vm, p))
        },
        "UnityEngine.Transform::set_position" => |vm, a| {
            let t = rv(vm, &a[0]);
            let p = rv(vm, &a[1]);
            let cur = crate::ui::world_pos(vm, &t);
            let np = [sf(&p, 0), sf(&p, 1), sf(&p, 2)];
            if (cur[0] - np[0]).abs() > 1e-4 || (cur[1] - np[1]).abs() > 1e-4 || with_tr(&t, |x| !x.is_rect && (x.pos[2] - np[2]).abs() > 1e-4).unwrap_or(false) {
                crate::ui::set_world_pos(vm, &t, np);
                mark_dirty(&t);
            }
            Ok(Value::Null)
        },
        "UnityEngine.Transform::get_localPosition" => |vm, a| {
            let t = rv(vm, &a[0]);
            let p = crate::ui::local_pos(vm, &t);
            Ok(vec3(vm, p))
        },
        "UnityEngine.Transform::set_localPosition" => |vm, a| {
            let t = rv(vm, &a[0]);
            let p = rv(vm, &a[1]);
            let np = [sf(&p, 0), sf(&p, 1), sf(&p, 2)];
            let cur = crate::ui::local_pos(vm, &t);
            if cur != np {
                crate::ui::set_local_pos(vm, &t, np);
                mark_dirty(&t);
            }
            Ok(Value::Null)
        },
        "UnityEngine.Transform::Translate" => |vm, a| {
            let t = rv(vm, &a[0]);
            let p = rv(vm, &a[1]);
            let cur = crate::ui::world_pos(vm, &t);
            crate::ui::set_world_pos(vm, &t, [cur[0] + sf(&p, 0), cur[1] + sf(&p, 1), cur[2] + sf(&p, 2)]);
            mark_dirty(&t);
            Ok(Value::Null)
        },
        "UnityEngine.Transform::SetAsFirstSibling" | "UnityEngine.Transform::SetAsLastSibling" => |vm, a| {
            let t = rv(vm, &a[0]);
            let first = a.len() == 1 && false;
            let _ = first;
            let parent = with_tr(&t, |x| x.parent.clone()).unwrap_or(Value::Null);
            with_tr(&parent, |p| {
                if let Some(i) = p.children.iter().position(|c| same(c, &t)) {
                    let c = p.children.remove(i);
                    p.children.push(c);
                }
            });
            mark_dirty(&t);
            Ok(Value::Null)
        },
        "UnityEngine.Transform::GetChild" => |vm, a| {
            let t = rv(vm, &a[0]);
            Ok(with_tr(&t, |x| x.children.get(a[1].as_i32() as usize).cloned()).flatten().unwrap_or(Value::Null))
        },
        "UnityEngine.Transform::Find" => |vm, a| {
            let t = rv(vm, &a[0]);
            let path = a[1].as_str().unwrap_or("").to_string();
            let mut cur = t;
            for part in path.split('/') {
                let ch = with_tr(&cur, |x| x.children.clone()).unwrap_or_default();
                let mut found = Value::Null;
                for c in ch {
                    let g = go_of(&c);
                    if with_go(&g, |g| g.name == part).unwrap_or(false) {
                        found = c;
                        break;
                    }
                }
                if found.is_null() {
                    return Ok(Value::Null);
                }
                cur = found;
            }
            Ok(cur)
        },
        "UnityEngine.Transform::InverseTransformPoint" => |vm, a| {
            let t = rv(vm, &a[0]);
            let p = rv(vm, &a[1]);
            let w = crate::ui::world_pos(vm, &t);
            Ok(vec3(vm, [sf(&p, 0) - w[0], sf(&p, 1) - w[1], sf(&p, 2) - w[2]]))
        },
        "UnityEngine.RectTransform::get_rect" => |vm, a| {
            let t = rv(vm, &a[0]);
            let r = crate::ui::local_rect(vm, &t);
            Ok(rect_val(vm, r))
        },
        "UnityEngine.Canvas::get_scaleFactor" => |_vm, _a| Ok(Value::F32(1.0)),
        "UnityEngine.CanvasRenderer::SetAlpha" => stub0!(),
        // ------------------------------------------------------- Input
        "UnityEngine.Input::get_mousePosition" => |vm, _a| {
            let (x, y) = vm.eng.mouse;
            Ok(vec3(vm, [x, y, 0.0]))
        },
        "UnityEngine.Input::GetMouseButton" => |vm, a| Ok(Value::bool(vm.eng.mouse_down && a[0].as_i32() == vm.eng.mouse_button)),
        "UnityEngine.Input::GetMouseButtonDown" => |vm, a| Ok(Value::bool(vm.eng.mouse_down_edge && a[0].as_i32() == vm.eng.mouse_button)),
        "UnityEngine.Input::GetMouseButtonUp" => |vm, a| Ok(Value::bool(vm.eng.mouse_up_edge && a[0].as_i32() == vm.eng.mouse_button)),
        "UnityEngine.Input::GetKey" => |vm, a| {
            let k = key_arg(vm, &a[0]);
            Ok(Value::bool(vm.eng.keys_down.contains(&k)))
        },
        "UnityEngine.Input::GetKeyDown" => |vm, a| {
            let k = key_arg(vm, &a[0]);
            Ok(Value::bool(vm.eng.keys_edge.contains(&k)))
        },
        // ------------------------------------------------------- PlayerPrefs / Resources / TextAsset
        "UnityEngine.PlayerPrefs::GetInt" => |vm, a| {
            let k = a[0].as_str().unwrap_or("").to_string();
            Ok(match vm.eng.prefs.get(&k) {
                Some(Value::I32(i)) => Value::I32(*i),
                _ => Value::I32(a.get(1).map(|x| x.as_i32()).unwrap_or(0)),
            })
        },
        "UnityEngine.PlayerPrefs::GetString" => |vm, a| {
            let k = a[0].as_str().unwrap_or("").to_string();
            Ok(match vm.eng.prefs.get(&k) {
                Some(Value::Str(s)) => Value::Str(s.clone()),
                _ => a.get(1).cloned().unwrap_or_else(|| Value::str("")),
            })
        },
        "UnityEngine.PlayerPrefs::SetInt" | "UnityEngine.PlayerPrefs::SetString" => |vm, a| {
            let k = a[0].as_str().unwrap_or("").to_string();
            vm.eng.prefs.insert(k, a[1].clone());
            save_prefs(vm);
            Ok(Value::Null)
        },
        "UnityEngine.Resources::Load" => |vm, a| {
            let name = a[0].as_str().unwrap_or("").to_string();
            let tys: Vec<String> = vm.targs.clone().map(|t| t.iter().map(|x| crate::lower::ty_name(x, vm)).collect()).unwrap_or_default();
            let _ = tys;
            Ok(vm.eng.keys.get(&format!("res:{}", name)).cloned().unwrap_or(Value::Null))
        },
        "UnityEngine.TextAsset::get_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            let key = with_comp(&v, |c| if let CK::TextAsset { key, .. } = &c.k { key.clone() } else { String::new() }).unwrap_or_default();
            Ok(Value::str(&load_text(vm, &key)))
        },
        // ------------------------------------------------------- UI
        "UnityEngine.UI.Text::get_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(with_comp(&v, |c| if let CK::Text(t) = &c.k { Value::str(&t.text) } else { Value::Null }).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Text::set_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            let s = to_string(vm, &a[1])?;
            set_text_comp(&v, &s);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Text::set_fontSize" => |vm, a| {
            let v = rv(vm, &a[0]);
            with_comp(&v, |c| if let CK::Text(t) = &mut c.k { t.size = a[1].as_i32() });
            mark_dirty(&v);
            bump_layout(&v);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Text::set_alignment" => |vm, a| {
            let v = rv(vm, &a[0]);
            with_comp(&v, |c| if let CK::Text(t) = &mut c.k { t.align = a[1].as_i32() });
            mark_dirty(&v);
            bump_layout(&v);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Graphic::set_color" => |vm, a| {
            let v = rv(vm, &a[0]);
            let c = to_col(&rv(vm, &a[1]));
            let ch = with_comp(&v, |k| match &mut k.k {
                CK::Image(i) | CK::RawImage(i) => std::mem::replace(&mut i.color, c) != c,
                CK::Text(t) => std::mem::replace(&mut t.color, c) != c,
                _ => false,
            })
            .unwrap_or(false);
            if ch {
                mark_dirty(&v);
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.Graphic::get_color" => |vm, a| {
            let v = rv(vm, &a[0]);
            let c = with_comp(&v, |k| match &k.k {
                CK::Image(i) | CK::RawImage(i) => i.color,
                CK::Text(t) => t.color,
                _ => [1.0; 4],
            })
            .unwrap_or([1.0; 4]);
            Ok(col(vm, c))
        },
        "UnityEngine.UI.Image::set_sprite" => |vm, a| {
            let v = rv(vm, &a[0]);
            let ch = with_comp(&v, |k| if let CK::Image(i) = &mut k.k { !same(&std::mem::replace(&mut i.sprite, a[1].clone()), &a[1]) } else { false }).unwrap_or(false);
            if ch {
                mark_dirty(&v);
                bump_layout(&v);
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.Image::get_sprite" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(with_comp(&v, |k| if let CK::Image(i) = &k.k { i.sprite.clone() } else { Value::Null }).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.RawImage::set_texture" => stub0!(),
        "UnityEngine.UI.Selectable::set_interactable" => |vm, a| {
            let v = rv(vm, &a[0]);
            if sel(&v, |s| std::mem::replace(&mut s.interactable, a[1].as_bool()) != a[1].as_bool()) == Some(true) {
                mark_sel_dirty(&v);
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.Selectable::get_interactable" | "UnityEngine.UI.Selectable::IsInteractable" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::bool(sel(&v, |s| s.interactable).unwrap_or(true)))
        },
        "UnityEngine.UI.Selectable::get_image" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| s.target_graphic.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Button::get_onClick" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| s.on_click.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.Events.UnityEvent::AddListener" => |vm, a| {
            let ev = rv(vm, &a[0]);
            let d = if let Value::Obj(o) = &a[1] {
                if let Ext::Delegate(l) = &*o.x.borrow() { l.clone() } else { vec![] }
            } else {
                vec![]
            };
            with_comp(&ev, |c| if let CK::Event(e) = &mut c.k { e.runtime.extend(d) });
            Ok(Value::Null)
        },
        "UnityEngine.Events.UnityEventBase::RemoveAllListeners" => |vm, a| {
            let ev = rv(vm, &a[0]);
            with_comp(&ev, |c| if let CK::Event(e) = &mut c.k { e.runtime.clear() });
            Ok(Value::Null)
        },
        "UnityEngine.Events.UnityEvent::Invoke" => |vm, a| {
            let ev = rv(vm, &a[0]);
            vm.invoke_event(&ev, vec![]);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Slider::get_value" | "UnityEngine.UI.Toggle::get_isOn" | "UnityEngine.UI.Scrollbar::get_value" | "UnityEngine.UI.Dropdown::get_value" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| match s.kind {
                SelKind::Toggle => Value::bool(s.is_on),
                SelKind::Dropdown => Value::I32(s.value as i32),
                _ => Value::F32(s.value),
            })
            .unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Slider::set_value" | "UnityEngine.UI.Scrollbar::set_value" | "UnityEngine.UI.Dropdown::set_value" => |vm, a| {
            let v = rv(vm, &a[0]);
            let nv = a[1].as_f64() as f32;
            let (changed, on_change, kind) = sel(&v, |s| {
                let nv = if s.whole { nv.round() } else { nv.clamp(s.min.min(s.max), s.max.max(s.min)) };
                let ch = (s.value - nv).abs() > 1e-7;
                s.value = nv;
                (ch, s.on_change.clone(), s.kind)
            })
            .unwrap_or((false, Value::Null, SelKind::Slider));
            if kind != SelKind::Dropdown && changed {
                slider_visuals(vm, &v);
            }
            if kind == SelKind::Scrollbar && changed {
                vm.scrollbar_moved(&v);
            }
            if changed && !on_change.is_null() {
                let val = sel(&v, |s| s.value).unwrap_or(0.0);
                vm.invoke_event(&on_change, vec![Value::F32(val)]);
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.InputField::get_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| Value::str(&s.text)).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.InputField::set_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            let s = to_string(vm, &a[1])?;
            let tc = sel(&v, |x| {
                x.text = s.clone();
                x.text_component.clone()
            })
            .unwrap_or(Value::Null);
            set_text_comp(&tc, &s);
            Ok(Value::Null)
        },
        "UnityEngine.UI.InputField::get_isFocused" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::bool(sel(&v, |s| s.focused).unwrap_or(false)))
        },
        "UnityEngine.UI.Dropdown::ClearOptions" => |vm, a| {
            let v = rv(vm, &a[0]);
            sel(&v, |s| s.options.clear());
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown::Hide" => stub0!(),
        "UnityEngine.UI.Dropdown::RefreshShownValue" => |vm, a| {
            // the caption shows the text of the selected option
            let v = rv(vm, &a[0]);
            let (cap, opt) = sel(&v, |s| (s.caption.clone(), s.options.get(s.value as usize).cloned())).unwrap_or((Value::Null, None));
            if let Some(o) = opt {
                if let Some(t) = opt_data(&o, |d| d.text.clone()) {
                    set_text_comp(&cap, &t);
                }
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown::AddOptions" => |vm, a| {
            // List<string> or List<OptionData>
            let v = rv(vm, &a[0]);
            let items = iter_values(vm, &a[1])?;
            let id = vm.type_id(OPTION_DATA);
            let mut opts = vec![];
            for it in items {
                if opt_data(&it, |_| ()).is_some() {
                    opts.push(it);
                } else {
                    let text = to_string(vm, &it)?;
                    opts.push(Value::Obj(Obj::new(id, 0, Ext::Native(Box::new(OptD { text, image: Value::Null })))));
                }
            }
            sel(&v, |s| s.options.extend(opts));
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown::get_options" => |vm, a| {
            let v = rv(vm, &a[0]);
            let items = sel(&v, |s| s.options.clone()).unwrap_or_default();
            Ok(crate::bcl_core::new_list(vm, items))
        },
        "UnityEngine.UI.Dropdown::get_captionText" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| s.caption.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Dropdown/OptionData::get_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::str(&opt_data(&v, |d| d.text.clone()).unwrap_or_default()))
        },
        "UnityEngine.UI.Dropdown/OptionData::set_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            let t = to_string(vm, &a[1])?;
            opt_data(&v, |d| d.text = t);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown/OptionData::get_image" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(opt_data(&v, |d| d.image.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Dropdown/OptionData::set_image" => |vm, a| {
            let v = rv(vm, &a[0]);
            let img = a[1].clone();
            opt_data(&v, |d| d.image = img);
            Ok(Value::Null)
        },
        "UnityEngine.EventSystems.PointerEventData::get_button" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::I32(crate::input::with_ped(&v, |p| p.button).unwrap_or(0)))
        },
        "UnityEngine.EventSystems.PointerEventData::get_pointerId" => |_vm, _a| Ok(Value::I32(-1)),
        "UnityEngine.EventSystems.PointerEventData::get_clickCount" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::I32(crate::input::with_ped(&v, |p| p.click_count).unwrap_or(1)))
        },
        "UnityEngine.EventSystems.PointerEventData::get_pointerPress" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(crate::input::with_ped(&v, |p| p.press.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.EventSystems.PointerEventData::get_position" => |vm, a| {
            let v = rv(vm, &a[0]);
            let p = crate::input::with_ped(&v, |p| p.pos).unwrap_or([0.0; 2]);
            Ok(vec2(vm, p))
        },
        // ------------------------------------------------------- Texture / Sprite (generes a l'execution)
        "UnityEngine.Texture2D::.ctor" | "UnityEngine.Texture2D::SetPixel" | "UnityEngine.Texture2D::Apply" | "UnityEngine.ImageConversion::EncodeToPNG" => stub0!(),
        "UnityEngine.Sprite::Create" => stub0!(),
        "UnityEngine.AudioSource::Play" | "UnityEngine.AudioSource::set_clip" | "UnityEngine.AudioSource::set_mute" => stub0!(),
        "UnityEngine.Matrix4x4::TRS" | "UnityEngine.GUI::set_matrix" => stub0!(),
        // ------------------------------------------------------- Network (offline)
        "UnityEngine.WWW::.ctor" => stub0!(),
        "UnityEngine.WWW::get_isDone" => |_vm, _a| Ok(Value::I32(1)),
        "UnityEngine.WWW::get_error" => |_vm, _a| Ok(Value::str("offline")),
        "UnityEngine.WWW::get_text" => |_vm, _a| Ok(Value::str("")),
        "UnityEngine.WWWForm::.ctor" | "UnityEngine.WWWForm::AddField" => stub0!(),
        // ------------------------------------------------------- Steam / Kartridge (stubs)
        k if k.starts_with("Steamworks.") || k.starts_with("Kongregate.") => {
            if k.ends_with("::Init") || k.ends_with("KongregateAPI_IsReady") {
                |_vm, _a| Ok(Value::I32(0))
            } else if k.ends_with("RestartAppIfNecessary") {
                |_vm, _a| Ok(Value::I32(0))
            } else if k.ends_with("GetPersonaName") || k.ends_with("GetCurrentGameLanguage") || k.ends_with("GetUsername") {
                |_vm, _a| Ok(Value::str("english"))
            } else {
                |_vm, _a| Ok(Value::Null)
            }
        }
        _ => return None,
    };
    Some(f)
}

fn alive(vm: &mut Vm, v: &Value) -> Value {
    let v = vm.deref(v.clone());
    let g = go_of(&v);
    if !g.is_null() && with_go(&g, |x| x.destroyed).unwrap_or(false) {
        return Value::Null;
    }
    v
}

trait StructI {
    fn as_struct_i(&self, i: usize) -> i32;
}
impl StructI for Value {
    fn as_struct_i(&self, i: usize) -> i32 {
        if let Value::Struct(s) = self { s.f[i].as_i32() } else { 0 }
    }
}

/// Public instance fields of native components (read).
pub fn native_field(vm: &mut Vm, o: &Value, key: &str) -> R<Value> {
    match key {
        "UnityEngine.UI.Toggle::graphic" => Ok(sel(o, |s| s.toggle_graphic.clone()).unwrap_or(Value::Null)),
        "UnityEngine.UI.Toggle::onValueChanged" | "UnityEngine.UI.Slider::onValueChanged" => Ok(sel(o, |s| s.on_change.clone()).unwrap_or(Value::Null)),
        _ => vm.throw("System.NotImplementedException", &format!("native field {}", key)),
    }
}

/// Updates fill/handle of a Slider / Scrollbar according to its value.
pub fn slider_visuals(vm: &mut Vm, v: &Value) {
    let _ = vm;
    let info = sel(v, |s| (s.kind, s.value, s.min, s.max, s.direction, s.fill_rect.clone(), s.handle_rect.clone(), s.size));
    let (kind, val, min, max, dir, fill, handle, size) = match info {
        Some(i) => i,
        None => return,
    };
    let n = if kind == SelKind::Scrollbar { val } else if max > min { (val - min) / (max - min) } else { 0.0 };
    let n = n.clamp(0.0, 1.0);
    // axis: 0/1 horizontal (left->right / right->left), 2/3 vertical
    let (axis, rev) = match dir {
        0 => (0, false),
        1 => (0, true),
        2 => (1, false),
        _ => (1, true),
    };
    if kind == SelKind::Slider {
        mark_dirty(&fill);
        mark_dirty(&handle);
        with_tr(&fill, |t| {
            t.amin[axis] = if rev { 1.0 - n } else { 0.0 };
            t.amax[axis] = if rev { 1.0 } else { n };
        });
        with_tr(&handle, |t| {
            let p = if rev { 1.0 - n } else { n };
            t.amin[axis] = p;
            t.amax[axis] = p;
        });
    } else {
        mark_dirty(&handle);
        with_tr(&handle, |t| {
            let m = (1.0 - size) * n;
            let (a, b) = if rev { (1.0 - m - size, 1.0 - m) } else { (m, m + size) };
            t.amin[axis] = a;
            t.amax[axis] = b;
        });
    }
}

/// `Input.GetKey(KeyCode)` passes an int, `Input.GetKey("left shift")` a name.
fn key_arg(vm: &mut Vm, v: &Value) -> i32 {
    match v {
        Value::I32(i) => *i,
        v => crate::input::key_by_name(&crate::dev::show(vm, v)),
    }
}

/// Access to a Dropdown option object.
fn opt_data<R>(v: &Value, f: impl FnOnce(&mut OptD) -> R) -> Option<R> {
    if let Value::Obj(o) = v {
        if let Ext::Native(b) = &mut *o.x.borrow_mut() {
            if let Some(d) = b.downcast_mut::<OptD>() {
                return Some(f(d));
            }
        }
    }
    None
}
