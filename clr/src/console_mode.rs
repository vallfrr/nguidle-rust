//! Console mode (`docs/CONSOLE-MODE.md`): a gamepad-first presentation of the same game, shared by the console ports and
//! tried on PC. Settings > Page 2 > "Bloopydoops" holds its switch (off by default) and its scale (100 / 110 / 120 %),
//! built from the game's own setting buttons and kept in PlayerPrefs.
//!
//! The menu column is hidden: L / R change menu, holding L shows the menu list in place of the player column. The page is
//! laid out by the PC layout (`desktop_layout`) in the freed width. Above 100 % everything is drawn bigger and fewer game
//! rows fit on screen: `desktop_layout` squeezes empty bands, then shortens the game's own scroll views, and scrolls only
//! what still does not fit (`ScrollRegion`, with the game's scrollbar images). This module drives those scrolls from the
//! right stick, the mouse wheel and the selection.
use crate::desktop_layout::{ScrollRegion, COLUMN, LIST};
use crate::save_slots::{child, image, label, named_go, new_node, style, text, tr};
use crate::unity::*;
use crate::value::Value;
use crate::Vm;

const PREF: &str = "nguConsoleMode";
const SCALE_PREF: &str = "nguConsoleScale";
/// Scale steps, in percent of the PC size.
pub const SCALES: [i32; 3] = [100, 110, 120];
const TAG: &str = "PortConsoleMode";
/// Narrowest logical width the PC layout handles without squeezing the page sideways (player column + page).
const MIN_WIDTH: f32 = 834.0;
/// Margin kept around the selection when a part scrolls to it, and the step of a d-pad press at a scroll edge.
const MARGIN: f32 = 12.0;
const STEP: f32 = 40.0;

pub(crate) struct Switch {
    on: Value,
    off: Value,
    scales: Vec<(Value, i32)>,
    /// the game's own setting button and caption these copy their look from (themes)
    sample: Value,
    sample_text: Value,
    captions: Vec<Value>,
}

pub fn enabled(vm: &Vm) -> bool {
    matches!(vm.eng.prefs.get(PREF), Some(Value::I32(1)))
}

pub fn set_enabled(vm: &mut Vm, on: bool) {
    vm.eng.prefs.insert(PREF.into(), Value::I32(on as i32));
    crate::unity_api::save_prefs(vm);
    refresh(vm);
}

/// The chosen scale in percent (100 when unset or not one of `SCALES`).
pub fn scale(vm: &Vm) -> i32 {
    match vm.eng.prefs.get(SCALE_PREF) {
        Some(Value::I32(s)) if SCALES.contains(s) => *s,
        _ => 100,
    }
}

pub fn set_scale(vm: &mut Vm, percent: i32) {
    vm.eng.prefs.insert(SCALE_PREF.into(), Value::I32(percent));
    crate::unity_api::save_prefs(vm);
    refresh(vm);
    vm.eng.desktop_layout.pans.clear();
}

/// Console presentation on a `w` x `h` screen: (screen pixels per game pixel, game rows shown, logical width).
/// At 100 % it is the PC fit; higher scales zoom in, capped where the page would have to be squeezed sideways.
pub fn view(vm: &Vm, w: f32, h: f32) -> (f32, f32, f32) {
    let base = (w / 960.0).min(h / 600.0).max(1e-3);
    let cap = (w / (MIN_WIDTH * base)).max(1.0);
    let k = base * (scale(vm) as f32 / 100.0).min(cap);
    (k, (h / k).min(600.0), w / k)
}

