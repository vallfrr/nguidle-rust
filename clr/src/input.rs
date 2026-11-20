//! Pointer input (mouse / touch): UI raycast, hover, press, click, drag and drop, Slider, Toggle.
use crate::ui::{child_xf, xf, Xf};
use crate::unity::*;
use crate::value::*;
use crate::Vm;

#[derive(Default)]
pub struct PointerState {
    pub hover: Value,
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

/// (Virtual) point inside this node's local rect?
fn hit_rect(x: &Xf, p: [f32; 2]) -> bool {
    let (dx, dy) = (p[0] - x.origin[0], p[1] - x.origin[1]);
    let (c, s) = ((-x.rot).cos(), (-x.rot).sin());
    let lx = (c * dx - s * dy) / if x.scale[0] == 0.0 { 1.0 } else { x.scale[0] };
    let ly = (s * dx + c * dy) / if x.scale[1] == 0.0 { 1.0 } else { x.scale[1] };
    let (x0, y0) = (-x.pivot[0] * x.w, -x.pivot[1] * x.h);
    lx >= x0 && lx <= x0 + x.w && ly >= y0 && ly <= y0 + x.h
}

fn raycast_node(vm: &mut Vm, tr: &Value, x: Xf, p: [f32; 2], masked_out: bool, hit: &mut Value) {
    let go = go_of(tr);
    let (active, comps) = match with_go(&go, |g| (g.active, g.comps.clone())) {
        Some(v) => v,
        None => return,
    };
    if !active {
        return;
    }
    let inside = hit_rect(&x, p);
    let mut child_masked = masked_out;
    let mut is_graphic = false;
    for c in &comps {
        with_comp(c, |k| {
            if !k.enabled {
                return;
            }
            match &k.k {
                CK::Image(i) | CK::RawImage(i) => {
                    if i.raycast && i.color[3] > 0.0 {
                        is_graphic = true;
                    }
                }
                CK::Text(t) => {
                    if t.raycast {
                        is_graphic = true;
                    }
                }
                CK::Mask { .. } => {
                    if !inside {
                        child_masked = true;
                    }
                }
                _ => {}
            }
        });
    }
    if is_graphic && inside && !masked_out {
        *hit = go.clone();
    }
    let children = with_tr(tr, |t| t.children.clone()).unwrap_or_default();
    for ch in children {
        let cx = match with_tr(&ch, |t| child_xf(&x, t)) {
            Some(v) => v,
            None => continue,
        };
        raycast_node(vm, &ch, cx, p, child_masked, hit);
    }
}

pub fn raycast(vm: &mut Vm, p: [f32; 2]) -> Value {
    let mut hit = Value::Null;
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
    for (_, tr) in roots {
        let x = xf(vm, &tr);
        raycast_node(vm, &tr, x, p, false, &mut hit);
    }
    hit
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
                        (SelKind::Button, Ev::Click) | (SelKind::Toggle, Ev::Click) | (SelKind::InputField, Ev::Click) => 3,
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
                    if e == Ev::Drag {
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
                    if e == Ev::Click {
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
            button: 0,
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
        let ped = self.mk_ped(pos);
        if !old.is_null() {
            let h = self.find_ev_handler(&old, Ev::Exit);
            if !h.is_null() {
                self.dispatch(&h, Ev::Exit, &ped);
            }
        }
        self.eng.ptr.hover = new.clone();
        if !new.is_null() {
            let h = self.find_ev_handler(&new, Ev::Enter);
            if !h.is_null() {
                self.dispatch(&h, Ev::Enter, &ped);
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

    fn pointer_update_inner(&mut self) {
        let pos = self.eng.mouse;
        let down = self.eng.mouse_down;
        let (edge_d, edge_u) = (self.eng.mouse_down_edge, self.eng.mouse_up_edge);
        if !down && !edge_d && !edge_u && self.eng.ptr.pressed.is_null() && self.eng.ptr.hover.is_null() && !self.eng.hover_always {
            return;
        }
        let p = [pos.0, pos.1];
        let hit = if edge_d || down || edge_u || self.eng.hover_always { raycast(self, p) } else { Value::Null };
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
            if !sl.is_null() && self.is_slider(&sl) {
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
        self.eng.ptr.last_pos = pos;
    }

    fn is_slider(&mut self, go: &Value) -> bool {
        let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
        comps.iter().any(|c| with_comp(c, |k| matches!(&k.k, CK::Sel(s) if s.kind == SelKind::Slider)).unwrap_or(false))
    }

    fn slider_drag(&mut self, go: &Value, p: [f32; 2]) {
        let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
        let tr = with_go(go, |g| g.transform.clone()).unwrap_or(Value::Null);
        let x = xf(self, &tr);
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
            let new_top = if c_h > v_h { want.clamp(v_top, v_top + (c_h - v_h)) } else { v_top };
            my = new_top - c_top;
        }
        if horizontal {
            let (v_left, v_w) = (vx.origin[0] - vx.pivot[0] * vx.w * vx.scale[0], vx.w * vx.scale[0]);
            let (c_left, c_w) = (cx.origin[0] - cx.pivot[0] * cx.w * cx.scale[0], cx.w * cx.scale[0]);
            let want = c_left + dx;
            let new_left = if c_w > v_w { want.clamp(v_left - (c_w - v_w), v_left) } else { v_left };
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
