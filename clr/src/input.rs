//! Pointer input (mouse / touch): UI raycast, hover, press, click, drag and drop, Slider, Toggle.
use crate::ui::{view_child_xf, view_width, view_xf, xf, Xf};
use crate::unity::*;
use crate::value::*;
use crate::Vm;

#[derive(Default)]
pub struct PointerState {
    pub hover: Value,
    hover_focus: Value,
    hover_on_save_page: bool,
    pub pressed: Value,
    pub drag: Value,
    pub down_pos: (f32, f32),
    pub dragging: bool,
    pub last_pos: (f32, f32),
    pub slider: Value,
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

pub struct Ped {
    pub pos: [f32; 2],
    pub delta: [f32; 2],
    pub press_pos: [f32; 2],
    pub button: i32,
    pub click_count: i32,
    pub dragging: bool,
    pub press: Value,
    pub drag: Value,
}

pub fn new_ped(vm: &mut Vm, p: Ped) -> Value {
    let id = vm.type_id("UnityEngine.EventSystems.PointerEventData");
    Value::Obj(Obj::new(id, 0, Ext::Native(Box::new(p))))
}

pub fn with_ped<R>(v: &Value, f: impl FnOnce(&Ped) -> R) -> Option<R> {
    if let Value::Obj(o) = v {
        if let Ext::Native(b) = &*o.x.borrow() {
            if let Some(p) = b.downcast_ref::<Ped>() {
                return Some(f(p));
            }
        }
    }
    None
}

struct HitRect {
    origin: [f32; 2],
    scale: [f32; 2],
    bounds: [f32; 4],
    sin: f32,
    cos: f32,
}

impl HitRect {
    fn new(x: &Xf) -> Self {
        let [sx, sy] = x.scale.map(|scale| if scale == 0.0 { 1.0 } else { scale });
        let (x0, y0) = (-x.pivot[0] * x.w, -x.pivot[1] * x.h);
        let (sin, cos) = if x.rot == 0.0 { (0.0, 1.0) } else { (-x.rot).sin_cos() };
        Self {
            origin: x.origin,
            scale: [sx, sy],
            bounds: [x0, y0, x0 + x.w, y0 + x.h],
            sin,
            cos,
        }
    }

    fn contains(&self, p: [f32; 2]) -> bool {
        let [x0, y0, x1, y1] = self.bounds;
        let (dx, dy) = (p[0] - self.origin[0], p[1] - self.origin[1]);
        let lx = (self.cos * dx - self.sin * dy) / self.scale[0];
        let ly = (self.sin * dx + self.cos * dy) / self.scale[1];
        lx >= x0 && lx <= x1 && ly >= y0 && ly <= y1
    }

