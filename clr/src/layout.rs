//! Unity UI automatic layout: VerticalLayoutGroup, HorizontalLayoutGroup, GridLayoutGroup, ContentSizeFitter.
use crate::render::Renderer;
use crate::ui::xf;
use crate::unity::*;
use crate::value::*;
use crate::Vm;
use std::collections::HashMap;

struct Lay {
    kind: String,
    d: HashMap<String, f32>,
}

impl Lay {
    fn g(&self, k: &str) -> f32 {
        self.d.get(k).copied().unwrap_or(0.0)
    }
    fn pad(&self, axis: usize) -> (f32, f32) {
        if axis == 0 {
            (self.g("m_Padding.m_Left"), self.g("m_Padding.m_Right"))
        } else {
            (self.g("m_Padding.m_Top"), self.g("m_Padding.m_Bottom"))
        }
    }
    fn pad_total(&self, axis: usize) -> f32 {
        let p = self.pad(axis);
        p.0 + p.1
    }
    fn align(&self, axis: usize) -> f32 {
        let a = self.g("m_ChildAlignment") as i32;
        if axis == 0 { (a % 3) as f32 * 0.5 } else { (a / 3) as f32 * 0.5 }
    }
    fn is_box(&self) -> bool {
        self.kind == "VerticalLayoutGroup" || self.kind == "HorizontalLayoutGroup"
    }
}

fn layout_of(go: &Value) -> (Option<Lay>, Option<(i32, i32)>) {
    let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
    let (mut group, mut csf) = (None, None);
    for c in comps {
        with_comp(&c, |k| {
            if !k.enabled {
                return;
            }
            if let CK::Layout { kind, d } = &k.k {
                match kind.as_str() {
                    "ContentSizeFitter" => csf = Some((d.get("m_HorizontalFit").copied().unwrap_or(0.0) as i32, d.get("m_VerticalFit").copied().unwrap_or(0.0) as i32)),
                    "LayoutElement" => {}
                    _ => group = Some(Lay { kind: kind.clone(), d: d.clone() }),
                }
            }
        });
    }
    (group, csf)
}

fn active_children(tr: &Value) -> Vec<Value> {
    with_tr(tr, |t| t.children.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|c| with_go(&go_of(c), |g| g.active).unwrap_or(false))
        .collect()
}

/// Places a child along an axis (top-left anchor, like Unity).
fn set_child_axis(child: &Value, axis: usize, start: f32, size: Option<f32>) {
    with_tr(child, |t| {
        let sz = size.unwrap_or(t.size[axis]);
        if axis == 0 {
            t.amin[0] = 0.0;
            t.amax[0] = 0.0;
            t.size[0] = sz;
            t.apos[0] = start + sz * t.pivot[0];
        } else {
            t.amin[1] = 1.0;
            t.amax[1] = 1.0;
            t.size[1] = sz;
            t.apos[1] = -(start + sz * (1.0 - t.pivot[1]));
        }
    });
    mark_dirty(child);
}

impl Renderer {
    /// (min, preferred, flexible) of a child along the axis.
    fn child_sizes(&mut self, vm: &mut Vm, child: &Value, axis: usize, control: bool, force_expand: bool) -> (f32, f32, f32) {
        if !control {
            let s = with_tr(child, |t| t.size[axis]).unwrap_or(0.0);
            return (s, s, if force_expand { 1.0 } else { 0.0 });
        }
        let go = go_of(child);
        let comps = with_go(&go, |g| g.comps.clone()).unwrap_or_default();
        let mut pref = 0.0f32;
        let mut flex = 0.0f32;
        let mut min = 0.0f32;
        // nested group?
        let (grp, _) = layout_of(&go);
        if let Some(g) = &grp {
            let t = self.group_totals(vm, &go, g, axis);
            min = t.0;
            pref = t.1;
            flex = t.2;
        } else {
            let w = xf(vm, child).w;
            for c in &comps {
                let is_text = with_comp(c, |k| k.enabled && matches!(k.k, CK::Text(_))).unwrap_or(false);
                let is_img = with_comp(c, |k| k.enabled && matches!(&k.k, CK::Image(_))).unwrap_or(false);
                if is_text {
                    if let Some((pw, ph)) = self.measure_text_comp(vm, c, if axis == 1 { Some(w) } else { None }) {
                        pref = if axis == 0 { pw } else { ph };
                    }
                } else if is_img {
                    let sp = with_comp(c, |k| if let CK::Image(i) = &k.k { i.sprite.clone() } else { Value::Null }).unwrap_or(Value::Null);
                    let sz = with_comp(&sp, |k| if let CK::Sprite(s) = &k.k { Some((s.rect[2] * 100.0 / s.ppu.max(1.0), s.rect[3] * 100.0 / s.ppu.max(1.0))) } else { None }).flatten();
                    if let Some((sw, sh)) = sz {
                        pref = pref.max(if axis == 0 { sw } else { sh });
                    }
                }
            }
        }
        if force_expand {
            flex = flex.max(1.0);
        }
        (min, pref, flex)
    }

