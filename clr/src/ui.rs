//! RectTransform layout (world positions in screen pixels, bottom-left origin).
use crate::unity::*;
use crate::value::*;
use crate::Vm;

#[inline]
pub(crate) fn rotation_axes(angle: f32) -> (f32, f32) {
    if angle == 0.0 { (1.0, angle) } else { (angle.cos(), angle.sin()) }
}

#[derive(Clone, Copy, Debug)]
pub struct Xf {
    /// pivot position in screen pixels
    pub origin: [f32; 2],
    pub scale: [f32; 2],
    pub rot: f32, // radians
    pub w: f32,
    pub h: f32,
    pub pivot: [f32; 2],
}

fn canvas_info(tr: &Value) -> Option<(i32, f32)> {
    let go = go_of(tr);
    let comps = with_go(&go, |g| g.comps.clone())?;
    let mut mode = None;
    let mut scaler = None;
    for c in comps {
        with_comp(&c, |k| match &k.k {
            CK::Canvas(cd) => mode = Some(cd.mode),
            CK::CanvasScaler { mode, scale, reference, match_ } => scaler = Some((*mode, *scale, *reference, *match_)),
            _ => {}
        });
    }
    mode.map(|m| (m, scaler.map(|s| s.1).unwrap_or(1.0)))
}

pub fn canvas_scale(vm: &Vm, tr: &Value) -> f32 {
    let go = go_of(tr);
    let comps = with_go(&go, |g| g.comps.clone()).unwrap_or_default();
    for c in comps {
        if let Some(s) = with_comp(&c, |k| match &k.k {
            CK::CanvasScaler { mode, scale, reference, match_ } => Some((*mode, *scale, *reference, *match_)),
            _ => None,
        })
        .flatten()
        {
            let (mode, scale, reference, m) = s;
            if mode == 1 {
                let (w, h) = (vm.eng.screen.0 as f32, vm.eng.screen.1 as f32);
                let lw = (w / reference[0]).log2();
                let lh = (h / reference[1]).log2();
                return 2f32.powf(lw + (lh - lw) * m);
            }
            return scale;
        }
    }
    1.0
}

pub fn xf(vm: &Vm, tr: &Value) -> Xf {
    let (parent, pos, scale, rot, amin, amax, pivot, apos, size, is_rect) =
        match with_tr(tr, |t| (t.parent.clone(), t.pos, t.scale, t.rot_z, t.amin, t.amax, t.pivot, t.apos, t.size, t.is_rect)) {
            Some(x) => x,
            None => return Xf { origin: [0.0; 2], scale: [1.0; 2], rot: 0.0, w: 0.0, h: 0.0, pivot: [0.5; 2] },
        };
    // canvas root?
    if let Some((mode, _)) = canvas_info(tr) {
        let is_root = parent.is_null() || canvas_info(&parent).is_none() && !has_canvas_ancestor(&parent);
        if is_root && (mode == 0 || mode == 1) {
            let s = canvas_scale(vm, tr);
            let (sw, sh) = (vm.eng.screen.0 as f32, vm.eng.screen.1 as f32);
            return Xf { origin: [pivot[0] * sw, pivot[1] * sh], scale: [s, s], rot: 0.0, w: sw / s, h: sh / s, pivot };
        }
    }
    if parent.is_null() {
        return Xf { origin: [pos[0], pos[1]], scale: [scale[0], scale[1]], rot: rot.to_radians(), w: size[0], h: size[1], pivot };
    }
    let p = xf(vm, &parent);
    let (pw, ph) = (p.w, p.h);
    let px0 = -p.pivot[0] * pw;
    let py0 = -p.pivot[1] * ph;
    let (w, h, local) = if is_rect {
        let a0 = [px0 + amin[0] * pw, py0 + amin[1] * ph];
        let a1 = [px0 + amax[0] * pw, py0 + amax[1] * ph];
        let w = (amax[0] - amin[0]) * pw + size[0];
        let h = (amax[1] - amin[1]) * ph + size[1];
        let rx = a0[0] + (a1[0] - a0[0]) * pivot[0] + apos[0];
        let ry = a0[1] + (a1[1] - a0[1]) * pivot[1] + apos[1];
        (w, h, [rx, ry])
    } else {
        (0.0, 0.0, [pos[0], pos[1]])
    };
    let (c, s) = rotation_axes(p.rot);
    let lx = local[0] * p.scale[0];
    let ly = local[1] * p.scale[1];
    let origin = [p.origin[0] + c * lx - s * ly, p.origin[1] + s * lx + c * ly];
    Xf { origin, scale: [p.scale[0] * scale[0], p.scale[1] * scale[1]], rot: p.rot + rot.to_radians(), w, h, pivot }
}

