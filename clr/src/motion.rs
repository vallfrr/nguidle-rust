//! Render-time motion. The authoritative UI and game state still update on their normal clocks.

use crate::ui::{set_world_pos, view_xf, world_pos};
use crate::unity::*;
use crate::value::Value;
use crate::Vm;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

#[derive(Default)]
pub(crate) struct Motion {
    pub enabled: bool,
    pub clock: Option<(f64, f64)>,
    bars: HashMap<usize, Bar>,
    followers: Vec<Follower>,
    moving: Vec<MovingPanel>,
    fades: Vec<Fade>,
    color_picker: Value,
}

struct Follower {
    transform: Value,
    offset: Option<[f32; 2]>,
}

struct MovingPanel {
    transform: Value,
    tween: Option<Tween<2>>,
}

struct Fade {
    text: Value,
    tween: Option<Tween<4>>,
}

struct Tween<const N: usize> {
    start: f64,
    duration: f64,
    from: [f32; N],
    to: [f32; N],
}

impl<const N: usize> Tween<N> {
    fn fraction(&self, time: f64) -> f32 {
        ((time - self.start) / self.duration).clamp(0.0, 1.0) as f32
    }

    fn at(&self, time: f64) -> [f32; N] {
        let fraction = self.fraction(time);
        std::array::from_fn(|i| self.from[i] + (self.to[i] - self.from[i]) * fraction)
    }
}

struct Bar {
    fill: Value,
    handle: Value,
    axis: usize,
    reverse: bool,
    samples: VecDeque<BarSample>,
}

struct BarSample {
    tween: Tween<1>,
    wrap: bool,
}

impl BarSample {
    fn at(&self, time: f64) -> f32 {
        if self.wrap && self.tween.fraction(time) < 1.0 {
            let a = self.tween.from[0];
            a + (1.0 - a) * self.tween.fraction(time)
        } else {
            self.tween.at(time)[0]
        }
    }
}

fn id(value: &Value) -> Option<usize> {
    match value {
        Value::Obj(object) => Some(Rc::as_ptr(object) as usize),
        _ => None,
    }
}

fn first_text(go: &Value) -> Value {
    let text = with_go(go, |g| g.comps.iter().find(|c| {
        with_comp(c, |c| matches!(c.k, CK::Text(_))) == Some(true)
    }).cloned()).flatten();
    if let Some(text) = text { return text; }
    for child in with_tr(&transform_of(go), |tr| tr.children.clone()).unwrap_or_default() {
        let text = first_text(&go_of(&child));
        if !text.is_null() { return text; }
    }
    Value::Null
}

impl Vm {
    /// Desktop renderers opt in after loading the scene. Reference runs keep raw game samples.
    pub fn enable_visual_motion(&mut self) {
        if self.eng.motion.enabled {
            return;
        }
        let mut motion = Motion { enabled: true, ..Default::default() };
        for component in &self.eng.user_comps {
            let Value::Obj(object) = component else { continue };
            let ty = &self.types[object.ty as usize];
            let field = |name| ty.fields.iter().position(|field| field == name)
                .and_then(|index| object.f.borrow().get(index).cloned()).unwrap_or_default();
            if ty.name == "CUIColorPicker" {
                motion.color_picker = component.clone();
            }
            let ghost = transform_of(&field("ghost"));
            if !ghost.is_null() && !motion.followers.iter().any(|entry| same(&entry.transform, &ghost)) {
                motion.followers.push(Follower { transform: ghost, offset: None });
            }
            if ty.name == "ShakeySalesPanel" && !field("itemPanel").is_null() {
                let transform = transform_of(&field("salesPanel"));
                if !transform.is_null() {
                    let text = first_text(&field("salesPanel"));
                    if !text.is_null() { motion.fades.push(Fade { text, tween: None }); }
                    motion.moving.push(MovingPanel { transform, tween: None });
                }
            }
        }
        self.eng.motion = motion;
    }

    /// The existing hue and saturation handlers also accept held motion between
    /// game ticks. Press and release still go through the normal input dispatch.
    pub fn update_color_picker_pointer(&mut self) {
        if !self.eng.mouse_down || self.eng.mouse_button != 0 ||
            self.eng.mouse_down_edge || self.eng.mouse_up_edge || crate::save_slots::is_open(self) {
            return;
        }
        let picker = self.eng.motion.color_picker.clone();
        if with_comp(&picker, |c| c.enabled).unwrap_or(false) && active_in_hierarchy(&go_of(&picker)) {
            let _ = self.call_msg(&picker, "Update");
        }
    }
}