/// Builds the settings once the settings menus exist (desktop builds only): Page 2's empty "Bloopydoops" column.
pub(crate) fn prepare(vm: &mut Vm) {
    if !vm.eng.desktop_panels || vm.eng.console_switch.is_some() {
        return;
    }
    // two "Settings Menu" objects exist (page 1 and page 2): the sample is page 1's Tooltips setting
    let sample_group = vm.eng.gos.iter().filter(|go| with_go(go, |g| g.name == "Settings Menu").unwrap_or(false))
        .map(|menu| child(&child(menu, "Settings Panel 2"), "Tooltips")).find(|g| !g.is_null()).unwrap_or_default();
    // the empty column of Settings page 2 (another node shares its name elsewhere)
    let under = |go: &Value, name: &str| {
        let mut t = with_go(go, |g| g.transform.clone()).unwrap_or_default();
        while !t.is_null() {
            if with_go(&go_of(&t), |g| g.name.trim() == name).unwrap_or(false) {
                return true;
            }
            t = with_tr(&t, |x| x.parent.clone()).unwrap_or_default();
        }
        false
    };
    let Some(panel) = vm.eng.gos.iter().find(|go| {
        with_go(go, |g| g.name == "Augment Settings 1").unwrap_or(false) && under(go, "Settings 2 Canvas")
    }).cloned() else { return };
    let (sample, sample_text) = (child(&sample_group, "On Button"), child(&sample_group, "Tooltips Text"));
    if sample.is_null() {
        return;
    }
    let frame = style(&sample, |k| match k { CK::Image(i) => Some(i.as_ref().clone()), _ => None });
    let sel = style(&sample, |k| match k { CK::Sel(s) => Some((s.colors, s.transition)), _ => None });
    let caption_style = style(&sample_text, |k| match k { CK::Text(t) => Some(t.as_ref().clone()), _ => None });
    let button_style = style(&child(&sample, "Text"), |k| match k { CK::Text(t) => Some(t.as_ref().clone()), _ => None });
    let mut captions = vec![];
    let mut caption = |vm: &mut Vm, name: &str, value: &str, y: f32| {
        let node = new_node(vm, name, &tr(&panel), [0.0, 1.0], y, 20.0);
        let t = text(vm, &node, caption_style.as_ref(), None, 4);
        label(&t, value.into());
        with_go(&node, |g| g.tag = TAG.into());
        captions.push(t);
    };
    // the column is 160 x 480 under its title
    caption(vm, "Console Mode Text", "Console Mode", 40.0);
    caption(vm, "Console Scale Text", "Console Scale", 108.0);
    let button = |vm: &mut Vm, name: &str, value: &str, x: [f32; 2], y: f32| {
        let go = new_node(vm, name, &tr(&panel), x, y, 30.0);
        let graphic = image(vm, &go, frame.as_ref(), true);
        let mut s = default_sel(SelKind::Button);
        if let Some((colors, transition)) = sel {
            s.colors = colors;
            s.transition = transition;
        }
        s.target_graphic = graphic;
        let comp = new_comp(vm, "UnityEngine.UI.Button", go.clone(), CK::Sel(Box::new(s)));
        with_go(&go, |g| {
            g.comps.push(comp);
            g.tag = TAG.into();
        });
        let t = new_node(vm, "Text", &tr(&go), [0.0, 1.0], 0.0, 30.0);
        let t = text(vm, &t, button_style.as_ref(), None, 4);
        label(&t, value.into());
        go
    };
    let on = button(vm, "Console Mode On", "On", [8.0 / 160.0, 78.0 / 160.0], 64.0);
    let off = button(vm, "Console Mode Off", "Off", [82.0 / 160.0, 152.0 / 160.0], 64.0);
    let scales = SCALES.iter().enumerate().map(|(i, &p)| {
        let x0 = 6.0 + i as f32 * 50.0;
        (button(vm, "Console Scale", &format!("{p}%"), [x0 / 160.0, (x0 + 48.0) / 160.0], 132.0), p)
    }).collect();
    vm.eng.console_switch = Some(Switch { on, off, scales, sample, sample_text, captions });
    refresh(vm);
}

/// The game marks the active choice of a setting by making its button non-interactable (darker); same here.
fn refresh(vm: &mut Vm) {
    let Some(s) = &vm.eng.console_switch else { return };
    let (on, now) = (enabled(vm), scale(vm));
    let mut states: Vec<(Value, bool)> = vec![(s.on.clone(), !on), (s.off.clone(), on)];
    states.extend(s.scales.iter().map(|(go, p)| (go.clone(), *p != now)));
    for (go, interactable) in states {
        for comp in with_go(&go, |g| g.comps.clone()).unwrap_or_default() {
            with_comp(&comp, |c| {
                if let CK::Sel(sel) = &mut c.k {
                    sel.interactable = interactable;
                }
            });
        }
        invalidate_input();
        mark_sel_dirty(&go);
        mark_dirty(&go);
    }
}

