//! Focus navigation (gamepad / keyboard): list of the really visible and reachable interactive elements,
//! spatial movement (up / down / left / right) and activation (virtual click at the centre).
//! Coordinates: virtual pixels 960x600, origin at the top left (y down).
use crate::input::{raycast, Ev};
use crate::ui::{child_xf, xf, Xf};
use crate::unity::*;
use crate::value::*;
use crate::Vm;

#[derive(Clone)]
pub struct Focusable {
    pub go: Value,
    /// x0, y0, x1, y1
    pub rect: [f32; 4],
    pub label: String,
    pub kind: &'static str,
}

/// Bounding box (top-left screen space) of a node.
fn bbox(x: &Xf, vh: f32) -> [f32; 4] {
    let (c, s) = (x.rot.cos(), x.rot.sin());
    let (x0, y0) = (-x.pivot[0] * x.w, -x.pivot[1] * x.h);
    let pts = [(x0, y0), (x0 + x.w, y0), (x0 + x.w, y0 + x.h), (x0, y0 + x.h)];
    let (mut a, mut b, mut cc, mut d) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (lx, ly) in pts {
        let (sx, sy) = (lx * x.scale[0], ly * x.scale[1]);
        let (wx, wy) = (x.origin[0] + c * sx - s * sy, x.origin[1] + s * sx + c * sy);
        let wy = vh - wy;
        a = a.min(wx);
        b = b.min(wy);
        cc = cc.max(wx);
        d = d.max(wy);
    }
    [a, b, cc, d]
}

fn text_under(tr: &Value, depth: usize) -> String {
    let go = go_of(tr);
    for c in with_go(&go, |g| g.comps.clone()).unwrap_or_default() {
        if let Some(t) = with_comp(&c, |k| if let CK::Text(t) = &k.k { Some(t.text.clone()) } else { None }).flatten() {
            let t: String = t.chars().filter(|c| !c.is_control()).collect();
            if !t.trim().is_empty() {
                return t;
            }
        }
    }
    if depth > 0 {
        for ch in with_tr(tr, |t| t.children.clone()).unwrap_or_default() {
            let s = text_under(&ch, depth - 1);
            if !s.is_empty() {
                return s;
            }
        }
    }
    String::new()
}

fn kind_of(vm: &mut Vm, go: &Value) -> Option<&'static str> {
    let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
    for c in &comps {
        let k = with_comp(c, |k| {
            if !k.enabled {
                return None;
            }
            match &k.k {
                CK::Sel(s) if s.interactable => Some(match s.kind {
                    SelKind::Button => "button",
                    SelKind::Slider => "slider",
                    SelKind::Toggle => "toggle",
                    SelKind::InputField => "input",
                    SelKind::Dropdown => "dropdown",
                    SelKind::Scrollbar => "scrollbar",
                }),
                _ => None,
            }
        })
        .flatten();
        if k.is_some() {
            return k;
        }
    }
    if vm.has_handler(go, Ev::Click) || vm.has_handler(go, Ev::Down) {
        return Some("zone");
    }
    None
}

fn collect_node(vm: &mut Vm, tr: &Value, x: Xf, vh: f32, out: &mut Vec<Focusable>) {
    let go = go_of(tr);
    let active = with_go(&go, |g| g.active).unwrap_or(false);
    if !active {
        return;
    }
    if let Some(kind) = kind_of(vm, &go) {
        let r = bbox(&x, vh);
        out.push(Focusable { go: go.clone(), rect: r, label: text_under(tr, 3), kind });
    }
    for ch in with_tr(tr, |t| t.children.clone()).unwrap_or_default() {
        if let Some(cx) = with_tr(&ch, |t| child_xf(&x, t)) {
            collect_node(vm, &ch, cx, vh, out);
        }
    }
}

fn within(inner: &Value, outer: &Value) -> bool {
    let mut cur = inner.clone();
    while !cur.is_null() {
        if same(&cur, outer) {
            return true;
        }
        let tr = with_go(&cur, |g| g.transform.clone()).unwrap_or(Value::Null);
        let p = with_tr(&tr, |t| t.parent.clone()).unwrap_or(Value::Null);
        cur = if p.is_null() { Value::Null } else { go_of(&p) };
    }
    false
}