    // The common axis-aligned case can narrow a batch by x before running the exact test.
    fn x_span(&self) -> [f32; 2] {
        if self.sin != 0.0 || self.cos != 1.0 {
            return [f32::NEG_INFINITY, f32::INFINITY];
        }
        let a = self.origin[0] + self.bounds[0] * self.scale[0];
        let b = self.origin[0] + self.bounds[2] * self.scale[0];
        if !(a.is_finite() && b.is_finite()) {
            return [f32::NEG_INFINITY, f32::INFINITY];
        }
        // Roundoff at the edge must never exclude a point the inverse transform would hit.
        let margin = 8.0 * f32::EPSILON * a.abs().max(b.abs()).max(self.origin[0].abs()).max(1.0);
        [a.min(b) - margin, a.max(b) + margin]
    }
}

struct Probe {
    point: [f32; 2],
    index: usize,
    hit: Value,
}

pub(crate) fn offline_screen_visible(vm: &Vm) -> bool {
    // The built-in OfflinePanel covers the game until "On To The Game!".
    if !vm.eng.desktop_panels {
        return false;
    }
    let Some(panel) = vm.eng.keys.get("level0:1165").filter(|go| active_in_hierarchy(go)) else {
        return false;
    };
    // Closing the splash parks its transform outside the canvas. It stays active.
    let rect = xf(vm, &transform_of(panel));
    let width = rect.w * rect.scale[0].abs();
    let height = rect.h * rect.scale[1].abs();
    let left = rect.origin[0] - rect.pivot[0] * width;
    let bottom = rect.origin[1] - rect.pivot[1] * height;
    left + width > 0.0 && left < vm.eng.screen.0 as f32
        && bottom + height > 0.0 && bottom < vm.eng.screen.1 as f32
}

fn raycast_node(
    vm: &Vm,
    tr: &Value,
    x: Xf,
    probes: &mut [Probe],
    masks: &mut Vec<HitRect>,
    remaining: &mut usize,
) {
    let go = go_of(tr);
    if crate::ui::menu_is_parked(vm, &go, &x) {
        return;
    }
    let Some((graphic, mask)) = with_go(&go, |g| {
        (g.active && !g.port_hidden && !(g.name == "Hover Tooltip" && offline_screen_visible(vm))).then(|| {
            let mut graphic = false;
            let mut mask = false;
            for c in &g.comps {
                with_comp(c, |k| {
                    if !k.enabled {
                        return;
                    }
                    match &k.k {
                        CK::Image(i) | CK::RawImage(i) => graphic |= i.raycast && i.color[3] > 0.0,
                        CK::Text(t) => graphic |= t.raycast,
                        CK::Mask { .. } => mask = true,
                        _ => {}
                    }
                });
            }
            (graphic, mask)
        })
    }).flatten() else { return };

    // A later child draws over its siblings and its parent.
    if mask {
        let rect = HitRect::new(&x);
        if !probes.iter().any(|p| p.hit.is_null() && rect.contains(p.point)) {
            return;
        }
        masks.push(rect);
    }
    let children = with_tr(tr, |t| t.children.clone()).unwrap_or_default();
    for child in children.iter().rev() {
        if *remaining == 0 {
            break;
        }
        if !with_go(&go_of(child), |g| g.active && !g.port_hidden).unwrap_or(false) {
            continue;
        }
        if let Some(cx) = view_child_xf(vm, &x, child) {
            raycast_node(vm, child, cx, probes, masks, remaining);
        }
    }
    if mask {
        masks.pop();
    }
    if graphic && *remaining > 0 {
        let width = view_width(vm, tr);
        let rect = HitRect::new(&x);
        let candidates = if probes.len() > 8 {
            let [lo, hi] = rect.x_span();
            let start = probes.partition_point(|p| p.point[0] < lo);
            let end = start + probes[start..].partition_point(|p| p.point[0] <= hi && p.point[0] < width);
            &mut probes[start..end]
        } else {
            probes
        };
        for p in candidates.iter_mut().filter(|p| p.hit.is_null() && p.point[0] < width) {
            if rect.contains(p.point) && masks.iter().all(|m| m.contains(p.point)) {
                p.hit = go.clone();
                *remaining -= 1;
            }
        }
    }
}

pub(crate) fn canvas_roots(vm: &Vm) -> Vec<(i32, Value)> {
    let choosing_save = crate::save_slots::choosing_save(vm);
    vm.eng.canvases.iter().filter(|(_, tr)| {
        let go = go_of(tr);
        (!choosing_save || crate::save_slots::owns(&go)) &&
            with_go(&go, |g| g.active && !g.port_hidden).unwrap_or(false) &&
            with_tr(tr, |t| t.parent.is_null()).unwrap_or(false)
    }).cloned().collect()
}

pub(crate) fn raycast_points(vm: &Vm, roots: &[(i32, Value)], points: &[[f32; 2]]) -> Vec<Value> {
    let width = vm.eng.desktop_layout.width().max(vm.eng.screen.0 as f32);
    let height = vm.eng.screen.1 as f32;
    let mut remaining = 0;
    let mut probes: Vec<_> = points.iter().enumerate().map(|(index, &point)| {
        let valid = (0.0..width).contains(&point[0]) && (0.0..height).contains(&point[1]);
        remaining += usize::from(valid);
        Probe { point: if valid { point } else { [f32::INFINITY; 2] }, index, hit: Value::Null }
    }).collect();
    if probes.len() > 8 {
        probes.sort_unstable_by(|a, b| a.point[0].total_cmp(&b.point[0]));
    }
    let mut masks = Vec::new();
    for (_, root) in roots.iter().rev() {
        if remaining == 0 {
            break;
        }
        raycast_node(vm, root, view_xf(vm, root), &mut probes, &mut masks, &mut remaining);
    }
    let mut hits = vec![Value::Null; points.len()];
    for p in probes {
        hits[p.index] = p.hit;
    }
    hits
}

pub fn raycast(vm: &Vm, p: [f32; 2]) -> Value {
    let width = vm.eng.desktop_layout.width().max(vm.eng.screen.0 as f32);
    if !(0.0..width).contains(&p[0]) || !(0.0..vm.eng.screen.1 as f32).contains(&p[1]) {
        return Value::Null;
    }
    raycast_points(vm, &canvas_roots(vm), &[p]).pop().unwrap_or_default()
}

#[derive(Clone, Copy, PartialEq)]
pub enum Ev {
    Enter,
    Exit,
    Down,
    Up,
    Click,
    Drag,
    Drop,
    BeginDrag,
    EndDrag,
}

fn ev_id(e: Ev) -> i32 {
    match e {
        Ev::Enter => 0,
        Ev::Exit => 1,
        Ev::Down => 2,
        Ev::Up => 3,
        Ev::Click => 4,
        Ev::Drag => 5,
        Ev::Drop => 6,
        Ev::BeginDrag => 13,
        Ev::EndDrag => 14,
    }
}

fn ev_method(e: Ev) -> &'static str {
    match e {
        Ev::Enter => "OnPointerEnter",
        Ev::Exit => "OnPointerExit",
        Ev::Down => "OnPointerDown",
        Ev::Up => "OnPointerUp",
        Ev::Click => "OnPointerClick",
        Ev::Drag => "OnDrag",
        Ev::Drop => "OnDrop",
        Ev::BeginDrag => "OnBeginDrag",
        Ev::EndDrag => "OnEndDrag",
    }
}