/// UI themes swap the settings' sprites and colours: the added buttons and captions follow their samples.
fn follow_theme(vm: &Vm) {
    let Some(s) = &vm.eng.console_switch else { return };
    let look = |go: &Value| with_go(go, |g| g.comps.clone()).unwrap_or_default().iter().find_map(|c| {
        with_comp(c, |k| match &k.k { CK::Image(i) => Some((i.sprite.clone(), i.color)), _ => None }).flatten()
    });
    let text_colour = |go: &Value| with_go(go, |g| g.comps.clone()).unwrap_or_default().iter().find_map(|c| {
        with_comp(c, |k| match &k.k { CK::Text(t) => Some(t.color), _ => None }).flatten()
    });
    let Some((sprite, colour)) = look(&s.sample) else { return };
    let caption = text_colour(&s.sample_text);
    let label_colour = text_colour(&child(&s.sample, "Text"));
    let buttons = [s.on.clone(), s.off.clone()].into_iter().chain(s.scales.iter().map(|(g, _)| g.clone()));
    for go in buttons {
        for c in with_go(&go, |g| g.comps.clone()).unwrap_or_default() {
            let changed = with_comp(&c, |k| match &mut k.k {
                CK::Image(i) if !same(&i.sprite, &sprite) || i.color != colour => {
                    i.sprite = sprite.clone();
                    i.color = colour;
                    true
                }
                _ => false,
            }).unwrap_or(false);
            if changed {
                mark_dirty(&go);
            }
        }
        if let Some(col) = label_colour {
            recolour(&child(&go, "Text"), col);
        }
    }
    if let Some(col) = caption {
        for t in &s.captions {
            recolour(&go_of_comp(t), col);
        }
    }
}

fn go_of_comp(comp: &Value) -> Value {
    with_comp(comp, |c| c.go.clone()).unwrap_or_default()
}

fn recolour(go: &Value, col: [f32; 4]) {
    for c in with_go(go, |g| g.comps.clone()).unwrap_or_default() {
        let changed = with_comp(&c, |k| match &mut k.k {
            CK::Text(t) if t.color != col => {
                t.color = col;
                true
            }
            _ => false,
        }).unwrap_or(false);
        if changed {
            mark_dirty(go);
        }
    }
}

// ------------------------------------------------------------------ presentation and controls

/// Controller input, already decoded (the shell handles timing: tap vs hold of L, repeats, stick dead zones).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pad {
    /// d-pad / left stick: 'u', 'd', 'l', 'r'
    Dir(char),
    /// cross: press the selected control
    Activate,
    /// square: the game's right click on the selected control
    Secondary,
    /// circle: close / back
    Back,
    /// L / R tapped: previous / next menu
    Prev,
    Next,
    /// L held: the menu list replaces the player column; released: open the menu under the cursor
    HoldStart,
    HoldEnd,
    /// right stick, in game pixels for this poll (y > 0 = towards the bottom)
    Scroll(f32, f32),
}

/// A click the shell performs over two logic steps (press, release), in engine space (y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Click {
    pub x: f32,
    pub y: f32,
    /// 0 left, 1 right
    pub button: i32,
}

/// Console presentation state, kept in `Engine::console` while the mode is on.
#[derive(Default)]
pub struct Console {
    pub hold: bool,
    focus_before_hold: Value,
    scrollbar_drag: Option<(usize, f32)>,
    /// the menu column and the player column (root canvases)
    columns: Option<(Value, Value)>,
    /// junction bolts of the root canvas: on the menu column's right edge, and on the player column's sections
    bolts: Option<(Vec<Value>, Vec<Value>)>,
}

/// Buttons of the menu column that open a menu (`MenuSwapper.swapMenu(id)`), top to bottom, with their menu id.
fn menu_buttons(vm: &Vm, column: &Value) -> Vec<(Value, i32)> {
    fn walk(vm: &Vm, tr: &Value, out: &mut Vec<(Value, i32, f32)>) {
        let go = go_of(tr);
        if !with_go(&go, |g| g.active).unwrap_or(false) {
            return;
        }
        for c in with_go(&go, |g| g.comps.clone()).unwrap_or_default() {
            let id = with_comp(&c, |k| match &k.k {
                CK::Sel(s) if k.enabled && s.interactable && s.kind == SelKind::Button => with_comp(&s.on_click, |e| match &e.k {
                    CK::Event(ev) => ev.persistent.iter().find(|p| p.method == "swapMenu").map(|p| p.arg_i),
                    _ => None,
                }).flatten(),
                _ => None,
            }).flatten();
            if let Some(id) = id {
                out.push((go.clone(), id, crate::ui::xf(vm, tr).origin[1]));
            }
        }
        for ch in with_tr(tr, |t| t.children.clone()).unwrap_or_default() {
            walk(vm, &ch, out);
        }
    }
    let mut out = vec![];
    walk(vm, &tr(column), &mut out);
    out.sort_by(|a, b| b.2.total_cmp(&a.2));
    out.into_iter().map(|(go, id, _)| (go, id)).collect()
}