/// Interactive elements visible on screen and reachable (not hidden by another element).
pub fn collect(vm: &mut Vm) -> Vec<Focusable> {
    let (sw, sh) = (vm.eng.screen.0 as f32, vm.eng.screen.1 as f32);
    let mut roots: Vec<(i32, Value)> = vec![];
    for go in vm.eng.gos.clone() {
        if !with_go(&go, |g| g.active).unwrap_or(false) {
            continue;
        }
        let tr = with_go(&go, |g| g.transform.clone()).unwrap_or(Value::Null);
        if !with_tr(&tr, |t| t.parent.is_null()).unwrap_or(false) {
            continue;
        }
        let mut s = None;
        for c in with_go(&go, |g| g.comps.clone()).unwrap_or_default() {
            with_comp(&c, |k| {
                if let CK::Canvas(cd) = &k.k {
                    s = Some(cd.sorting);
                }
            });
        }
        if let Some(s) = s {
            roots.push((s, tr));
        }
    }
    roots.sort_by_key(|r| r.0);
    let mut all = vec![];
    for (_, tr) in roots {
        let x = xf(vm, &tr);
        collect_node(vm, &tr, x, sh, &mut all);
    }
    let mut out = vec![];
    for f in all {
        let [x0, y0, x1, y1] = f.rect;
        // must have a part visible on screen
        let (vx0, vy0, vx1, vy1) = (x0.max(0.0), y0.max(0.0), x1.min(sw), y1.min(sh));
        if vx1 - vx0 < 2.0 || vy1 - vy0 < 2.0 {
            continue;
        }
        let (cx, cy) = ((vx0 + vx1) / 2.0, (vy0 + vy1) / 2.0);
        // reachable: the element (or one of its graphic descendants / ancestors) is on top at the visible centre
        let hit = raycast(vm, [cx, sh - cy]);
        if hit.is_null() || !(within(&hit, &f.go) || within(&f.go, &hit)) {
            continue;
        }
        out.push(Focusable { rect: [vx0, vy0, vx1, vy1], ..f });
    }
    out
}

/// Active nodes whose rectangle contains the point (top-left space), in draw order: (path, components).
pub fn nodes_at(vm: &mut Vm, px: f32, py: f32) -> Vec<(String, Vec<Value>)> {
    fn walk(tr: &Value, x: Xf, vh: f32, p: (f32, f32), path: &str, out: &mut Vec<(String, Vec<Value>)>) {
        let go = go_of(tr);
        let Some((active, name, comps)) = with_go(&go, |g| (g.active, g.name.clone(), g.comps.clone())) else { return };
        if !active {
            return;
        }
        let path = if path.is_empty() { name } else { format!("{path}/{name}") };
        let r = bbox(&x, vh);
        if p.0 >= r[0] && p.0 <= r[2] && p.1 >= r[1] && p.1 <= r[3] {
            out.push((path.clone(), comps));
        }
        for ch in with_tr(tr, |t| t.children.clone()).unwrap_or_default() {
            if let Some(cx) = with_tr(&ch, |t| child_xf(&x, t)) {
                walk(&ch, cx, vh, p, &path, out);
            }
        }
    }
    let sh = vm.eng.screen.1 as f32;
    let mut roots: Vec<(i32, Value)> = vec![];
    for go in vm.eng.gos.clone() {
        let tr = with_go(&go, |g| g.transform.clone()).unwrap_or(Value::Null);
        if !with_tr(&tr, |t| t.parent.is_null()).unwrap_or(false) || !with_go(&go, |g| g.active).unwrap_or(false) {
            continue;
        }
        let mut s = None;
        for c in with_go(&go, |g| g.comps.clone()).unwrap_or_default() {
            with_comp(&c, |k| {
                if let CK::Canvas(cd) = &k.k {
                    s = Some(cd.sorting);
                }
            });
        }
        if let Some(s) = s {
            roots.push((s, tr));
        }
    }
    roots.sort_by_key(|r| r.0);
    let mut out = vec![];
    for (_, tr) in roots {
        let x = xf(vm, &tr);
        walk(&tr, x, sh, (px, py), "", &mut out);
    }
    out
}

/// Non-empty texts of active nodes whose rectangle touches the screen (no occlusion test).
pub fn visible_texts(vm: &mut Vm) -> Vec<([f32; 4], String)> {
    fn walk(tr: &Value, x: Xf, vh: f32, sw: f32, out: &mut Vec<([f32; 4], String)>) {
        let go = go_of(tr);
        if !with_go(&go, |g| g.active).unwrap_or(false) {
            return;
        }
        for c in with_go(&go, |g| g.comps.clone()).unwrap_or_default() {
            if let Some(t) = with_comp(&c, |k| if let CK::Text(t) = &k.k { k.enabled.then(|| t.text.clone()) } else { None }).flatten() {
                let r = bbox(&x, vh);
                let t: String = t.chars().filter(|c| *c == '\n' || !c.is_control()).collect();
                if !t.trim().is_empty() && r[2] > 0.0 && r[3] > 0.0 && r[0] < sw && r[1] < vh {
                    out.push((r, t));
                }
            }
        }
        for ch in with_tr(tr, |t| t.children.clone()).unwrap_or_default() {
            if let Some(cx) = with_tr(&ch, |t| child_xf(&x, t)) {
                walk(&ch, cx, vh, sw, out);
            }
        }
    }
    let (sw, sh) = (vm.eng.screen.0 as f32, vm.eng.screen.1 as f32);
    let mut out = vec![];
    for go in vm.eng.gos.clone() {
        let tr = with_go(&go, |g| g.transform.clone()).unwrap_or(Value::Null);
        if !with_tr(&tr, |t| t.parent.is_null()).unwrap_or(false) || !with_go(&go, |g| g.active).unwrap_or(false) {
            continue;
        }
        let x = xf(vm, &tr);
        walk(&tr, x, sh, sw, &mut out);
    }
    out
}