impl Vm {
    /// Does the GO have a handler for this event?
    pub fn has_handler(&mut self, go: &Value, e: Ev) -> bool {
        let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
        for c in comps {
            let kind = with_comp(&c, |k| {
                if !k.enabled {
                    return 0;
                }
                match &k.k {
                    CK::User => 1,
                    CK::EventTrigger(l) => {
                        if l.iter().any(|(id, _)| *id == ev_id(e)) {
                            2
                        } else {
                            0
                        }
                    }
                    CK::ScrollRect { .. } => {
                        if matches!(e, Ev::Drag | Ev::BeginDrag | Ev::EndDrag) {
                            4
                        } else {
                            0
                        }
                    }
                    CK::Sel(s) => match (s.kind, e) {
                        (SelKind::Button, Ev::Click) | (SelKind::Toggle, Ev::Click) | (SelKind::InputField, Ev::Click) | (SelKind::Dropdown, Ev::Click) => 3,
                        (SelKind::Slider, Ev::Down) | (SelKind::Slider, Ev::Drag) => 3,
                        _ => 0,
                    },
                    _ => 0,
                }
            })
            .unwrap_or(0);
            match kind {
                1 => {
                    if let Value::Obj(o) = &c {
                        if self.find_method(o.ty, ev_method(e), 2).is_some() {
                            return true;
                        }
                    }
                }
                2 | 3 | 4 => return true,
                _ => {}
            }
        }
        false
    }

    /// Finds the first ancestor (self included) handling the event.
    pub fn find_ev_handler(&mut self, go: &Value, e: Ev) -> Value {
        let mut cur = go.clone();
        while !cur.is_null() {
            if self.has_handler(&cur, e) {
                return cur;
            }
            let tr = with_go(&cur, |g| g.transform.clone()).unwrap_or(Value::Null);
            let parent = with_tr(&tr, |t| t.parent.clone()).unwrap_or(Value::Null);
            cur = if parent.is_null() { Value::Null } else { go_of(&parent) };
        }
        Value::Null
    }