fn current_menu(vm: &Vm) -> i32 {
    crate::vm::field_ref(vm, "MenuSwapper", "menuID").and_then(|f| {
        vm.eng.user_comps.iter().find_map(|c| match c {
            Value::Obj(o) if o.ty == f.0 => crate::vm::read_field(c, f),
            _ => None,
        })
    }).map_or(0, |v| v.as_i32())
}

/// Opens a menu the way its button does (the button may be hidden).
fn open(vm: &mut Vm, button: &Value) {
    let event = with_go(button, |g| g.comps.clone()).unwrap_or_default().iter().find_map(|c| with_comp(c, |k| match &k.k {
        CK::Sel(s) => Some(s.on_click.clone()),
        _ => None,
    }).flatten());
    if let Some(event) = event.filter(|e| !e.is_null()) {
        vm.invoke_event(&event, vec![]);
    }
}

/// Bolts placed directly on the root canvas (y down): those on the menu column's right edge (it is hidden in console mode)
/// and those between the player column's sections (covered by the menu list while L is held).
fn junction_bolts(vm: &Vm) -> (Vec<Value>, Vec<Value>) {
    let Some(canvas) = named_go(vm, "Canvas") else { return Default::default() };
    let (mut edge, mut sections) = (vec![], vec![]);
    for node in with_tr(&tr(&canvas), |t| t.children.clone()).unwrap_or_default() {
        let go = go_of(&node);
        if !with_go(&go, |g| g.name.starts_with("Middle Bolt")).unwrap_or(false) {
            continue;
        }
        let x = crate::ui::xf(vm, &node);
        let (cx, cy) = (x.origin[0], 600.0 - x.origin[1]);
        if (250.0..320.0).contains(&cx) {
            edge.push(go);
        } else if cx < 200.0 && (30.0..570.0).contains(&cy) {
            sections.push(go);
        }
    }
    (edge, sections)
}

fn set_hidden(go: &Value, hidden: bool) {
    let changed = with_go(go, |g| std::mem::replace(&mut g.port_hidden, hidden) != hidden).unwrap_or(false);
    if changed {
        mark_dirty(go);
        invalidate_input();
        bump_layout(go);
    }
}


/// Drops the cached focus targets after a scroll offset changed (their presented rectangles moved).
fn relayout(vm: &mut Vm) {
    crate::desktop_layout::rebuild(vm);
    vm.eng.focus_cache = None;
    invalidate_input();
}

/// Scrolls a region by `delta` rows (> 0 = further down). A game scroll view scrolls by itself first (or last when going
/// back up), the console squeeze covers the rest. Returns whether anything moved.
fn scroll_region(vm: &mut Vm, r: &ScrollRegion, delta: f32) -> bool {
    let mut left = delta;
    let mut moved = false;
    let native = |vm: &mut Vm, d: f32| {
        let before = crate::desktop_layout::native_offset(vm, &r.native);
        vm.scroll_by(&r.native, 0.0, d);
        crate::desktop_layout::native_offset(vm, &r.native) - before
    };
    if !r.native.is_null() && left > 0.0 {
        let done = native(vm, left);
        moved |= done.abs() > 0.01;
        left -= done;
    }
    let pans = &mut vm.eng.desktop_layout.pans;
    let p = pans.entry(r.key).or_insert(0.0);
    let next = (*p + left).clamp(0.0, r.extra);
    left -= next - *p;
    moved |= (next - *p).abs() > 0.01;
    *p = next;
    if !r.native.is_null() && left < 0.0 {
        moved |= native(vm, left).abs() > 0.01;
    }
    if moved {
        relayout(vm);
    }
    moved
}

