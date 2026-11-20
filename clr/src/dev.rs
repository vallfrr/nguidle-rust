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

/// Names of boolean fields that mean "unlocked / bought / completed / feature on" (see `ultimate`).
fn is_unlock_flag(n: &str) -> bool {
    const SKIP: &[&str] = &["Challenge", "challenge", "tutorial", "Tutorial", "isNaughty", "steam", "Steam", "firstBossEver", "ignore", "troll", "Troll", "filter", "simple", "unassign", "shakey", "poopOnly", "antiFlicker", "syncTraining", "inQuest", "auto"];
    if SKIP.iter().any(|s| n.contains(s)) {
        return false;
    }
    let has_prefix = n.starts_with("has") && n.chars().nth(3).map(|c| c.is_uppercase() || c.is_ascii_digit()).unwrap_or(false);
    has_prefix || ["On", "Unlocked", "unlocked", "Bought", "bought", "Complete", "complete", "Enabled", "enabled"].iter().any(|s| n.ends_with(s))
}

/// "Ultimate" state (dev): sets every unlock-like boolean field reachable from the scene's game objects
/// (depth <= 6, game classes only) so that progression-gated UI elements become visible. Returns a summary.
pub fn ultimate(vm: &mut Vm) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<(Value, usize)> = vm.eng.user_comps.iter().map(|c| (c.clone(), 0)).collect();
    let mut set = std::collections::BTreeMap::<String, usize>::new();
    while let Some((v, depth)) = stack.pop() {
        let Value::Obj(o) = &v else { continue };
        if o.ty >= vm.n_user || !seen.insert(std::rc::Rc::as_ptr(o) as usize) {
            continue;
        }
        let tys = vm.types[o.ty as usize].field_tys.clone();
        let names = vm.types[o.ty as usize].fields.clone();
        let tname = vm.types[o.ty as usize].name.clone();
        let mut f = o.f.borrow_mut();
        for (i, v) in f.iter_mut().enumerate() {
            match (tys.get(i), &*v) {
                (Some(crate::program::Ty::Bool), Value::I32(0)) if is_unlock_flag(&names[i]) => {
                    *v = Value::I32(1);
                    *set.entry(format!("{tname}.{}", names[i])).or_default() += 1;
                }
                (Some(crate::program::Ty::User(_)), Value::Obj(_)) if depth < 6 => stack.push((v.clone(), depth + 1)),
                _ => {}
            }
        }
    }
    format!("{} unlock flags set: {}", set.len(), set.keys().cloned().collect::<Vec<_>>().join(" "))
}

/// GameObjects whose name contains `needle`: tag, active flag and world rectangle (virtual px, y up).
pub fn find(vm: &mut Vm, needle: &str) -> String {
    let mut out = String::new();
    for go in vm.eng.gos.clone() {
        let Some((name, active, tag, tr)) = with_go(&go, |g| (g.name.clone(), g.active, g.tag.clone(), g.transform.clone())) else { continue };
        if !name.contains(needle) {
            continue;
        }
        let x = crate::ui::xf(vm, &tr);
        out += &format!("{name:<40} tag={tag:<9} active={active} origin=({:.0},{:.0}) size=({:.0},{:.0}) scale=({:.2},{:.2})\n", x.origin[0], x.origin[1], x.w, x.h, x.scale[0], x.scale[1]);
    }
    out
}

/// Root GameObjects of the 58 menus (`MenuSwapper.allMenus`).
pub fn menu_roots(vm: &Vm) -> Vec<Value> {
    let Some(Value::Obj(o)) = comp_of(vm, "MenuSwapper") else { return vec![] };
    let Some(slot) = field_slot(vm, &o, "allMenus") else { return vec![] };
    let arr = o.f.borrow()[slot].clone();
    let Value::Obj(a) = arr else { return vec![] };
    let v = match &*a.x.borrow() {
        Ext::Arr(v) => v.clone(),
        _ => vec![],
    };
    v
}