    pub fn dispatch(&mut self, go: &Value, e: Ev, ped: &Value) {
        if e == Ev::Click && with_ped(ped, |p| p.button == 0).unwrap_or(true)
            && crate::save_slots::click(self, go, self.eng.mouse.0)
        {
            return;
        }
        let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
        for c in comps {
            let k = with_comp(&c, |k| {
                if !k.enabled {
                    return (0, Value::Null);
                }
                match &k.k {
                    CK::User => (1, Value::Null),
                    CK::EventTrigger(l) => (2, l.iter().filter(|(id, _)| *id == ev_id(e)).map(|(_, v)| v.clone()).next().unwrap_or(Value::Null)),
                    CK::Sel(s) => (3, s.on_click.clone()),
                    CK::ScrollRect { .. } => (4, Value::Null),
                    _ => (0, Value::Null),
                }
            })
            .unwrap_or((0, Value::Null));
            match k.0 {
                1 => {
                    if let Value::Obj(o) = &c {
                        if let Some(m) = self.find_method(o.ty, ev_method(e), 2) {
                            if let Err(x) = self.invoke(m, vec![c.clone(), ped.clone()]) {
                                self.report_exc(ev_method(e), &c, &x);
                            }
                        }
                    }
                }
                2 => {
                    if !k.1.is_null() {
                        self.invoke_event(&k.1, vec![ped.clone()]);
                    }
                }
                4 => {
                    if e == Ev::Drag && with_ped(ped, |p| p.button == 0).unwrap_or(true) {
                        let d = with_ped(ped, |p| p.delta).unwrap_or([0.0; 2]);
                        self.scroll_by(&c, d[0], d[1]);
                    }
                }
                3 => {
                    let (kind, inter, on_change) = with_comp(&c, |k| match &k.k {
                        CK::Sel(s) => (s.kind, s.interactable, s.on_change.clone()),
                        _ => (SelKind::Scrollbar, false, Value::Null),
                    })
                    .unwrap_or((SelKind::Scrollbar, false, Value::Null));
                    if !inter {
                        continue;
                    }
                    let left = with_ped(ped, |p| p.button == 0).unwrap_or(true);
                    if e == Ev::Click && left {
                        match kind {
                            SelKind::Button => {
                                if !k.1.is_null() {
                                    self.invoke_event(&k.1, vec![]);
                                }
                            }
                            SelKind::InputField => {
                                let (txt, max, ct) = with_comp(&c, |k| match &k.k {
                                    CK::Sel(s) => (s.text.clone(), s.char_limit, s.content_type),
                                    _ => (String::new(), 0, 0),
                                })
                                .unwrap_or_default();
                                self.eng.text_req = Some(TextReq { comp: c.clone(), text: txt, max, numeric: ct == 2 || ct == 3, pending: true });
                            }
                            SelKind::Dropdown => self.open_dropdown(&c),
                            SelKind::Toggle => {
                                let now = with_comp(&c, |k| match &mut k.k {
                                    CK::Sel(s) => {
                                        s.is_on = !s.is_on;
                                        s.is_on
                                    }
                                    _ => false,
                                })
                                .unwrap_or(false);
                                if !on_change.is_null() {
                                    self.invoke_event(&on_change, vec![Value::bool(now)]);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn mk_ped(&mut self, pos: (f32, f32)) -> Value {
        let ps = &self.eng.ptr;
        let p = Ped {
            pos: [pos.0, pos.1],
            delta: [pos.0 - ps.last_pos.0, pos.1 - ps.last_pos.1],
            press_pos: [ps.down_pos.0, ps.down_pos.1],
            button: self.eng.mouse_button,
            click_count: 1,
            dragging: ps.dragging,
            press: ps.pressed.clone(),
            drag: ps.drag.clone(),
        };
        new_ped(self, p)
    }

    fn set_hover(&mut self, new: Value, pos: (f32, f32)) {
        let old = self.eng.ptr.hover.clone();
        if same(&old, &new) {
            return;
        }
        let ancestry = |mut go: Value| {
            let mut branch = Vec::new();
            while !go.is_null() {
                let tr = with_go(&go, |g| g.transform.clone()).unwrap_or_default();
                branch.push(go);
                let parent = with_tr(&tr, |t| t.parent.clone()).unwrap_or_default();
                go = go_of(&parent);
            }
            branch
        };
        let mut exits = ancestry(old);
        let mut enters = ancestry(new.clone());
        while exits.last().zip(enters.last()).is_some_and(|(a, b)| same(a, b)) {
            exits.pop();
            enters.pop();
        }
        let ped = self.mk_ped(pos);
        for go in exits {
            if self.has_handler(&go, Ev::Exit) {
                self.dispatch(&go, Ev::Exit, &ped);
            }
        }
        self.eng.ptr.hover = new;
        for go in enters {
            if self.has_handler(&go, Ev::Enter) {
                self.dispatch(&go, Ev::Enter, &ped);
            }
        }
    }

    /// To call once per frame (before Update).
    pub fn pointer_update(&mut self) {
        let (h0, p0) = (self.eng.ptr.hover.clone(), self.eng.ptr.pressed.clone());
        self.pointer_update_inner();
        // ColorTint tint of Selectables: redraw those whose hover/press state changed
        for (a, b) in [(h0, self.eng.ptr.hover.clone()), (p0, self.eng.ptr.pressed.clone())] {
            if !same(&a, &b) {
                [a, b].iter().filter(|v| !v.is_null()).for_each(mark_sel_dirty);
            }
        }
    }

    /// Scroll views can follow a held pointer between game ticks without dispatching
    /// the scripted drag events used by inventory items and cards.
    pub fn pointer_holds_scroll_view(&self) -> bool {
        self.eng.mouse_down && with_go(&self.eng.ptr.drag, |go| {
            go.comps.iter().any(|component| {
                with_comp(component, |c| matches!(c.k, CK::ScrollRect { .. })).unwrap_or(false)
            })
        }).unwrap_or(false)
    }

    fn pointer_update_inner(&mut self) {
        if self.dropdown_pointer() {
            return;
        }
        let pos = self.eng.mouse;
        let down = self.eng.mouse_down;
        let (edge_d, edge_u) = (self.eng.mouse_down_edge, self.eng.mouse_up_edge);
        if !down && !edge_d && !edge_u && self.eng.ptr.pressed.is_null() && self.eng.ptr.hover.is_null() && !self.eng.hover_always {
            return;
        }
        let p = [pos.0, pos.1];
        // A sleeping gamepad keeps its virtual pointer on the selected control.
        // Revalidate real clicks and recompute hover when focus or its position moves.
        let on_save_page = crate::save_slots::is_open(self) || self.at_desktop_start();
        let stable_pad = self.eng.pad_hover && !self.eng.focus.is_null()
            && same(&self.eng.focus, &self.eng.ptr.hover_focus)
            && pos == self.eng.ptr.last_pos
            && on_save_page == self.eng.ptr.hover_on_save_page
            && !offline_screen_visible(self)
            && active_in_hierarchy(&self.eng.ptr.hover);
        let hit = if edge_d || edge_u || (!down && self.eng.hover_always && !stable_pad) {
            raycast(self, p)
        } else if stable_pad {
            self.eng.ptr.hover.clone()
        } else {
            Value::Null
        };
        if edge_d {
            self.set_hover(hit.clone(), pos);
            self.eng.ptr.down_pos = pos;
            self.eng.ptr.dragging = false;
            let ped = self.mk_ped(pos);
            let h = self.find_ev_handler(&hit, Ev::Down);
            if !h.is_null() {
                self.dispatch(&h, Ev::Down, &ped);
            }
            // the "pressed" element: click or drag handler
            let mut press = self.find_ev_handler(&hit, Ev::Click);
            if press.is_null() {
                press = h.clone();
            }
            let drag_h = self.find_ev_handler(&hit, Ev::Drag);
            self.eng.ptr.pressed = press;
            self.eng.ptr.drag = drag_h;
            // slider ?
            self.eng.ptr.slider = Value::Null;
            let sl = self.find_ev_handler(&hit, Ev::Drag);
            if !sl.is_null() && self.eng.mouse_button == 0 && self.is_slider(&sl) {
                self.eng.ptr.slider = sl;
            }
        }
        if down && !edge_d {
            let ps = &self.eng.ptr;
            let moved = ((pos.0 - ps.down_pos.0).powi(2) + (pos.1 - ps.down_pos.1).powi(2)).sqrt() > 5.0;
            if moved && !self.eng.ptr.dragging && !self.eng.ptr.drag.is_null() && !self.is_slider(&self.eng.ptr.drag.clone()) {
                self.eng.ptr.dragging = true;
                let ped = self.mk_ped(pos);
                let d = self.eng.ptr.drag.clone();
                self.dispatch(&d, Ev::BeginDrag, &ped);
            }
            if self.eng.ptr.dragging {
                let ped = self.mk_ped(pos);
                let d = self.eng.ptr.drag.clone();
                self.dispatch(&d, Ev::Drag, &ped);
            }
        }
        if down && !self.eng.ptr.slider.is_null() {
            let s = self.eng.ptr.slider.clone();
            self.slider_drag(&s, p);
        }
        if edge_u {
            let ped = self.mk_ped(pos);
            let pressed = self.eng.ptr.pressed.clone();
            if !pressed.is_null() {
                self.dispatch(&pressed, Ev::Up, &ped);
                let h_click = self.find_ev_handler(&hit, Ev::Click);
                if same(&h_click, &pressed) && !self.eng.ptr.dragging {
                    self.dispatch(&pressed, Ev::Click, &ped);
                }
            }
            if self.eng.ptr.dragging {
                let h_drop = self.find_ev_handler(&hit, Ev::Drop);
                if !h_drop.is_null() {
                    self.dispatch(&h_drop, Ev::Drop, &ped);
                }
                let d = self.eng.ptr.drag.clone();
                self.dispatch(&d, Ev::EndDrag, &ped);
            }
            self.eng.ptr = PointerState { last_pos: pos, hover: self.eng.ptr.hover.clone(), ..Default::default() };
            if !self.eng.hover_always {
                self.set_hover(Value::Null, pos); // touch: the finger leaves the screen
            }
        } else if self.eng.hover_always && !down {
            self.set_hover(hit, pos);
        }
        self.eng.ptr.hover_focus = if self.eng.pad_hover {
            self.eng.focus.clone()
        } else {
            Value::Null
        };
        self.eng.ptr.hover_on_save_page = on_save_page;
        self.eng.ptr.last_pos = pos;
    }

    fn is_slider(&mut self, go: &Value) -> bool {
        let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
        comps.iter().any(|c| with_comp(c, |k| matches!(&k.k, CK::Sel(s) if s.kind == SelKind::Slider)).unwrap_or(false))
    }

    fn slider_drag(&mut self, go: &Value, p: [f32; 2]) {
        let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
        let tr = with_go(go, |g| g.transform.clone()).unwrap_or(Value::Null);
        let x = view_xf(self, &tr);
        let (dx, dy) = (p[0] - x.origin[0], p[1] - x.origin[1]);
        let (c, s) = ((-x.rot).cos(), (-x.rot).sin());
        let lx = (c * dx - s * dy) / x.scale[0].max(1e-6);
        let ly = (s * dx + c * dy) / x.scale[1].max(1e-6);
        for cm in comps {
            let (is_slider, dir, min, max, whole, old, on_change) = match with_comp(&cm, |k| match &k.k {
                CK::Sel(s) if s.kind == SelKind::Slider && s.interactable => Some((true, s.direction, s.min, s.max, s.whole, s.value, s.on_change.clone())),
                _ => None,
            })
            .flatten()
            {
                Some(v) => v,
                None => continue,
            };
            let _ = is_slider;
            let (x0, y0) = (-x.pivot[0] * x.w, -x.pivot[1] * x.h);
            let n = match dir {
                0 => (lx - x0) / x.w,
                1 => 1.0 - (lx - x0) / x.w,
                2 => (ly - y0) / x.h,
                _ => 1.0 - (ly - y0) / x.h,
            }
            .clamp(0.0, 1.0);
            let mut v = min + (max - min) * n;
            if whole {
                v = v.round();
            }
            if (v - old).abs() > 1e-6 {
                with_comp(&cm, |k| {
                    if let CK::Sel(s) = &mut k.k {
                        s.value = v;
                    }
                });
                crate::unity_api::slider_visuals(self, &cm);
                if !on_change.is_null() {
                    self.invoke_event(&on_change, vec![Value::F32(v)]);
                }
            }
        }
    }
}

impl Vm {
    /// Scrolls the content of a ScrollRect (delta in screen pixels, y up).
    pub fn scroll_by(&mut self, sr: &Value, dx: f32, dy: f32) {
        let (content, viewport, horizontal, vertical, hscroll, vscroll) = match with_comp(sr, |k| match &k.k {
            CK::ScrollRect { content, viewport, horizontal, vertical, hscroll, vscroll, .. } => Some((content.clone(), viewport.clone(), *horizontal, *vertical, hscroll.clone(), vscroll.clone())),
            _ => None,
        })
        .flatten()
        {
            Some(v) => v,
            None => return,
        };
        if content.is_null() {
            return;
        }
        let vp_tr = if viewport.is_null() { transform_of(sr) } else { viewport };
        let vx = xf(self, &vp_tr);
        let cx = xf(self, &content);
        let (mut mx, mut my) = (0.0f32, 0.0f32);
        if vertical {
            let (v_top, v_h) = (vx.origin[1] + (1.0 - vx.pivot[1]) * vx.h * vx.scale[1], vx.h * vx.scale[1]);
            let (c_top, c_h) = (cx.origin[1] + (1.0 - cx.pivot[1]) * cx.h * cx.scale[1], cx.h * cx.scale[1]);
            let want = c_top + dy;
            let new_top = if c_h > v_h { want.clamp(v_top, v_top + (c_h - v_h)) } else { c_top };
            my = new_top - c_top;
        }
        if horizontal {
            let (v_left, v_w) = (vx.origin[0] - vx.pivot[0] * vx.w * vx.scale[0], vx.w * vx.scale[0]);
            let (c_left, c_w) = (cx.origin[0] - cx.pivot[0] * cx.w * cx.scale[0], cx.w * cx.scale[0]);
            let want = c_left + dx;
            let new_left = if c_w > v_w { want.clamp(v_left - (c_w - v_w), v_left) } else { c_left };
            mx = new_left - c_left;
        }
        if mx == 0.0 && my == 0.0 {
            return;
        }
        let (ls, ps) = with_tr(&content, |t| (t.scale, [cx.scale[0] / t.scale[0].max(1e-6), cx.scale[1] / t.scale[1].max(1e-6)])).unwrap_or(([1.0; 3], [1.0; 2]));
        let _ = ls;
        with_tr(&content, |t| {
            t.apos[0] += mx / ps[0];
            t.apos[1] += my / ps[1];
        });
        mark_dirty(&content);
        invalidate_input();
        // scroll bars
        let cx2 = xf(self, &content);
        if vertical && !vscroll.is_null() {
            let (v_top, v_h) = (vx.origin[1] + (1.0 - vx.pivot[1]) * vx.h * vx.scale[1], vx.h * vx.scale[1]);
            let (c_top, c_h) = (cx2.origin[1] + (1.0 - cx2.pivot[1]) * cx2.h * cx2.scale[1], cx2.h * cx2.scale[1]);
            let norm = if c_h > v_h { 1.0 - (c_top - v_top) / (c_h - v_h) } else { 1.0 };
            self.set_scrollbar(&vscroll, norm, (v_h / c_h.max(1e-6)).min(1.0));
        }
        if horizontal && !hscroll.is_null() {
            let (v_left, v_w) = (vx.origin[0] - vx.pivot[0] * vx.w * vx.scale[0], vx.w * vx.scale[0]);
            let (c_left, c_w) = (cx2.origin[0] - cx2.pivot[0] * cx2.w * cx2.scale[0], cx2.w * cx2.scale[0]);
            let norm = if c_w > v_w { (v_left - c_left) / (c_w - v_w) } else { 0.0 };
            self.set_scrollbar(&hscroll, norm, (v_w / c_w.max(1e-6)).min(1.0));
        }
    }

    fn set_scrollbar(&mut self, sb: &Value, norm: f32, size: f32) {
        let dir = with_comp(sb, |k| match &k.k {
            CK::Sel(s) => s.direction,
            _ => 0,
        })
        .unwrap_or(0);
        let v = if dir == 3 || dir == 1 { 1.0 - norm } else { norm };
        with_comp(sb, |k| {
            if let CK::Sel(s) = &mut k.k {
                s.value = v.clamp(0.0, 1.0);
                s.size = size;
            }
        });
        crate::unity_api::slider_visuals(self, sb);
    }

    /// A scroll bar changed (game code): repositions the associated ScrollRect.
    pub fn scrollbar_moved(&mut self, sb: &Value) {
        let rects = self.eng.scrollrects.clone();
        for sr in rects {
            let (content, viewport, hs, vs, hz, vt) = match with_comp(&sr, |k| match &k.k {
                CK::ScrollRect { content, viewport, hscroll, vscroll, horizontal, vertical, .. } => Some((content.clone(), viewport.clone(), hscroll.clone(), vscroll.clone(), *horizontal, *vertical)),
                _ => None,
            })
            .flatten()
            {
                Some(v) => v,
                None => continue,
            };
            let is_v = same(&vs, sb);
            let is_h = same(&hs, sb);
            if !(is_v || is_h) || content.is_null() {
                continue;
            }
            let (dir, val) = with_comp(sb, |k| match &k.k {
                CK::Sel(s) => (s.direction, s.value),
                _ => (0, 0.0),
            })
            .unwrap_or((0, 0.0));
            let norm = if dir == 3 || dir == 1 { 1.0 - val } else { val };
            let vp_tr = if viewport.is_null() { transform_of(&sr) } else { viewport };
            let vx = xf(self, &vp_tr);
            let cx = xf(self, &content);
            if is_v && vt {
                let (v_top, v_h) = (vx.origin[1] + (1.0 - vx.pivot[1]) * vx.h * vx.scale[1], vx.h * vx.scale[1]);
                let (c_top, c_h) = (cx.origin[1] + (1.0 - cx.pivot[1]) * cx.h * cx.scale[1], cx.h * cx.scale[1]);
                if c_h > v_h {
                    let want_top = v_top + (1.0 - norm) * (c_h - v_h);
                    let ps = cx.scale[1] / with_tr(&content, |t| t.scale[1]).unwrap_or(1.0).max(1e-6);
                    with_tr(&content, |t| t.apos[1] += (want_top - c_top) / ps);
                    mark_dirty(&content);
                    invalidate_input();
                }
            }
            if is_h && hz {
                let (v_left, v_w) = (vx.origin[0] - vx.pivot[0] * vx.w * vx.scale[0], vx.w * vx.scale[0]);
                let (c_left, c_w) = (cx.origin[0] - cx.pivot[0] * cx.w * cx.scale[0], cx.w * cx.scale[0]);
                if c_w > v_w {
                    let want_left = v_left - norm * (c_w - v_w);
                    let ps = cx.scale[0] / with_tr(&content, |t| t.scale[0]).unwrap_or(1.0).max(1e-6);
                    with_tr(&content, |t| t.apos[0] += (want_left - c_left) / ps);
                    mark_dirty(&content);
                    invalidate_input();
                }
            }
        }
    }

    /// Text typed by the user into an InputField (system keyboard).
    pub fn input_commit(&mut self, comp: &Value, text: &str) {
        let (max, ct, on_change, on_end, tc, ph) = with_comp(comp, |k| match &k.k {
            CK::Sel(s) => (s.char_limit, s.content_type, s.on_change.clone(), s.on_end.clone(), s.text_component.clone(), s.placeholder.clone()),
            _ => (0, 0, Value::Null, Value::Null, Value::Null, Value::Null),
        })
        .unwrap_or((0, 0, Value::Null, Value::Null, Value::Null, Value::Null));
        let mut t: String = match ct {
            2 => text.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect(),
            3 => text.chars().filter(|c| c.is_ascii_digit() || *c == '-' || *c == '.').collect(),
            _ => text.to_string(),
        };
        if max > 0 {
            t = t.chars().take(max as usize).collect();
        }
        with_comp(comp, |k| {
            if let CK::Sel(s) = &mut k.k {
                s.text = t.clone();
            }
        });
        // display: text or placeholder
        if !tc.is_null() {
            with_comp(&tc, |k| {
                if let CK::Text(x) = &mut k.k {
                    x.text = t.clone();
                }
            });
            mark_dirty(&tc);
        }
        if !ph.is_null() {
            with_comp(&ph, |k| k.enabled = t.is_empty());
            mark_dirty(&ph);
        }
        if !on_change.is_null() {
            self.invoke_event(&on_change, vec![Value::str(&t)]);
        }
        if !on_end.is_null() {
            self.invoke_event(&on_end, vec![Value::str(&t)]);
        }
    }
}

/// Unity `KeyCode` values used by the game, by the lower-case names of `Input.GetKey(string)`.
pub fn key_by_name(n: &str) -> i32 {
    match n.to_ascii_lowercase().as_str() {
        "left shift" => 304,
        "right shift" => 303,
        "left ctrl" => 306,
        "right ctrl" => 305,
        "left alt" => 308,
        "right alt" => 307,
        "space" => 32,
        "return" | "enter" => 13,
        "escape" => 27,
        "tab" => 9,
        "backspace" => 8,
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => s.as_bytes()[0] as i32,
        _ => 0,
    }
}

impl Vm {
    /// Mouse wheel over a ScrollRect (Unity: content moves by `delta * scrollSensitivity` pixels).
    pub fn scroll_wheel(&mut self, notches: f32) {
        if notches == 0.0 || self.dropdown_scroll(notches) {
            return;
        }
        let pos = self.eng.mouse;
        let hit = raycast(self, [pos.0, pos.1]);
        let mut cur = hit;
        while !cur.is_null() {
            for c in with_go(&cur, |g| g.comps.clone()).unwrap_or_default() {
                if let Some(sens) = with_comp(&c, |k| if let CK::ScrollRect { sens, .. } = &k.k { Some(*sens) } else { None }).flatten() {
                    self.scroll_by(&c, 0.0, -notches * sens);
                    return;
                }
            }
            let tr = with_go(&cur, |g| g.transform.clone()).unwrap_or(Value::Null);
            let parent = with_tr(&tr, |t| t.parent.clone()).unwrap_or(Value::Null);
            cur = if parent.is_null() { Value::Null } else { go_of(&parent) };
        }
    }
}