    /// Totals (min, pref, flex) of a group along an axis.
    fn group_totals(&mut self, vm: &mut Vm, go: &Value, g: &Lay, axis: usize) -> (f32, f32, f32) {
        let tr = with_go(go, |x| x.transform.clone()).unwrap_or(Value::Null);
        let kids = active_children(&tr);
        if g.is_box() {
            let vertical = g.kind == "VerticalLayoutGroup";
            let (ctl, fe) = if axis == 0 { (g.g("m_ChildControlWidth") != 0.0, g.g("m_ChildForceExpandWidth") != 0.0) } else { (g.g("m_ChildControlHeight") != 0.0, g.g("m_ChildForceExpandHeight") != 0.0) };
            let other = vertical != (axis == 1);
            let spacing = g.g("m_Spacing");
            let mut tot_min = g.pad_total(axis);
            let mut tot_pref = tot_min;
            let mut tot_flex = 0.0f32;
            for c in &kids {
                let (mn, pf, fl) = self.child_sizes(vm, c, axis, ctl, fe);
                if other {
                    tot_min = tot_min.max(mn + g.pad_total(axis));
                    tot_pref = tot_pref.max(pf + g.pad_total(axis));
                    tot_flex = tot_flex.max(fl);
                } else {
                    tot_min += mn + spacing;
                    tot_pref += pf + spacing;
                    tot_flex += fl;
                }
            }
            if !other && !kids.is_empty() {
                tot_min -= spacing;
                tot_pref -= spacing;
            }
            (tot_min, tot_pref.max(tot_min), tot_flex)
        } else {
            // Grid
            let n = kids.len() as f32;
            let (cw, ch) = (g.g("m_CellSize.x"), g.g("m_CellSize.y"));
            let (sx, sy) = (g.g("m_Spacing.x"), g.g("m_Spacing.y"));
            let constraint = g.g("m_Constraint") as i32;
            let count = g.g("m_ConstraintCount").max(1.0);
            if axis == 0 {
                let (min_c, pref_c) = match constraint {
                    1 => (count, count),
                    2 => ((n / count).ceil(), (n / count).ceil()),
                    _ => (1.0, n.sqrt().ceil()),
                };
                (g.pad_total(0) + (cw + sx) * min_c - sx, g.pad_total(0) + (cw + sx) * pref_c - sx, -1.0)
            } else {
                let w = xf(vm, &tr).w;
                let rows = match constraint {
                    1 => (n / count - 0.001).ceil(),
                    2 => count,
                    _ => {
                        let cx = (((w - g.pad_total(0) + sx + 0.001) / (cw + sx)).floor()).max(1.0);
                        (n / cx).ceil()
                    }
                };
                let m = g.pad_total(1) + (ch + sy) * rows - sy;
                (m, m, -1.0)
            }
        }
    }