/// UI map (task R1): for every menu, shows it (`swapMenu`) and lists its visible elements with their kind, rectangle
/// (960x600, top-left), anchoring (point / stretch) and image mode. Returns (JSON lines with every element, Markdown summary).
/// No game text is written: only lengths, so the summary can be published.
pub fn uimap(vm: &mut Vm) -> (String, String) {
    use crate::ui::{child_xf, xf, Xf};
    let roots = menu_roots(vm);
    let swapper = comp_of(vm, "MenuSwapper");
    let mut json = String::new();
    let mut md = String::from("| # | menu | buttons | other interactive | texts | images | stretched | large simple images (drawn layout, candidates to redraw) |\n|---|---|---|---|---|---|---|---|\n");
    for (i, root) in roots.iter().enumerate() {
        if let (Some(Value::Obj(so)), true) = (&swapper, !root.is_null()) {
            if let Some(m) = vm.find_method(so.ty, "swapMenu", 2) {
                let _ = vm.invoke(m, vec![Value::Obj(so.clone()), Value::I32(i as i32)]);
            }
            for _ in 0..3 {
                vm.frame(1.0 / 30.0);
            }
        }
        let name = with_go(root, |g| g.name.clone()).unwrap_or_default();
        let tr = with_go(root, |g| g.transform.clone()).unwrap_or(Value::Null);
        let (mut buttons, mut inter, mut texts, mut images, mut stretched) = (0, 0, 0, 0, 0);
        let mut large: Vec<String> = vec![];
        fn walk(vm: &mut Vm, tr: &Value, x: Xf, path: &str, out: &mut Vec<(String, Xf, Vec<Value>, Tr2)>) {
            let go = go_of(tr);
            let Some((active, name, comps)) = with_go(&go, |g| (g.active, g.name.clone(), g.comps.clone())) else { return };
            if !active {
                return;
            }
            let path = format!("{path}/{name}");
            let t = with_tr(tr, |t| Tr2 { stretch: t.amin != t.amax }).unwrap_or(Tr2 { stretch: false });
            out.push((path.clone(), x.clone(), comps, t));
            for ch in with_tr(tr, |t| t.children.clone()).unwrap_or_default() {
                if let Some(cx) = with_tr(&ch, |t| child_xf(&x, t)) {
                    walk(vm, &ch, cx, &path, out);
                }
            }
        }
        let mut nodes = vec![];
        let rx = xf(vm, &tr);
        walk(vm, &tr, rx, "", &mut nodes);
        let menu_area = nodes.first().map(|n| (n.1.w * n.1.h).abs()).unwrap_or(1.0).max(1.0);
        for (path, x, comps, t) in &nodes {
            let r = [x.origin[0] - x.pivot[0] * x.w, 600.0 - (x.origin[1] + (1.0 - x.pivot[1]) * x.h), x.origin[0] + (1.0 - x.pivot[0]) * x.w, 600.0 - (x.origin[1] - x.pivot[1] * x.h)];
            if t.stretch {
                stretched += 1;
            }
            for c in comps {
                let desc = with_comp(c, |k| match &k.k {
                    CK::Image(im) | CK::RawImage(im) => Some(("image", format!("\"sprite\":{:?},\"mode\":{}", sprite_name(&im.sprite), im.typ))),
                    CK::Text(tx) => Some(("text", format!("\"len\":{},\"size\":{},\"align\":{},\"best_fit\":{}", tx.text.chars().count(), tx.size, tx.align, tx.best_fit))),
                    CK::Sel(s) => Some(("selectable", format!("\"sel\":\"{:?}\",\"interactable\":{}", s.kind, s.interactable))),
                    CK::ScrollRect { .. } => Some(("scroll", String::new())),
                    CK::Mask { .. } => Some(("mask", String::new())),
                    CK::Layout { kind, .. } => Some(("layout", format!("\"kind\":{kind:?}"))),
                    _ => None,
                })
                .flatten();
                let Some((kind, extra)) = desc else { continue };
                match kind {
                    "image" => {
                        images += 1;
                        let simple = extra.contains("\"mode\":0");
                        let area = (x.w * x.h).abs();
                        let sp = extra.split('"').nth(3).unwrap_or("-").to_string();
                        if simple && sp != "-" && area >= 0.3 * menu_area && !large.contains(&sp) {
                            large.push(sp);
                        }
                    }
                    "text" => texts += 1,
                    "selectable" => {
                        if extra.contains("Button") {
                            buttons += 1;
                        } else {
                            inter += 1;
                        }
                    }
                    _ => {}
                }
                let sep = if extra.is_empty() { "" } else { "," };
                json += &format!("{{\"menu\":{i},\"path\":{path:?},\"kind\":\"{kind}\",\"rect\":[{:.0},{:.0},{:.0},{:.0}],\"stretch\":{}{sep}{extra}}}\n", r[0], r[1], r[2], r[3], t.stretch);
            }
        }
        md += &format!("| {i} | {name} | {buttons} | {inter} | {texts} | {images} | {stretched} | {} |\n", large.join(", "));
    }
    (json, md)
}

pub struct Tr2 {
    stretch: bool,
}
