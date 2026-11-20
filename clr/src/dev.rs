//! Script-driven development tools (designed for the assistant: text output, no manual clicks).
//! Paths: `Type.field.field...` where `Type` = name of a game class attached to the scene (first component of that type).
//! ponytail: interpreter mode only (native state lives in the typed twins of ngu-host); wire to the bridge if needed.
use crate::unity::*;
use crate::value::*;
use crate::Vm;

fn comp_of(vm: &Vm, ty: &str) -> Option<Value> {
    vm.eng.user_comps.iter().find(|c| matches!(c, Value::Obj(o) if vm.types[o.ty as usize].name == ty)).cloned()
}

fn field_slot(vm: &Vm, o: &Obj, name: &str) -> Option<usize> {
    vm.types[o.ty as usize].fields.iter().position(|n| n == name)
}

/// (parent object, slot) of the last link, or the root alone if the path has no field.
fn resolve(vm: &Vm, path: &str) -> Result<(Value, Option<usize>), String> {
    let mut it = path.split('.');
    let root = it.next().unwrap_or("");
    let mut cur = comp_of(vm, root).ok_or(format!("no component {root}"))?;
    let names: Vec<&str> = it.collect();
    for (i, n) in names.iter().enumerate() {
        let Value::Obj(o) = &cur else { return Err(format!("{n}: parent is null or not an object")) };
        let s = field_slot(vm, o, n).ok_or(format!("no field {n} in {}", vm.types[o.ty as usize].name))?;
        if i + 1 == names.len() {
            return Ok((cur.clone(), Some(s)));
        }
        let next = o.f.borrow()[s].clone();
        cur = next;
    }
    Ok((cur, None))
}

pub fn show(vm: &mut Vm, v: &Value) -> String {
    match v {
        Value::Obj(o) if vm.types[o.ty as usize].name == "System.String" => crate::bcl_core::to_string(vm, v).unwrap_or_default(),
        Value::Str(s) => s.to_string(),
        Value::I32(x) => x.to_string(),
        Value::I64(x) => x.to_string(),
        Value::F32(x) => x.to_string(),
        Value::F64(x) => x.to_string(),
        o => vm.describe(o),
    }
}

pub fn get(vm: &mut Vm, path: &str) -> String {
    match resolve(vm, path) {
        Ok((Value::Obj(o), Some(s))) => {
            let v = o.f.borrow()[s].clone();
            show(vm, &v)
        }
        Ok((v, _)) => show(vm, &v),
        Err(e) => format!("ERROR {e}"),
    }
}

/// The new value takes the type of the old one (true/false accepted for booleans).
pub fn set(vm: &mut Vm, path: &str, val: &str) -> String {
    let (Value::Obj(o), Some(s)) = (match resolve(vm, path) {
        Ok(r) => r,
        Err(e) => return format!("ERROR {e}"),
    }) else {
        return "ERROR path without field".into();
    };
    let old = o.f.borrow()[s].clone();
    let b = |v: &str| match v {
        "true" => Some(1),
        "false" => Some(0),
        _ => v.parse().ok(),
    };
    let new = match &old {
        Value::I32(_) => b(val).map(Value::I32),
        Value::I64(_) => val.parse().ok().map(Value::I64),
        Value::F32(_) => val.parse().ok().map(Value::F32),
        Value::F64(_) => val.parse().ok().map(Value::F64),
        _ => Some(Value::str(val)),
    };
    match new {
        Some(n) => {
            o.f.borrow_mut()[s] = n;
            format!("{path} = {val}")
        }
        None => format!("ERROR value {val} incompatible with {}", show(vm, &old)),
    }
}

fn arg(a: &str) -> Value {
    match a {
        "true" => Value::bool(true),
        "false" => Value::bool(false),
        _ if a.parse::<i32>().is_ok() => Value::I32(a.parse().unwrap()),
        _ if a.parse::<f64>().is_ok() => Value::F32(a.parse().unwrap()),
        _ => Value::str(a),
    }
}