    fn layout_box(&mut self, vm: &mut Vm, go: &Value, g: &Lay) {
        let tr = with_go(go, |x| x.transform.clone()).unwrap_or(Value::Null);
        let kids = active_children(&tr);
        if kids.is_empty() {
            return;
        }
        let vertical = g.kind == "VerticalLayoutGroup";
        let spacing = g.g("m_Spacing");
        for axis in 0..2usize {
            let (ctl, fe) = if axis == 0 { (g.g("m_ChildControlWidth") != 0.0, g.g("m_ChildForceExpandWidth") != 0.0) } else { (g.g("m_ChildControlHeight") != 0.0, g.g("m_ChildForceExpandHeight") != 0.0) };
            let x = xf(vm, &tr);
            let size = if axis == 0 { x.w } else { x.h };
            let other = vertical != (axis == 1);
            let totals = self.group_totals(vm, go, g, axis);
            let align = g.align(axis);
            let pad = g.pad(axis);
            if other {
                let inner = size - g.pad_total(axis);
                for c in &kids {
                    let (mn, pf, fl) = self.child_sizes(vm, c, axis, ctl, fe);
                    let req = inner.clamp(mn, if fl > 0.0 { size } else { pf.max(mn) });
                    let start = (size - (req + g.pad_total(axis))) * align + pad.0;
                    if ctl {
                        set_child_axis(c, axis, start, Some(req));
                    } else {
                        let cur = with_tr(c, |t| t.size[axis]).unwrap_or(0.0);
                        set_child_axis(c, axis, start + (req - cur) * align, None);
                    }
                }
            } else {
                let mut pos = pad.0;
                let mut flex_mul = 0.0f32;
                let surplus = size - totals.1;
                if surplus > 0.0 {
                    if totals.2 == 0.0 {
                        pos = (size - totals.1) * align + pad.0;
                    } else if totals.2 > 0.0 {
                        flex_mul = surplus / totals.2;
                    }
                }
                let lerp = if (totals.0 - totals.1).abs() > 1e-6 { ((size - totals.0) / (totals.1 - totals.0)).clamp(0.0, 1.0) } else { 0.0 };
                for c in &kids {
                    let (mn, pf, fl) = self.child_sizes(vm, c, axis, ctl, fe);
                    let cs = mn + (pf - mn) * lerp + fl * flex_mul;
                    if ctl {
                        set_child_axis(c, axis, pos, Some(cs));
                    } else {
                        let cur = with_tr(c, |t| t.size[axis]).unwrap_or(0.0);
                        set_child_axis(c, axis, pos + (cs - cur) * align, None);
                    }
                    pos += cs + spacing;
                }
            }
        }
    }

    fn layout_grid(&mut self, vm: &mut Vm, go: &Value, g: &Lay) {
        let tr = with_go(go, |x| x.transform.clone()).unwrap_or(Value::Null);
        let kids = active_children(&tr);
        let n = kids.len();
        let (cw, ch) = (g.g("m_CellSize.x"), g.g("m_CellSize.y"));
        let (sx, sy) = (g.g("m_Spacing.x"), g.g("m_Spacing.y"));
        let x = xf(vm, &tr);
        let (width, height) = (x.w, x.h);
        for c in &kids {
            with_tr(c, |t| {
                t.amin = [0.0, 1.0];
                t.amax = [0.0, 1.0];
                t.size = [cw, ch];
            });
        }
        if n == 0 {
            return;
        }
        let constraint = g.g("m_Constraint") as i32;
        let cnt = g.g("m_ConstraintCount").max(1.0) as usize;
        let (mut cx, mut cy) = (1usize, 1usize);
        match constraint {
            1 => {
                cx = cnt;
                if n > cx {
                    cy = n / cx + (n % cx > 0) as usize;
                }
            }
            2 => {
                cy = cnt;
                if n > cy {
                    cx = n / cy + (n % cy > 0) as usize;
                }
            }
            _ => {
                cx = if cw + sx <= 0.0 { usize::MAX } else { (((width - g.pad_total(0) + sx + 0.001) / (cw + sx)).floor() as i64).max(1) as usize };
                cy = if ch + sy <= 0.0 { usize::MAX } else { (((height - g.pad_total(1) + sy + 0.001) / (ch + sy)).floor() as i64).max(1) as usize };
            }
        }
        let corner = g.g("m_StartCorner") as i32;
        let (corner_x, corner_y) = (corner % 2, corner / 2);
        let horizontal_axis = g.g("m_StartAxis") as i32 == 0;
        let (per_main, ax, ay) = if horizontal_axis {
            (cx, cx.clamp(1, n), (((n as f32) / cx as f32).ceil() as usize).clamp(1, cy))
        } else {
            (cy, (((n as f32) / cy as f32).ceil() as usize).clamp(1, cx), cy.clamp(1, n))
        };
        let req_w = ax as f32 * cw + (ax as f32 - 1.0) * sx;
        let req_h = ay as f32 * ch + (ay as f32 - 1.0) * sy;
        let start_off = |axis: usize, req: f32| -> f32 {
            let total = req + g.pad_total(axis);
            let avail = if axis == 0 { width } else { height };
            (avail - total) * g.align(axis) + g.pad(axis).0
        };
        let (so_x, so_y) = (start_off(0, req_w), start_off(1, req_h));
        for (i, c) in kids.iter().enumerate() {
            let (mut px, mut py) = if horizontal_axis { (i % per_main, i / per_main) } else { (i / per_main, i % per_main) };
            if corner_x == 1 {
                px = ax - 1 - px.min(ax - 1);
            }
            if corner_y == 1 {
                py = ay - 1 - py.min(ay - 1);
            }
            set_child_axis(c, 0, so_x + (cw + sx) * px as f32, Some(cw));
            set_child_axis(c, 1, so_y + (ch + sy) * py as f32, Some(ch));
        }
    }

