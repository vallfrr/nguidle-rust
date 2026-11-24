//! Focus navigation (gamepad / keyboard): list of the really visible and reachable interactive elements,
//! spatial movement (up / down / left / right) and activation (virtual click at the centre).
//! Coordinates: virtual pixels 960x600, origin at the top left (y down).
use crate::input::{canvas_roots, raycast, raycast_points, Ev};
use crate::ui::{view_child_xf, view_xf, xf, Xf};
use crate::unity::*;
use crate::value::*;
use crate::Vm;
use std::sync::atomic::Ordering::Relaxed;

#[derive(Clone)]
pub struct Focusable {
    pub go: Value,
    /// x0, y0, x1, y1
    pub rect: [f32; 4],
    pub label: String,
    pub kind: &'static str,
}

pub(crate) struct ControlCache {
    revision: (u32, u64, f32),
    controls: Vec<Focusable>,
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

fn current_rect(vm: &Vm) -> Option<[f32; 4]> {
    vm.eng.focus_rect?;
    if !active_in_hierarchy(&vm.eng.focus) {
        return None;
    }
    let tr = with_go(&vm.eng.focus, |g| g.transform.clone())?;
    visible_rect(vm, bbox(&view_xf(vm, &tr), vm.eng.screen.1 as f32))
}

pub fn node_rect(vm: &Vm, go: &Value) -> Option<[f32; 4]> {
    if !active_in_hierarchy(go) { return None; }
    let node = with_go(go, |g| g.transform.clone())?;
    visible_rect(vm, bbox(&view_xf(vm, &node), vm.eng.screen.1 as f32))
}

fn visible_rect(vm: &Vm, r: [f32; 4]) -> Option<[f32; 4]> {
    let [x0, y0, x1, y1] = r;
    let r = [x0.max(0.0), y0.max(0.0), x1.min(vm.eng.desktop_layout.width()),
        y1.min(vm.eng.screen.1 as f32)];
    (r[2] - r[0] >= 2.0 && r[3] - r[1] >= 2.0).then_some(r)
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
    let mut selectable = false;
    for c in &comps {
        let k = with_comp(c, |k| {
            selectable |= matches!(k.k, CK::Sel(_));
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
    if !selectable && (vm.has_handler(go, Ev::Click) || vm.has_handler(go, Ev::Down)) {
        return Some("zone");
    }
    None
}

/// Select the reachable control containing a point, in top-left view coordinates.
pub fn select_at(vm: &mut Vm, x: f32, y: f32) -> bool {
    let hit = raycast(vm, [x, vm.eng.screen.1 as f32 - y]);
    let control = collect(vm).into_iter().rev().find(|f| within(&hit, &f.go));
    if let Some(f) = control {
        set_focus(vm, f.go, Some(f.rect));
        true
    } else {
        false
    }
}

fn collect_node(vm: &mut Vm, tr: &Value, x: Xf, vh: f32, out: &mut Vec<Focusable>) {
    let go = go_of(tr);
    if crate::ui::menu_is_parked(vm, &go, &x) {
        return;
    }
    let active = with_go(&go, |g| g.active && !g.port_hidden).unwrap_or(false);
    if !active {
        return;
    }
    if let Some(kind) = kind_of(vm, &go) {
        if let Some(rect) = visible_rect(vm, bbox(&x, vh)) {
            out.push(Focusable { go: go.clone(), rect, label: String::new(), kind });
        }
    }
    for ch in with_tr(tr, |t| t.children.clone()).unwrap_or_default() {
        if let Some(cx) = view_child_xf(vm, &x, &ch) {
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

fn reachable(vm: &mut Vm) -> Vec<Focusable> {
    let sh = vm.eng.screen.1 as f32;
    let roots = canvas_roots(vm);
    let mut all = vec![];
    for (_, tr) in &roots {
        let x = view_xf(vm, tr);
        collect_node(vm, tr, x, sh, &mut all);
    }
    let points: Vec<_> = all.iter().map(|f| {
        let (cx, cy) = center(&f.rect);
        [cx, sh - cy]
    }).collect();
    let hits = raycast_points(vm, &roots, &points);
    all.into_iter().zip(hits).filter_map(|(f, hit)| {
        (!hit.is_null() && (within(&hit, &f.go) || within(&f.go, &hit))).then_some(f)
    }).collect()
}

/// Whether the focused control sits inside a scrollable list (the right stick then scrolls that list).
pub fn in_scroll_view(vm: &Vm) -> bool {
    !vm.eng.focus.is_null() && vm.eng.scrollrects.iter().any(|comp| {
        let go = go_of(comp);
        active_in_hierarchy(&go) && within(&vm.eng.focus, &go) && with_comp(comp, |c| c.enabled).unwrap_or(false)
    })
}

/// Recomputes the focused control's presented rectangle (after its part scrolled).
pub fn refresh_rect(vm: &mut Vm) {
    if !vm.eng.focus.is_null() {
        let go = vm.eng.focus.clone();
        select_control(vm, &go);
    }
}

/// Text of the focused control (logs and tests).
pub fn focused_label(vm: &Vm) -> Option<String> {
    (!vm.eng.focus.is_null()).then(|| text_under(&transform_of(&vm.eng.focus), 3))
}


fn choose_control(vm: &mut Vm, dir: char) -> Option<Focusable> {
    let find = |controls: &[Focusable], focus: &Value| {
        let next = match controls.iter().position(|f| same(&f.go, focus)) {
            Some(i) => neighbour(controls, &controls[i].rect, dir).unwrap_or(i),
            None => (0..controls.len()).min_by(|&a, &b| {
                let (ca, cb) = (center(&controls[a].rect), center(&controls[b].rect));
                (ca.1, ca.0).partial_cmp(&(cb.1, cb.0)).unwrap()
            })?,
        };
        controls.get(next).cloned()
    };
    if !vm.eng.cache_focus_controls {
        let controls = reachable(vm);
        return find(&controls, &vm.eng.focus);
    }
    let revision = (INPUT_EPOCH.load(Relaxed), LAYOUT_EPOCH.load(Relaxed), vm.eng.desktop_layout.width());
    if vm.eng.focus_cache.as_ref().is_none_or(|cache| cache.revision != revision) {
        let controls = reachable(vm);
        vm.eng.focus_cache = Some(ControlCache { revision, controls });
    }
    find(&vm.eng.focus_cache.as_ref()?.controls, &vm.eng.focus)
}

/// Interactive elements visible on screen and reachable (not hidden by another element).
pub fn collect(vm: &mut Vm) -> Vec<Focusable> {
    let mut controls = reachable(vm);
    for control in &mut controls {
        control.label = text_under(&transform_of(&control.go), 3);
    }
    controls
}

/// Active nodes whose rectangle contains the point (top-left space), in draw order: (path, components).
pub fn nodes_at(vm: &mut Vm, px: f32, py: f32) -> Vec<(String, Vec<Value>)> {
    fn walk(vm: &Vm, tr: &Value, x: Xf, vh: f32, p: (f32, f32), path: &str, out: &mut Vec<(String, Vec<Value>)>) {
        let go = go_of(tr);
        let Some((active, name, comps)) = with_go(&go, |g| (g.active && !g.port_hidden, g.name.clone(), g.comps.clone())) else { return };
        if !active {
            return;
        }
        let path = if path.is_empty() { name } else { format!("{path}/{name}") };
        let r = bbox(&x, vh);
        if p.0 >= r[0] && p.0 <= r[2] && p.1 >= r[1] && p.1 <= r[3] {
            out.push((path.clone(), comps));
        }
        for ch in with_tr(tr, |t| t.children.clone()).unwrap_or_default() {
            if let Some(cx) = view_child_xf(vm, &x, &ch) {
                walk(vm, &ch, cx, vh, p, &path, out);
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
        let x = view_xf(vm, &tr);
        walk(vm, &tr, x, sh, (px, py), "", &mut out);
    }
    out
}

/// Non-empty texts of active nodes whose rectangle touches the screen (no occlusion test).
pub fn visible_texts(vm: &mut Vm) -> Vec<([f32; 4], String)> {
    fn walk(vm: &Vm, tr: &Value, x: Xf, vh: f32, sw: f32, out: &mut Vec<([f32; 4], String)>) {
        let go = go_of(tr);
        if !with_go(&go, |g| g.active && !g.port_hidden).unwrap_or(false) {
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
            if let Some(cx) = view_child_xf(vm, &x, &ch) {
                walk(vm, &ch, cx, vh, sw, out);
            }
        }
    }
    let (sw, sh) = (vm.eng.desktop_layout.width(), vm.eng.screen.1 as f32);
    let mut out = vec![];
    for go in vm.eng.gos.clone() {
        let tr = with_go(&go, |g| g.transform.clone()).unwrap_or(Value::Null);
        if !with_tr(&tr, |t| t.parent.is_null()).unwrap_or(false) || !with_go(&go, |g| g.active).unwrap_or(false) {
            continue;
        }
        let x = view_xf(vm, &tr);
        walk(vm, &tr, x, sh, sw, &mut out);
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

/// Applies navigation shared by the native hosts. The host owns click delivery.
pub fn pad_action(vm: &mut Vm, action: char, select_before_click: bool) -> (Option<(f32, f32)>, bool) {
    if vm.dropdown_navigate(action) {
        return (None, false);
    }
    let click = match action {
        'a' => {
            let at = activation_point(vm);
            if at.is_none() && select_before_click {
                navigate(vm, 'd');
                activation_point(vm)
            } else {
                at
            }
        }
        'b' => {
            if !vm.back_from_desktop_picker() {
                clear(vm);
            }
            None
        }
        'p' => menu_step(vm, -1),
        'n' => menu_step(vm, 1),
        'u' | 'd' | 'l' | 'r' => {
            let before = vm.eng.focus.clone();
            navigate(vm, action);
            return (None, !same(&before, &vm.eng.focus));
        }
        _ => None,
    };
    (click, false)
}

/// Moves the focus; without a valid focus, takes the first element in reading order.
pub fn navigate(vm: &mut Vm, dir: char) -> Option<Focusable> {
    let Some(mut f) = choose_control(vm, dir) else {
        clear(vm);
        return None;
    };
    f.label = text_under(&transform_of(&f.go), 3);
    set_focus(vm, f.go.clone(), Some(f.rect));
    Some(f)
}

/// Point (y up, like `Engine::mouse`) where to click to activate the focused element.
pub fn activation_point(vm: &mut Vm) -> Option<(f32, f32)> {
    if current_rect(vm).is_none() {
        let default = crate::save_slots::home_default(vm)?;
        select_control(vm, &default);
    }
    let at = pointer(vm)?;
    let go = vm.eng.focus.clone();
    if kind_of(vm, &go).is_none() {
        return None;
    }
    let hit = raycast(vm, [at.0, at.1]);
    (!hit.is_null() && (within(&hit, &go) || within(&go, &hit))).then_some(at)
}

pub fn pointer(vm: &Vm) -> Option<(f32, f32)> {
    let (x, y) = center(&current_rect(vm)?);
    Some((x, vm.eng.screen.1 as f32 - y))
}

pub(crate) fn select_control(vm: &mut Vm, go: &Value) {
    let position = view_xf(vm, &transform_of(go));
    let rect = visible_rect(vm, bbox(&position, vm.eng.screen.1 as f32));
    set_focus(vm, go.clone(), rect);
}

fn set_focus(vm: &mut Vm, go: Value, rect: Option<[f32; 4]>) {
    if !same(&vm.eng.focus, &go) {
        mark_sel_dirty(&vm.eng.focus);
        mark_dirty(&vm.eng.focus);
        mark_sel_dirty(&go);
        mark_dirty(&go);
    }
    vm.eng.focus = go;
    vm.eng.focus_rect = rect;
}

pub fn clear(vm: &mut Vm) {
    set_focus(vm, Value::Null, None);
}

/// Scroll the focused list, or the closest visible viewport on the current screen.
pub fn scroll(vm: &mut Vm, dx: f32, dy: f32) {
    if vm.dropdown_scroll(-dy.signum()) {
        return;
    }
    let sh = vm.eng.screen.1 as f32;
    let at = pointer(vm);
    let mut lists = Vec::new();
    for comp in &vm.eng.scrollrects {
        let go = go_of(comp);
        if with_go(&go, |g| g.port_hidden).unwrap_or(true) || !active_in_hierarchy(&go) {
            continue;
        }
        let Some(viewport) = with_comp(comp, |c| match &c.k {
            CK::ScrollRect { viewport, content, horizontal, vertical, .. }
                if c.enabled && !content.is_null() &&
                    (dx != 0.0 && *horizontal || dy != 0.0 && *vertical) =>
                Some(if viewport.is_null() { transform_of(&go) } else { viewport.clone() }),
            _ => None,
        }).flatten() else { continue };
        let Some(rect) = visible_rect(vm, bbox(&view_xf(vm, &viewport), sh)) else { continue };
        let p = at.map_or_else(|| center(&rect), |(x, y)| {
            (x.clamp(rect[0] + 0.5, rect[2] - 0.5), (sh - y).clamp(rect[1] + 0.5, rect[3] - 0.5))
        });
        let owned = within(&vm.eng.focus, &go);
        let distance = at.map_or(0.0, |(x, y)| (x - p.0).hypot(sh - y - p.1));
        lists.push((comp.clone(), go, viewport, [p.0, sh - p.1], owned, distance));
    }
    lists.sort_by(|a, b| b.4.cmp(&a.4).then_with(|| a.5.total_cmp(&b.5)));
    let points: Vec<_> = lists.iter().map(|list| list.3).collect();
    let hits = raycast_points(vm, &canvas_roots(vm), &points);
    for ((comp, go, viewport, _, _, _), hit) in lists.into_iter().zip(hits) {
        if !within(&hit, &go) && !within(&hit, &go_of(&viewport)) {
            continue;
        }
        let logical = xf(vm, &viewport);
        let presented = view_xf(vm, &viewport);
        vm.scroll_by(&comp, dx * logical.scale[0] / presented.scale[0].abs().max(1e-6),
            dy * logical.scale[1] / presented.scale[1].abs().max(1e-6));
        return;
    }
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
    set_focus(vm, f.go.clone(), Some(f.rect));
    let (cx, cy) = center(&f.rect);
    Some((cx, vm.eng.screen.1 as f32 - cy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires prepared assets and a late-game save"]
    fn parked_canvas_pruning_preserves_focus_and_hit_targets() {
        let dll = std::env::var("NGU_TEST_DLL").unwrap();
        let assets = std::env::var("NGU_TEST_ASSETS").unwrap();
        let save = std::env::var("NGU_TEST_SAVE").unwrap();
        let mut vm = Vm::new(&dll);
        vm.eng.screen = (960, 600);
        vm.eng.desktop_panels = true;
        vm.load_scene(&assets).unwrap();
        vm.awake_all();
        crate::script::run_steps(&mut vm, &[
            "w,40".into(), "click,Start New Game".into(), "click,PLAY".into(),
            format!("l,{save}"), "w,45".into(), "call,HoverTooltip.offForever".into(),
            "call,HoverTooltip.hideTutorialTooltip".into(),
        ]);
        vm.eng.desktop_layout.set_width(960.0 / (544.0 / 600.0));
        let mut renderer = crate::render::Renderer::new(0, 0);
        let menus = crate::dev::menu_roots(&vm);
        let menu_field = crate::vm::field_ref(&vm, "MenuSwapper", "menuID").unwrap();
        let swapper = vm.eng.user_comps.iter().find(|c| {
            matches!(c, Value::Obj(object) if object.ty == menu_field.0)
        }).unwrap().clone();
        let mut checked = 0;
        for (menu, node) in menus.iter().enumerate() {
            if !with_go(node, |go| !go.port_hidden).unwrap_or(false) {
                continue;
            }
            crate::script::run_steps(&mut vm, &[format!("menu,{menu}")]);
            let active = crate::vm::read_field(&swapper, menu_field).unwrap();
            assert_eq!(active.as_i32(), menu as i32);
            renderer.prepare_layout(&mut vm);
            vm.eng.parked_menu_canvases.clear();
            vm.eng.cache_focus_controls = false;
            let reference = collect(&mut vm);

            crate::ui::cache_menu_canvases(&mut vm);
            assert!(!vm.eng.parked_menu_canvases.is_empty());
            let controls = collect(&mut vm);
            assert_eq!(controls.len(), reference.len());
            for (actual, expected) in controls.iter().zip(&reference) {
                assert!(same(&actual.go, &expected.go));
                assert_eq!(actual.rect, expected.rect);
            }
            checked += 1;
            if controls.is_empty() {
                continue;
            }
            let run = |vm: &mut Vm, cached| {
                clear(vm);
                vm.eng.cache_focus_controls = cached;
                vm.eng.focus_cache = None;
                ['d', 'r', 'r', 'l'].map(|direction| navigate(vm, direction).unwrap().rect)
            };
            assert_eq!(run(&mut vm, false), run(&mut vm, true));
        }
        assert!(checked >= 50, "only checked {checked} screens");
        println!("Checked focus hit targets in {checked} menus");
    }
}