/// `Type.method` (or `Type.field.method`) with text arguments (integer, floating point -> float, true/false, string).
pub fn call(vm: &mut Vm, path: &str, args: &[&str]) -> String {
    let (obj_path, m) = path.rsplit_once('.').unwrap_or((path, ""));
    let target = match resolve(vm, obj_path) {
        Ok((Value::Obj(o), Some(s))) => o.f.borrow()[s].clone(),
        Ok((v, _)) => v,
        Err(e) => return format!("ERROR {e}"),
    };
    let Value::Obj(o) = &target else { return "ERROR null target".into() };
    let Some(mid) = vm.find_method(o.ty, m, args.len() + 1) else { return format!("ERROR no method {m}/{}", args.len()) };
    let mut a = vec![target.clone()];
    a.extend(args.iter().map(|s| arg(s)));
    match vm.invoke(mid, a) {
        Ok(Value::Null) => format!("{path} : ok"),
        Ok(v) => format!("{path} -> {}", show(vm, &v)),
        Err(e) => format!("{path} : EXCEPTION {}", show(vm, &e)),
    }
}

/// Visible interactive element whose text contains `label` (case-insensitive): click point (y up).
pub fn find_click(vm: &mut Vm, label: &str) -> Option<(f32, f32, String)> {
    let l = label.to_lowercase();
    let sh = vm.eng.screen.1 as f32;
    let list = crate::focus::collect(vm);
    let f = list.iter().find(|f| f.label.to_lowercase() == l).or_else(|| list.iter().find(|f| f.label.to_lowercase().contains(&l)))?;
    Some(((f.rect[0] + f.rect[2]) / 2.0, sh - (f.rect[1] + f.rect[3]) / 2.0, f.label.clone()))
}

/// Active nodes whose rectangle contains (x, y) (960x600, top-left), back to front: path + components.
pub fn inspect(vm: &mut Vm, px: f32, py: f32) -> String {
    let mut out = String::new();
    for (path, comps) in crate::focus::nodes_at(vm, px, py) {
        let d: Vec<String> = comps
            .iter()
            .filter_map(|c| {
                let user = matches!(c, Value::Obj(o) if o.ty < vm.n_user).then(|| match c {
                    Value::Obj(o) => vm.types[o.ty as usize].name.clone(),
                    _ => unreachable!(),
                });
                with_comp(c, |k| {
                    let on = if k.enabled { "" } else { "(off)" };
                    Some(match &k.k {
                        CK::Image(i) | CK::RawImage(i) => format!("Image{on}[col={:?} sprite={}]", i.color, crate::unity::sprite_name(&i.sprite)),
                        CK::Text(t) => format!("Text{on}[{:?} size={} col={:?}]", t.text.chars().take(30).collect::<String>(), t.size, t.color),
                        CK::Sel(s) => format!("Selectable{on}[interactable={} transition={}]", s.interactable, s.transition),
                        CK::Mask { show_graphic } => format!("Mask{on}[show_graphic={show_graphic}]"),
                        CK::Canvas(_) => format!("Canvas{on}"),
                        CK::ScrollRect { .. } => format!("ScrollRect{on}"),
                        CK::Layout { kind, .. } => format!("{kind}{on}"),
                        CK::Outline { .. } => format!("Outline{on}"),
                        CK::Transform(_) => return None,
                        CK::User => format!("{}{on}", user.clone().unwrap_or_default()),
                        CK::Other(n) => format!("{n}{on}"),
                        _ => return None,
                    })
                })
                .flatten()
            })
            .collect();
        out += &format!("{path}  {}\n", d.join(" "));
    }
    out
}

/// The screen as text: interactive (clickable) elements then visible texts, with their rectangle (960x600, top-left).
pub fn ui(vm: &mut Vm) -> String {
    let mut s = String::new();
    for f in crate::focus::collect(vm) {
        s += &format!("{:<9} {:>4.0},{:>3.0} {:>4.0},{:>3.0}  {}\n", f.kind, f.rect[0], f.rect[1], f.rect[2], f.rect[3], f.label.replace('\n', " | "));
    }
    for (r, t) in crate::focus::visible_texts(vm) {
        s += &format!("texte     {:>4.0},{:>3.0} {:>4.0},{:>3.0}  {}\n", r[0], r[1], r[2], r[3], t.replace('\n', " | "));
    }
    s
}