fn has_canvas_ancestor(tr: &Value) -> bool {
    let mut cur = with_tr(tr, |t| t.parent.clone()).unwrap_or(Value::Null);
    while !cur.is_null() {
        if canvas_info(&cur).is_some() {
            return true;
        }
        cur = with_tr(&cur, |t| t.parent.clone()).unwrap_or(Value::Null);
    }
    false
}

pub fn world_pos(vm: &mut Vm, tr: &Value) -> [f32; 3] {
    let x = xf(vm, tr);
    [x.origin[0], x.origin[1], 0.0]
}

pub fn set_world_pos(vm: &mut Vm, tr: &Value, p: [f32; 3]) {
    let parent = with_tr(tr, |t| t.parent.clone()).unwrap_or(Value::Null);
    if parent.is_null() {
        with_tr(tr, |t| t.pos = p);
        return;
    }
    let pxf = xf(vm, &parent);
    let (dx, dy) = (p[0] - pxf.origin[0], p[1] - pxf.origin[1]);
    let (c, s) = rotation_axes(-pxf.rot);
    let lx = (c * dx - s * dy) / if pxf.scale[0] == 0.0 { 1.0 } else { pxf.scale[0] };
    let ly = (s * dx + c * dy) / if pxf.scale[1] == 0.0 { 1.0 } else { pxf.scale[1] };
    let (amin, amax, pivot, is_rect) = with_tr(tr, |t| (t.amin, t.amax, t.pivot, t.is_rect)).unwrap();
    if is_rect {
        let (pw, ph) = (pxf.w, pxf.h);
        let px0 = -pxf.pivot[0] * pw;
        let py0 = -pxf.pivot[1] * ph;
        let a0 = [px0 + amin[0] * pw, py0 + amin[1] * ph];
        let a1 = [px0 + amax[0] * pw, py0 + amax[1] * ph];
        let rx = a0[0] + (a1[0] - a0[0]) * pivot[0];
        let ry = a0[1] + (a1[1] - a0[1]) * pivot[1];
        with_tr(tr, |t| t.apos = [lx - rx, ly - ry]);
    } else {
        with_tr(tr, |t| t.pos = [lx, ly, p[2]]);
    }
}

/// local rect (x, y, w, h) relative to the pivot.
pub fn local_rect(vm: &mut Vm, tr: &Value) -> [f32; 4] {
    let x = xf(vm, tr);
    [-x.pivot[0] * x.w, -x.pivot[1] * x.h, x.w, x.h]
}

/// Transform of a child from its parent's.
pub fn child_xf(p: &Xf, t: &Tr) -> Xf {
    let (pw, ph) = (p.w, p.h);
    let px0 = -p.pivot[0] * pw;
    let py0 = -p.pivot[1] * ph;
    let (w, h, local) = if t.is_rect {
        let a0 = [px0 + t.amin[0] * pw, py0 + t.amin[1] * ph];
        let a1 = [px0 + t.amax[0] * pw, py0 + t.amax[1] * ph];
        let w = (t.amax[0] - t.amin[0]) * pw + t.size[0];
        let h = (t.amax[1] - t.amin[1]) * ph + t.size[1];
        let rx = a0[0] + (a1[0] - a0[0]) * t.pivot[0] + t.apos[0];
        let ry = a0[1] + (a1[1] - a0[1]) * t.pivot[1] + t.apos[1];
        (w, h, [rx, ry])
    } else {
        (0.0, 0.0, [t.pos[0], t.pos[1]])
    };
    let (c, s) = rotation_axes(p.rot);
    let lx = local[0] * p.scale[0];
    let ly = local[1] * p.scale[1];
    let origin = [p.origin[0] + c * lx - s * ly, p.origin[1] + s * lx + c * ly];
    Xf { origin, scale: [p.scale[0] * t.scale[0], p.scale[1] * t.scale[1]], rot: p.rot + t.rot_z.to_radians(), w, h, pivot: t.pivot }
}