/// The innermost region on screen at a point (rows, y down; game x).
fn region_at(vm: &Vm, x: f32, y: f32) -> Option<ScrollRegion> {
    vm.eng.desktop_layout.regions.iter().filter(|r| contains(r.view, x, y) || contains(r.track, x, y))
        .min_by(|a, b| area(a).total_cmp(&area(b))).cloned()
}

fn contains([left, top, right, bottom]: [f32; 4], x: f32, y: f32) -> bool {
    left <= x && x < right && top <= y && y < bottom
}

fn area(r: &ScrollRegion) -> f32 {
    (r.view[2] - r.view[0]) * (r.view[3] - r.view[1])
}

/// The region holding the selection: the innermost whose columns contain it and whose rows it overlaps (it may lie
/// just under the view after a d-pad move).
fn region_of_focus(vm: &Vm) -> Option<ScrollRegion> {
    let f = vm.eng.focus_rect?;
    let cx = (f[0] + f[2]) / 2.0;
    vm.eng.desktop_layout.regions.iter()
        .filter(|r| cx >= r.view[0] && cx < r.view[2] && f[3] > r.view[1] - 1.0 && f[1] < r.view[3] + 1.0)
        .min_by(|a, b| area(a).total_cmp(&area(b))).cloned()
}

impl Console {
    /// Shows the player column or, while L is held, the menu list in its place; hides the other one.
    pub fn apply(&mut self, vm: &mut Vm) {
        if self.columns.is_none() {
            self.columns = named_go(vm, "Features Canvas").zip(named_go(vm, "Player Panel Canvas"));
        }
        let Some((menus, player)) = self.columns.clone() else { return };
        set_hidden(&menus, !self.hold);
        set_hidden(&player, self.hold);
        let zoomed = vm.eng.desktop_layout.console_view_h < 599.5;
        if let Some(build) = named_go(vm, "Build #") {
            set_hidden(&build, zoomed);
        }
        let (menu_edge, sections) = self.bolts.get_or_insert_with(|| junction_bolts(vm)).clone();
        for b in &menu_edge {
            set_hidden(b, true);
        }
        for b in &sections {
            set_hidden(b, self.hold);
        }
        vm.eng.desktop_layout.console = true;
        vm.eng.desktop_layout.console_hold = self.hold;
    }

    /// Restores the PC presentation.
    fn release(&mut self, vm: &mut Vm) {
        if let Some((menus, player)) = self.columns.take() {
            set_hidden(&menus, false);
            set_hidden(&player, false);
        }
        if let Some(build) = named_go(vm, "Build #") {
            set_hidden(&build, false);
        }
        for b in self.bolts.take().map(|(a, b)| [a, b].concat()).unwrap_or_default() {
            set_hidden(&b, false);
        }
        vm.eng.desktop_layout.reset_console();
        relayout(vm);
    }

    fn step_menu(&mut self, vm: &mut Vm, delta: i32) {
        let Some((menus, _)) = self.columns.clone() else { return };
        let list = menu_buttons(vm, &menus);
        if list.is_empty() {
            return;
        }
        let now = current_menu(vm);
        let at = list.iter().position(|(_, id)| *id == now).map_or(0, |i| i as i32);
        let n = list.len() as i32;
        let next = ((at + delta) % n + n) % n;
        open(vm, &list[next as usize].0);
        crate::focus::clear(vm);
        new_page(vm);
    }

    /// Scrolls whatever region holds the selection so that it is fully on screen.
    fn follow(&mut self, vm: &mut Vm) {
        for _ in 0..4 {
            let (Some(f), Some(r)) = (vm.eng.focus_rect, region_of_focus(vm)) else { return };
            let (lo, hi) = (r.view[1] + MARGIN, r.view[3] - MARGIN);
            let delta = if f[3] > hi { (f[3] - hi).min(f[1] - lo).max(0.0) } else if f[1] < lo { f[1] - lo } else { 0.0 };
            if delta.abs() < 0.5 || !scroll_region(vm, &r, delta) {
                return;
            }
            crate::focus::refresh_rect(vm);
        }
    }

