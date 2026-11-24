use super::*;

pub(super) fn lookup(key: &str) -> Option<NativeFn> {
    let function: NativeFn = match key {
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
                crate::unity::bump_hierarchy();
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
                *o.x.borrow_mut() = Ext::Native(Box::new(GoData { name, active: true, port_hidden: false, layer: 0, tag: "Untagged".into(), comps: vec![tr.clone()], transform: tr, destroyed: false, dirty: true, cache: None, in_layout: false, hier: (0, false) }));
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
                invalidate_input();
                let _ = vm.call_msg(&v, if on { "OnEnable" } else { "OnDisable" });
            }
            Ok(Value::Null)
        },
        "UnityEngine.MonoBehaviour::StartCoroutine" => |vm, a| {
            let owner = rv(vm, &a[0]);
            Ok(vm.start_coroutine(&owner, a[1].clone()))
        },
        "UnityEngine.MonoBehaviour::Invoke" | "UnityEngine.MonoBehaviour::InvokeRepeating" => |vm, a| {
            let owner = rv(vm, &a[0]);
            let method = match &a[1] { Value::Str(name) => name.clone(), _ => Rc::from("") };
            let repeat = a.get(3).map_or(0.0, |v| v.as_f32().max(0.0001) as f64);
            vm.schedule_invoke(owner, method, a[2].as_f32() as f64, repeat);
            Ok(Value::Null)
        },
        "UnityEngine.MonoBehaviour::CancelInvoke" => |vm, a| {
            let owner = rv(vm, &a[0]);
            let name = a.get(1).and_then(Value::as_str);
            vm.eng.invokes.retain(|i| !(same(&i.owner, &owner) && name.is_none_or(|name| name == i.method.as_ref())));
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
                crate::motion::world_position_changed(vm, &t, cur, np);
                crate::ui::set_world_pos(vm, &t, np);
                mark_dirty(&t);
                invalidate_input();
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
                invalidate_input();
            }
            Ok(Value::Null)
        },
        "UnityEngine.Transform::Translate" => |vm, a| {
            let t = rv(vm, &a[0]);
            let p = rv(vm, &a[1]);
            let cur = crate::ui::world_pos(vm, &t);
            crate::ui::set_world_pos(vm, &t, [cur[0] + sf(&p, 0), cur[1] + sf(&p, 1), cur[2] + sf(&p, 2)]);
            mark_dirty(&t);
            invalidate_input();
            Ok(Value::Null)
        },
        "UnityEngine.Transform::SetAsFirstSibling" | "UnityEngine.Transform::SetAsLastSibling" => |vm, a| {
            let t = rv(vm, &a[0]);
            let first = a.len() == 1 && false;
            let _ = first;
            let parent = with_tr(&t, |x| x.parent.clone()).unwrap_or(Value::Null);
            let changed = with_tr(&parent, |p| {
                if let Some(i) = p.children.iter().position(|c| same(c, &t)) {
                    if i + 1 == p.children.len() { return false; }
                    let c = p.children.remove(i);
                    p.children.push(c);
                    return true;
                }
                false
            }).unwrap_or(false);
            if changed {
                mark_dirty(&t);
                invalidate_input();
            }
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
        _ => return None,
    };
    Some(function)
}