/// World rectangle as presented to the player. The game still uses `xf`.
pub fn view_xf(vm: &Vm, tr: &Value) -> Xf {
    vm.eng.desktop_layout.rect(tr).unwrap_or_else(|| xf(vm, tr))
}

pub fn view_child_xf(vm: &Vm, parent: &Xf, tr: &Value) -> Option<Xf> {
    vm.eng.desktop_layout.rect(tr).or_else(|| with_tr(tr, |t| child_xf(parent, t)))
}

pub(crate) fn cache_menu_canvases(vm: &mut Vm) {
    for menu in crate::dev::menu_roots(vm) {
        let Value::Obj(object) = &menu else { continue };
        let canvas = with_go(&menu, |go| {
            go.comps.iter().find(|c| with_comp(c, |c| matches!(c.k, CK::Canvas(_))) == Some(true)).cloned()
        }).flatten();
        if let Some(canvas) = canvas {
            vm.eng.parked_menu_canvases.insert(std::rc::Rc::as_ptr(object) as usize, canvas);
        }
    }
}

pub(crate) fn menu_is_parked(vm: &Vm, go: &Value, x: &Xf) -> bool {
    // MenuSwapper disables a closed menu's canvas and translates its entire
    // hierarchy far outside the viewport. Its controls cannot receive input.
    let [left, bottom] = x.origin;
    if left.abs() < vm.eng.desktop_layout.width() * 2.0
        || bottom.abs() < vm.eng.screen.1 as f32 * 2.0 {
        return false;
    }
    let Value::Obj(object) = go else { return false };
    vm.eng.parked_menu_canvases.get(&(std::rc::Rc::as_ptr(object) as usize))
        .is_some_and(|canvas| with_comp(canvas, |canvas| !canvas.enabled) == Some(true))
}

/// Only projected nodes can use the extra room to the right of the original canvas.
pub(crate) fn view_width(vm: &Vm, tr: &Value) -> f32 {
    if vm.eng.desktop_layout.rect(tr).is_some() || crate::save_slots::owns(&go_of(tr)) {
        vm.eng.desktop_layout.width()
    } else {
        vm.eng.screen.0 as f32
    }
}

/// Local position (relative to the parent's pivot) of a node.
pub fn local_pos(vm: &Vm, tr: &Value) -> [f32; 3] {
    let (parent, pos, amin, amax, pivot, apos, is_rect) = match with_tr(tr, |t| (t.parent.clone(), t.pos, t.amin, t.amax, t.pivot, t.apos, t.is_rect)) {
        Some(x) => x,
        None => return [0.0; 3],
    };
    if !is_rect || parent.is_null() {
        return pos;
    }
    let p = xf(vm, &parent);
    let (pw, ph) = (p.w, p.h);
    let (px0, py0) = (-p.pivot[0] * pw, -p.pivot[1] * ph);
    let a0 = [px0 + amin[0] * pw, py0 + amin[1] * ph];
    let a1 = [px0 + amax[0] * pw, py0 + amax[1] * ph];
    [a0[0] + (a1[0] - a0[0]) * pivot[0] + apos[0], a0[1] + (a1[1] - a0[1]) * pivot[1] + apos[1], pos[2]]
}

/// Sets the local position; for a RectTransform, adjusts anchoredPosition.
pub fn set_local_pos(vm: &Vm, tr: &Value, p: [f32; 3]) {
    let (parent, amin, amax, pivot, is_rect) = match with_tr(tr, |t| (t.parent.clone(), t.amin, t.amax, t.pivot, t.is_rect)) {
        Some(x) => x,
        None => return,
    };
    if !is_rect || parent.is_null() {
        with_tr(tr, |t| t.pos = p);
        return;
    }
    let pxf = xf(vm, &parent);
    let (pw, ph) = (pxf.w, pxf.h);
    let (px0, py0) = (-pxf.pivot[0] * pw, -pxf.pivot[1] * ph);
    let a0 = [px0 + amin[0] * pw, py0 + amin[1] * ph];
    let a1 = [px0 + amax[0] * pw, py0 + amax[1] * ph];
    let rx = a0[0] + (a1[0] - a0[0]) * pivot[0];
    let ry = a0[1] + (a1[1] - a0[1]) * pivot[1];
    with_tr(tr, |t| {
        t.apos = [p[0] - rx, p[1] - ry];
        t.pos[2] = p[2];
    });
}