fn center(r: &[f32; 4]) -> (f32, f32) {
    ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0)
}

fn gap(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    if a1 < b0 {
        b0 - a1
    } else if b1 < a0 {
        a0 - b1
    } else {
        0.0
    }
}

/// Best neighbour in a direction ('u','d','l','r'); `None` if there is none.
pub fn neighbour(list: &[Focusable], cur: &[f32; 4], dir: char) -> Option<usize> {
    let (px, py) = center(cur);
    let mut best: Option<(f32, usize)> = None;
    for (i, f) in list.iter().enumerate() {
        let (cx, cy) = center(&f.rect);
        let (primary, secondary) = match dir {
            'u' => (py - cy, gap(cur[0], cur[2], f.rect[0], f.rect[2])),
            'd' => (cy - py, gap(cur[0], cur[2], f.rect[0], f.rect[2])),
            'l' => (px - cx, gap(cur[1], cur[3], f.rect[1], f.rect[3])),
            _ => (cx - px, gap(cur[1], cur[3], f.rect[1], f.rect[3])),
        };
        if primary < 2.0 {
            continue;
        }
        let off = if matches!(dir, 'u' | 'd') { (cx - px).abs() } else { (cy - py).abs() };
        let score = primary + 3.0 * secondary + 0.25 * off;
        if best.map(|b| score < b.0).unwrap_or(true) {
            best = Some((score, i));
        }
    }
    best.map(|b| b.1)
}

/// Moves the focus; without a valid focus, takes the first element in reading order.
pub fn navigate(vm: &mut Vm, dir: char) -> Option<Focusable> {
    let list = collect(vm);
    if list.is_empty() {
        vm.eng.focus = Value::Null;
        vm.eng.focus_rect = None;
        return None;
    }
    let cur = list.iter().position(|f| same(&f.go, &vm.eng.focus));
    let next = match cur {
        Some(i) => neighbour(&list, &list[i].rect, dir).unwrap_or(i),
        None => (0..list.len()).min_by(|&a, &b| {
            let (ca, cb) = (center(&list[a].rect), center(&list[b].rect));
            (ca.1, ca.0).partial_cmp(&(cb.1, cb.0)).unwrap()
        })?,
    };
    let f = list[next].clone();
    vm.eng.focus = f.go.clone();
    vm.eng.focus_rect = Some(f.rect);
    Some(f)
}

/// Point (y up, like `Engine::mouse`) where to click to activate the focused element.
pub fn activation_point(vm: &mut Vm) -> Option<(f32, f32)> {
    let sh = vm.eng.screen.1 as f32;
    let f = collect(vm).into_iter().find(|f| same(&f.go, &vm.eng.focus))?;
    let (cx, cy) = center(&f.rect);
    Some((cx, sh - cy))
}

/// Visible buttons that change menu (`swapMenu`), sorted top to bottom: the game's menu column.
pub fn menu_buttons(vm: &mut Vm) -> Vec<Focusable> {
    let mut v: Vec<Focusable> = collect(vm)
        .into_iter()
        .filter(|f| {
            with_go(&f.go, |g| g.comps.clone()).unwrap_or_default().iter().any(|c| {
                with_comp(c, |k| match &k.k {
                    CK::Sel(s) if s.kind == SelKind::Button => with_comp(&s.on_click, |e| matches!(&e.k, CK::Event(ev) if ev.persistent.iter().any(|p| p.method == "swapMenu"))).unwrap_or(false),
                    _ => false,
                })
                .unwrap_or(false)
            })
        })
        .collect();
    v.sort_by(|a, b| (a.rect[1], a.rect[0]).partial_cmp(&(b.rect[1], b.rect[0])).unwrap());
    v
}

/// Previous / next menu (`delta` = -1 / +1): moves the focus to the neighbouring menu button and returns its click point.
pub fn menu_step(vm: &mut Vm, delta: i32) -> Option<(f32, f32)> {
    let list = menu_buttons(vm);
    if list.is_empty() {
        return None;
    }
    let cur = list.iter().position(|f| same(&f.go, &vm.eng.focus)).unwrap_or(vm.eng.menu_idx.min(list.len() - 1));
    let n = list.len() as i32;
    let next = (((cur as i32 + delta) % n) + n) % n;
    vm.eng.menu_idx = next as usize;
    let f = &list[next as usize];
    vm.eng.focus = f.go.clone();
    vm.eng.focus_rect = Some(f.rect);
    let (cx, cy) = center(&f.rect);
    Some((cx, vm.eng.screen.1 as f32 - cy))
}