    /// Applies a controller input; returns the click to perform, if any.
    pub fn input(&mut self, vm: &mut Vm, pad: Pad) -> Option<Click> {
        if self.hold {
            match pad {
                Pad::Dir(d @ ('u' | 'd')) => {
                    if let Some((menus, _)) = &self.columns {
                        let list = menu_buttons(vm, menus);
                        if let Some(index) = list.iter().position(|(go, _)| same(go, &vm.eng.focus)) {
                            let next = if d == 'u' { index.checked_sub(1) } else { Some(index + 1) };
                            if let Some((go, _)) = next.and_then(|n| list.get(n)) {
                                crate::focus::select_control(vm, go);
                                self.follow(vm);
                            }
                        }
                    }
                }
                Pad::Scroll(_, dy) => {
                    if let Some(region) = vm.eng.desktop_layout.regions.iter().find(|r| r.key == LIST).cloned() {
                        scroll_region(vm, &region, dy);
                    }
                }
                Pad::HoldEnd | Pad::Back => {
                    let chosen = (pad == Pad::HoldEnd).then(|| {
                        self.columns.as_ref().and_then(|(menus, _)| {
                            menu_buttons(vm, menus).into_iter().find(|(go, _)| same(go, &vm.eng.focus))
                        })
                    }).flatten();
                    self.hold = false;
                    self.apply(vm);
                    crate::focus::clear(vm);
                    let previous = std::mem::take(&mut self.focus_before_hold);
                    let next = chosen.filter(|(_, id)| *id != current_menu(vm));
                    if let Some((go, _)) = &next {
                        crate::save_slots::close_for_console_navigation(vm);
                        open(vm, go);
                        new_page(vm);
                    }
                    relayout(vm);
                    if next.is_none() && active_in_hierarchy(&previous) {
                        crate::focus::select_control(vm, &previous);
                    }
                }
                _ => {}
            }
            return None;
        }
        let modal = match pad {
            Pad::Dir('u') => 'u',
            Pad::Dir('d') => 'd',
            Pad::Activate => 'a',
            Pad::Back => 'b',
            _ => ' ',
        };
        if !vm.eng.desktop_layout.console_modal && vm.dropdown_navigate(modal) {
            return None;
        }
        let click = |vm: &mut Vm, button| crate::focus::activation_point(vm).map(|(x, y)| Click { x, y, button });
        match pad {
            Pad::Dir(d) => {
                let before = vm.eng.focus.clone();
                crate::focus::navigate(vm, d);
                // at the edge of a region that scrolls: scroll it and look again (the next rows were clipped)
                if matches!(d, 'u' | 'd') && same(&before, &vm.eng.focus) {
                    let step = if d == 'd' { STEP } else { -STEP };
                    for _ in 0..6 {
                        let Some(r) = region_of_focus(vm) else { break };
                        if !scroll_region(vm, &r, step) {
                            break;
                        }
                        crate::focus::refresh_rect(vm);
                        crate::focus::navigate(vm, d);
                        if !same(&before, &vm.eng.focus) {
                            break;
                        }
                    }
                }
                self.follow(vm);
                None
            }
            Pad::Activate => click(vm, 0),
            Pad::Secondary => click(vm, 1),
            Pad::Back => {
                if !vm.back_from_desktop_picker() {
                    crate::focus::clear(vm);
                }
                None
            }
            Pad::Prev | Pad::Next if !vm.at_desktop_start() && !vm.eng.desktop_layout.console_modal => {
                crate::save_slots::close_for_console_navigation(vm);
                self.step_menu(vm, if pad == Pad::Prev { -1 } else { 1 });
                None
            }
            Pad::Prev | Pad::Next => None,
            Pad::HoldStart if !vm.at_desktop_start() && !vm.eng.desktop_layout.console_modal => {
                let (menus, _) = self.columns.as_ref()?;
                let now = current_menu(vm);
                let list = menu_buttons(vm, menus);
                let (go, _) = list.iter().find(|(_, id)| *id == now).or(list.first())?;
                self.focus_before_hold = vm.eng.focus.clone();
                self.hold = true;
                vm.eng.desktop_layout.pans.remove(&LIST);
                self.apply(vm);
                relayout(vm);
                crate::focus::select_control(vm, go);
                self.follow(vm);
                None
            }
            Pad::HoldStart | Pad::HoldEnd => None,
            Pad::Scroll(dx, dy) => {
                // the region under the selection, else the one in the middle of the page
                let r = region_of_focus(vm).or_else(|| {
                    let d = &vm.eng.desktop_layout;
                    d.regions.iter().filter(|r| r.key != COLUMN && r.key != LIST).min_by(|a, b| area(a).total_cmp(&area(b))).cloned()
                });
                match r {
                    Some(r) => {
                        scroll_region(vm, &r, dy);
                    }
                    None => crate::focus::scroll(vm, dx, dy),
                }
                None
            }
        }
    }