pub(crate) fn text_color_changed(vm: &mut Vm, text: &Value, old: Col, new: Col) {
    let Some((time, period)) = vm.eng.motion.clock else { return };
    if let Some(fade) = vm.eng.motion.fades.iter_mut().find(|entry| same(&entry.text, text)) {
        fade.tween = Some(Tween {
            start: time,
            duration: period.clamp(0.005, 0.1),
            from: fade.tween.as_ref().map_or(old, |tween| tween.at(time)),
            to: new,
        });
    }
}

pub(crate) fn slider_changed(
    vm: &mut Vm, slider: &Value, fill: &Value, handle: &Value, direction: (usize, bool),
    old: f32, new: f32,
) {
    if !vm.eng.motion.enabled || old == new || !old.is_finite() || !new.is_finite() {
        return;
    }
    let Some(key) = id(slider) else { return };
    let (axis, reverse) = direction;
    let Some((time, period)) = vm.eng.motion.clock else {
        vm.eng.motion.bars.remove(&key);
        return;
    };
    let bar = vm.eng.motion.bars.entry(key).or_insert_with(|| Bar {
        fill: fill.clone(), handle: handle.clone(), axis, reverse, samples: VecDeque::new(),
    });
    let period = period.clamp(0.005, 1.0);
    // Keep naturally slow meters moving over the interval between game samples.
    let time = time + (period - f64::from(vm.eng.dt)).max(0.0);
    let previous = bar.samples.back().map(|sample| sample.tween.start + sample.tween.duration);
    let start = previous.map_or(time - period, |end| end.max(time - period));
    let wrap = old >= 0.8 && new <= 0.2 &&
        !with_go(&go_of(slider), |go| go.name.contains("HP") || go.name.contains("Health")).unwrap_or(false);
    if previous.is_some_and(|end| end >= time) {
        // Some timers finish a period and wrap in the same invocation.
        let sample = bar.samples.back_mut().unwrap();
        sample.tween.to = [new];
        sample.wrap |= wrap;
    } else {
        bar.samples.push_back(BarSample {
            tween: Tween { start, duration: time - start, from: [old], to: [new] },
            wrap,
        });
        if bar.samples.len() > 32 {
            bar.samples.pop_front();
        }
    }
}

pub(crate) fn world_position_changed(vm: &mut Vm, transform: &Value, before: [f32; 3], after: [f32; 3]) {
    if !vm.eng.motion.enabled { return; }
    if let Some(ghost) = vm.eng.motion.followers.iter_mut().find(|entry| same(&entry.transform, transform)) {
        let offset = [after[0] - vm.eng.mouse.0, after[1] - vm.eng.mouse.1];
        ghost.offset = offset.iter().all(|v| v.abs() <= 48.0).then_some(offset);
    }
    if let Some(panel) = vm.eng.motion.moving.iter_mut().find(|entry| same(&entry.transform, transform)) {
        let (old, new) = ([before[0], before[1]], [after[0], after[1]]);
        panel.tween = vm.eng.motion.clock.and_then(|(time, period)| {
            old.iter().zip(new).all(|(&a, b)| (a - b).abs() <= 5.0).then(|| Tween {
                start: time,
                duration: period.clamp(0.005, 0.1),
                from: panel.tween.as_ref().map_or(old, |tween| tween.at(time)),
                to: new,
            })
        });
    }
}

pub(crate) struct Restore {
    anchors: Vec<(Value, usize, f32, f32)>,
    positions: Vec<(Value, [f32; 3])>,
    colors: Vec<(Value, Col)>,
}

impl Restore {
    pub fn apply(self, vm: &mut Vm) {
        for (tr, axis, min, max) in self.anchors {
            with_tr(&tr, |t| { t.amin[axis] = min; t.amax[axis] = max; });
            mark_dirty(&tr);
        }
        for (tr, position) in self.positions {
            set_world_pos(vm, &tr, position);
            mark_dirty(&tr);
        }
        for (comp, color) in self.colors {
            with_comp(&comp, |c| if let CK::Text(text) = &mut c.k { text.color = color; });
            mark_dirty(&comp);
        }
    }
}