    fn layout_one(&mut self, vm: &mut Vm, go: &Value) {
        let (group, csf) = layout_of(go);
        let tr = with_go(go, |x| x.transform.clone()).unwrap_or(Value::Null);
        if let Some(g) = &group {
            if g.is_box() {
                self.layout_box(vm, go, g);
            } else {
                self.layout_grid(vm, go, g);
            }
        }
        if let Some((hf, vf)) = csf {
            for (axis, fit) in [(0usize, hf), (1usize, vf)] {
                if fit == 0 {
                    continue;
                }
                let pref = if let Some(g) = &group {
                    let t = self.group_totals(vm, go, g, axis);
                    if fit == 1 { t.0 } else { t.1 }
                } else {
                    // text only
                    let comps = with_go(go, |x| x.comps.clone()).unwrap_or_default();
                    let w = xf(vm, &tr).w;
                    let mut v = 0.0;
                    for c in &comps {
                        if with_comp(c, |k| k.enabled && matches!(k.k, CK::Text(_))).unwrap_or(false) {
                            if let Some((pw, ph)) = self.measure_text_comp(vm, c, if axis == 1 { Some(w) } else { None }) {
                                v = if axis == 0 { pw } else { ph };
                            }
                        }
                    }
                    v
                };
                // size = pref (non-stretched anchor): sizeDelta = pref - parent*(amax-amin)
                let (parent, amin, amax) = match with_tr(&tr, |t| (t.parent.clone(), t.amin, t.amax)) {
                    Some(v) => v,
                    None => continue,
                };
                let pw = if parent.is_null() { 0.0 } else { let p = xf(vm, &parent); if axis == 0 { p.w } else { p.h } };
                let nd = pref - (amax[axis] - amin[axis]) * pw;
                let changed = with_tr(&tr, |t| {
                    let old = t.size[axis];
                    t.size[axis] = nd;
                    (old - nd).abs() > 1e-3
                })
                .unwrap_or(false);
                if changed {
                    mark_dirty(&tr);
                }
            }
        }
    }

    /// Applies all active layouts (called before quad production).
    pub fn run_layout(&mut self, vm: &mut Vm) {
        let epoch = LAYOUT_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
        if vm.eng.layout_seen == epoch {
            return;
        }
        vm.eng.layout_seen = epoch;
        if vm.eng.layout_gos.is_none() {
            let mut v: Vec<(usize, Value)> = vec![];
            for go in vm.eng.gos.clone() {
                let (g, c) = layout_of(&go);
                if g.is_some() || c.is_some() {
                    let tr = with_go(&go, |x| x.transform.clone()).unwrap_or(Value::Null);
                    let mut depth = 0;
                    let mut cur = tr;
                    while let Some(p) = with_tr(&cur, |t| t.parent.clone()) {
                        if p.is_null() {
                            break;
                        }
                        depth += 1;
                        cur = p;
                    }
                    v.push((depth, go));
                }
            }
            v.sort_by_key(|x| std::cmp::Reverse(x.0));
            vm.eng.layout_gos = Some(v.into_iter().map(|x| x.1).collect());
        }
        let list = vm.eng.layout_gos.clone().unwrap();
        for pass in 0..2 {
            let iter: Box<dyn Iterator<Item = &Value>> = if pass == 0 { Box::new(list.iter()) } else { Box::new(list.iter().rev()) };
            for go in iter {
                if active_in_hierarchy(go) {
                    self.layout_one(vm, go);
                }
            }
        }
    }
}
