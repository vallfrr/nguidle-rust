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
    unity_rand_state(&mut vm.eng.rng)
}

fn unity_rand_state(s: &mut [u32; 4]) -> u32 {
    let t = s[0] ^ (s[0] << 11);
    s[0] = s[1];
    s[1] = s[2];
    s[2] = s[3];
    s[3] = (s[3] ^ (s[3] >> 19)) ^ (t ^ (t >> 8));
    s[3]
}

fn unity_range_float(r: u32, lo: f32, hi: f32) -> f32 {
    let fraction = (r & 0x7FFFFF) as f32 / 8388607.0;
    (1.0 - fraction) * hi + fraction * lo
}

fn unity_range_int(r: u32, lo: i32, hi: i32) -> i32 {
    let span = (hi as i64 - lo as i64).max(0) as u32;
    if span == 0 { lo } else { lo.wrapping_add((r % span) as i32) }
}

fn init_state(vm: &mut Vm, seed: i32) {
    vm.eng.rng = unity_seed_state(seed);
}

fn unity_seed_state(seed: i32) -> [u32; 4] {
    let s0 = seed as u32;
    let s1 = s0.wrapping_mul(1812433253).wrapping_add(1);
    let s2 = s1.wrapping_mul(1812433253).wrapping_add(1);
    let s3 = s2.wrapping_mul(1812433253).wrapping_add(1);
    [s0, s1, s2, s3]
}

#[cfg(test)]
mod unity_random_tests {
    use super::{unity_rand_state, unity_range_float, unity_range_int, unity_seed_state};

    #[test]
    fn matches_unity_2019_reference_vectors() {
        let vectors = include_str!("../tests/fixtures/unity2019_rng.csv");
        let mut state = [0; 4];
        for line in vectors.lines().skip(1) {
            let fields: Vec<_> = line.split(',').collect();
            let seed: i32 = fields[1].parse().unwrap();
            let index: usize = fields[2].parse().unwrap();
            if index == 0 {
                state = unity_seed_state(seed);
            }
            let random = unity_rand_state(&mut state);
            match fields[0] {
                "value" => {
                    let actual = (random & 0x7fffff) as f32 / 8388607.0;
                    let expected: f32 = fields[3].parse().unwrap();
                    assert_eq!(actual.to_bits(), expected.to_bits(), "{line}");
                }
                "range_int" => {
                    let actual = unity_range_int(random, -100, 100);
                    let expected: i32 = fields[3].parse().unwrap();
                    assert_eq!(actual, expected, "{line}");
                }
                "range_float" => {
                    let actual = unity_range_float(random, -10.0, 30.0);
                    let expected: f32 = fields[3].parse().unwrap();
                    assert_eq!(actual.to_bits(), expected.to_bits(), "{line}");
                }
                kind => panic!("unexpected vector kind {kind}"),
            }
        }
    }

    #[test]
    fn state_roundtrips_match_unity_2019() {
        let vectors = include_str!("../tests/fixtures/unity2019_state.csv");
        let mut state = [0; 4];
        for line in vectors.lines().skip(1) {
            let fields: Vec<_> = line.split(',').collect();
            let seed: i32 = fields[0].parse().unwrap();
            let index: usize = fields[1].parse().unwrap();
            if index == 0 { state = unity_seed_state(seed); }
            let expected_state: Vec<u32> = fields[2..6].iter().map(|v| v.parse::<i32>().unwrap() as u32).collect();
            assert_eq!(state.as_slice(), expected_state, "{line}");
            let saved = state;
            let random = unity_rand_state(&mut state);
            let value = (random & 0x7fffff) as f32 / 8388607.0;
            let mut restored = saved;
            let random_again = unity_rand_state(&mut restored);
            let value_again = (random_again & 0x7fffff) as f32 / 8388607.0;
            assert_eq!(value.to_bits(), fields[6].parse::<f32>().unwrap().to_bits(), "{line}");
            assert_eq!(value_again.to_bits(), fields[7].parse::<f32>().unwrap().to_bits(), "{line}");
            assert_eq!(state, restored, "{line}");
        }
    }
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

pub(crate) fn save_prefs(vm: &mut Vm) {
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

mod controls;
mod runtime;
mod scene;
mod services;

pub fn lookup(key: &str) -> Option<NativeFn> {
    runtime::lookup(key)
        .or_else(|| scene::lookup(key))
        .or_else(|| controls::lookup(key))
        .or_else(|| services::lookup(key))
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
    let info = sel(v, |s| (s.kind, s.value, s.min, s.max, s.direction, s.fill_rect.clone(), s.handle_rect.clone(), s.size, s.interactable));
    let (kind, val, min, max, dir, fill, handle, size, interactable) = match info {
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
        if vm.eng.motion.enabled && !interactable {
            if let Some(old) = with_tr(&fill, |t| if rev { 1.0 - t.amin[axis] } else { t.amax[axis] }) {
                crate::motion::slider_changed(vm, v, &fill, &handle, (axis, rev), old.clamp(0.0, 1.0), n);
            }
        }
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
pub(crate) fn opt_data<R>(v: &Value, f: impl FnOnce(&mut OptD) -> R) -> Option<R> {
    if let Value::Obj(o) = v {
        if let Ext::Native(b) = &mut *o.x.borrow_mut() {
            if let Some(d) = b.downcast_mut::<OptD>() {
                return Some(f(d));
            }
        }
    }
    None
}

pub(crate) fn set_dropdown_value(vm: &mut Vm, comp: &Value, value: i32) {
    let Some((changed, event, caption, option, value)) = sel(comp, |s| {
        let value = (value.max(0) as usize).min(s.options.len().saturating_sub(1));
        let changed = s.value as usize != value;
        s.value = value as f32;
        (changed, s.on_change.clone(), s.caption.clone(), s.options.get(value).cloned(), value)
    }) else { return };
    if let Some(text) = option.and_then(|item| opt_data(&item, |d| d.text.clone())) {
        set_text_comp(&caption, &text);
    }
    if changed && !event.is_null() {
        vm.invoke_event(&event, vec![Value::I32(value as i32)]);
    }
}