pub(crate) fn present(vm: &mut Vm, time: f64) -> Restore {
    let mut undo = Restore { anchors: Vec::new(), positions: Vec::new(), colors: Vec::new() };
    // A frame can process several of the original 20 ms timers together.
    // One fixed-step buffer keeps their timestamps usable between presents.
    let bar_time = time - f64::from(vm.eng.dt);
    vm.eng.motion.bars.retain(|_, bar| {
        while bar.samples.front().is_some_and(|sample| bar_time >= sample.tween.start + sample.tween.duration) {
            bar.samples.pop_front();
        }
        if let Some(sample) = bar.samples.front() {
            let value = sample.at(bar_time);
            let edge = if bar.reverse { 1.0 - value } else { value };
            let fill = if bar.reverse { (edge, 1.0) } else { (0.0, edge) };
            for (tr, (min, max)) in [(&bar.fill, fill), (&bar.handle, (edge, edge))] {
                if let Some((old_min, old_max)) = with_tr(tr, |t| {
                    let old = (t.amin[bar.axis], t.amax[bar.axis]);
                    t.amin[bar.axis] = min;
                    t.amax[bar.axis] = max;
                    old
                }) {
                    if (old_min, old_max) != (min, max) {
                        undo.anchors.push((tr.clone(), bar.axis, old_min, old_max));
                        mark_dirty(tr);
                    }
                }
            }
        }
        !bar.samples.is_empty()
    });
    let panels: Vec<_> = vm.eng.motion.moving.iter().filter_map(|panel| {
        panel.tween.as_ref().filter(|tween| tween.fraction(time) < 1.0)
            .map(|tween| (panel.transform.clone(), tween.at(time)))
    }).collect();
    for (tr, [x, y]) in panels {
        let old = world_pos(vm, &tr);
        set_world_pos(vm, &tr, [x, y, old[2]]);
        mark_dirty(&tr);
        undo.positions.push((tr, old));
    }
    for fade in &vm.eng.motion.fades {
        let Some(tween) = fade.tween.as_ref().filter(|tween| tween.fraction(time) < 1.0) else { continue };
        let old = with_comp(&fade.text, |c| match &mut c.k {
            CK::Text(text) => Some(std::mem::replace(&mut text.color, tween.at(time))),
            _ => None,
        }).flatten();
        if let Some(old) = old {
            undo.colors.push((fade.text.clone(), old));
            mark_dirty(&fade.text);
        }
    }
    undo
}

pub(crate) fn is_follower(vm: &Vm, transform: &Value) -> bool {
    vm.eng.motion.followers.iter().any(|entry| same(&entry.transform, transform))
}

pub(crate) fn followers(vm: &Vm) -> Vec<(Value, crate::ui::Xf)> {
    let (mx, my) = vm.eng.mouse;
    if !vm.eng.mouse_down || !(0.0..vm.eng.desktop_layout.width()).contains(&mx)
        || !(0.0..vm.eng.screen.1 as f32).contains(&my) {
        return Vec::new();
    }
    vm.eng.motion.followers.iter().filter_map(|item| {
        let [dx, dy] = item.offset?;
        if !active_in_hierarchy(&go_of(&item.transform)) { return None; }
        let mut xf = view_xf(vm, &item.transform);
        xf.origin = [mx + dx, my + dy];
        Some((item.transform.clone(), xf))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meter(from: f32, to: f32, wrap: bool) -> BarSample {
        BarSample {
            tween: Tween { start: 0.0, duration: 0.02, from: [from], to: [to] },
            wrap,
        }
    }

    #[test]
    fn fills_advance_at_200_hz_between_game_updates() {
        let bar = meter(0.25, 0.75, false);
        for (ms, expected) in [(0, 0.25), (5, 0.375), (10, 0.5), (15, 0.625), (20, 0.75)] {
            assert_eq!(bar.at(f64::from(ms) / 1000.0), expected);
        }
    }

    #[test]
    fn activity_finishes_forward_but_health_drops_backward() {
        let activity = meter(0.875, 0.125, true);
        let health = meter(0.875, 0.125, false);
        assert_eq!(activity.at(0.01), 0.9375);
        assert_eq!(health.at(0.01), 0.5);
        assert_eq!(activity.at(0.02), 0.125);
        assert_eq!(health.at(0.02), 0.125);
    }
}