    /// Mouse wheel at a game point (y up, like `Engine::mouse`); `true` when a console region took it.
    pub fn wheel(&mut self, vm: &mut Vm, x: f32, y: f32, notches: f32) -> bool {
        let Some(r) = region_at(vm, x, 600.0 - y) else { return false };
        scroll_region(vm, &r, -notches * STEP);
        true
    }

    /// Capture the pointer on the same scrollbar geometry used by the renderer.
    pub(crate) fn scrollbar_pointer(&mut self, vm: &mut Vm) -> bool {
        let (x, y) = (vm.eng.mouse.0, 600.0 - vm.eng.mouse.1);
        if vm.eng.mouse_down_edge && vm.eng.mouse_button == 0 {
            if let Some(r) = vm.eng.desktop_layout.regions.iter().find(|r| contains(r.track, x, y)) {
                let pan = vm.eng.desktop_layout.pans.get(&r.key).copied().unwrap_or_default();
                let (top, h) = r.grip(pan);
                let grab = if (top..top + h).contains(&y) { y - top } else { h / 2.0 };
                self.scrollbar_drag = Some((r.key, grab));
            }
        }
        let Some((key, grab)) = self.scrollbar_drag else { return false };
        if vm.eng.mouse_up_edge || !vm.eng.mouse_down {
            self.scrollbar_drag = None;
        } else if let Some(r) = vm.eng.desktop_layout.regions.iter().find(|r| r.key == key).cloned() {
            let pan = vm.eng.desktop_layout.pans.get(&key).copied().unwrap_or_default();
            let (_, h) = r.grip(pan);
            let travel = (r.track[3] - r.track[1] - h).max(0.01);
            let fraction = ((y - grab - r.track[1]) / travel).clamp(0.0, 1.0);
            let delta = (r.native_range + r.extra) * fraction - r.native_off - pan;
            if delta.abs() > 0.01 {
                scroll_region(vm, &r, delta);
            }
        }
        true
    }
}

/// A new menu starts at its top.
fn new_page(vm: &mut Vm) {
    vm.eng.desktop_layout.pans.retain(|k, _| *k == COLUMN);
}

/// Turns the console presentation on or off to match the setting and keeps its scale (the shell calls this every frame
/// with the screen size, before drawing).
pub fn sync(vm: &mut Vm) -> bool {
    follow_theme(vm);
    let on = enabled(vm);
    let mut console = vm.eng.console.take();
    match (on, console.as_mut()) {
        (true, None) => {
            let mut c = Box::<Console>::default();
            c.apply(vm);
            crate::focus::clear(vm);
            console = Some(c);
        }
        (true, Some(c)) => c.apply(vm),
        (false, Some(c)) => {
            c.release(vm);
            crate::focus::clear(vm);
            console = None;
        }
        (false, None) => {}
    }
    vm.eng.console = console;
    on
}

/// Sets the rows shown for a screen size (console mode only); returns (pixels per game pixel, rows, logical width).
pub fn fit_screen(vm: &mut Vm, w: f32, h: f32) -> (f32, f32, f32) {
    let (k, rows, width) = view(vm, w, h);
    let d = &mut vm.eng.desktop_layout;
    if (d.console_view_h - rows).abs() > 0.01 {
        d.console_view_h = rows;
        d.pans.clear();
    }
    (k, rows, width)
}

/// Click on one of the settings buttons; `true` when handled.
pub(crate) fn click(vm: &mut Vm, go: &Value) -> bool {
    let Some(s) = &vm.eng.console_switch else { return false };
    if same(go, &s.on) || same(go, &s.off) {
        let on = same(go, &s.on);
        set_enabled(vm, on);
        return true;
    }
    if let Some(p) = s.scales.iter().find(|(g, _)| same(g, go)).map(|(_, p)| *p) {
        set_scale(vm, p);
        return true;
    }
    false
}
