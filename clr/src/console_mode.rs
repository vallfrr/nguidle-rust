//! Console mode switch (`docs/CONSOLE-MODE.md`): an On / Off pair added to Settings > "Some Settings", built from the
//! game's own Tooltips setting so it looks native. Off by default; stored in PlayerPrefs. The PC window reads `enabled`
//! to switch to the console presentation (player column + page in the menu column's width, menu list on hold of L).
use crate::save_slots::{child, image, label, named_go, new_node, style, text, tr};
use crate::unity::*;
use crate::value::Value;
use crate::Vm;

const PREF: &str = "nguConsoleMode";
const TAG: &str = "PortConsoleMode";

pub(crate) struct Switch {
    on: Value,
    off: Value,
}

pub fn enabled(vm: &Vm) -> bool {
    matches!(vm.eng.prefs.get(PREF), Some(Value::I32(1)))
}

pub fn set_enabled(vm: &mut Vm, on: bool) {
    vm.eng.prefs.insert(PREF.into(), Value::I32(on as i32));
    crate::unity_api::save_prefs(vm);
    refresh(vm);
}

/// Builds the switch once the settings menu exists (desktop builds only).
pub(crate) fn prepare(vm: &mut Vm) {
    if !vm.eng.desktop_panels || vm.eng.console_switch.is_some() {
        return;
    }
    let Some(menu) = named_go(vm, "Settings Menu") else { return };
    let panel = child(&menu, "Settings Panel 1");
    let sample = child(&child(&menu, "Settings Panel 2"), "Tooltips");
    let (sample_on, sample_text) = (child(&sample, "On Button"), child(&sample, "Tooltips Text"));
    if panel.is_null() || sample_on.is_null() {
        return;
    }
    let frame = style(&sample_on, |k| match k { CK::Image(i) => Some(i.as_ref().clone()), _ => None });
    let sel = style(&sample_on, |k| match k { CK::Sel(s) => Some((s.colors, s.transition)), _ => None });
    let caption_style = style(&sample_text, |k| match k { CK::Text(t) => Some(t.as_ref().clone()), _ => None });
    let button_style = style(&child(&sample_on, "Text"), |k| match k { CK::Text(t) => Some(t.as_ref().clone()), _ => None });
    // Panel 1 is 160 x 480; the free band sits between the resolution buttons and "Number Display Style".
    let title = new_node(vm, "Console Mode Text", &tr(&panel), [0.0, 1.0], 180.0, 18.0);
    let caption = text(vm, &title, caption_style.as_ref(), None, 4);
    label(&caption, "Console Mode".into());
    let button = |vm: &mut Vm, name: &str, caption: &str, x: [f32; 2]| {
        let go = new_node(vm, name, &tr(&panel), x, 202.0, 30.0);
        let graphic = image(vm, &go, frame.as_ref(), true);
        let mut s = default_sel(SelKind::Button);
        if let Some((colors, transition)) = sel {
            s.colors = colors;
            s.transition = transition;
        }
        s.target_graphic = graphic;
        let comp = new_comp(vm, "UnityEngine.UI.Button", go.clone(), CK::Sel(Box::new(s)));
        with_go(&go, |g| g.comps.push(comp));
        let t = new_node(vm, "Text", &tr(&go), [0.0, 1.0], 0.0, 30.0);
        let t = text(vm, &t, button_style.as_ref(), None, 4);
        label(&t, caption.into());
        go
    };
    let on = button(vm, "Console Mode On", "On", [8.0 / 160.0, 78.0 / 160.0]);
    let off = button(vm, "Console Mode Off", "Off", [82.0 / 160.0, 152.0 / 160.0]);
    for go in [&title, &on, &off] {
        with_go(go, |g| g.tag = TAG.into());
    }
    vm.eng.console_switch = Some(Switch { on, off });
    refresh(vm);
}

/// The game marks the active choice of a setting by making its button non-interactable (darker); same here.
fn refresh(vm: &mut Vm) {
    let Some(s) = &vm.eng.console_switch else { return };
    let on = enabled(vm);
    for (go, interactable) in [(s.on.clone(), !on), (s.off.clone(), on)] {
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
    /// right stick, in game pixels for this poll (y > 0 = towards the bottom of the list)
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

/// Console presentation state, kept in `Engine::console` while the mode is on. The page itself is laid out by the PC
/// layout (`desktop_layout`, `console` flag): the menu column is hidden and the page takes its width.
#[derive(Default)]
pub struct Console {
    pub hold: bool,
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

/// Console mode zoom: game rows shown on screen out of 600 (600 / 540 = 1.11x the PC size at the same window height).
/// The player column fits these rows once its build number goes; menus taller than that scroll.
pub const VIEW_H: f32 = 540.0;
/// Margin kept around the selection when a part scrolls to it.
const MARGIN: f32 = 12.0;

/// Drops the cached focus targets after a scroll offset changed (their presented rectangles moved).
fn relayout(vm: &mut Vm) {
    crate::desktop_layout::rebuild(vm);
    vm.eng.focus_cache = None;
    invalidate_input();
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
        if let Some(build) = named_go(vm, "Build #") {
            set_hidden(&build, true);
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
        vm.eng.desktop_layout.console_view_h = VIEW_H;
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
        let d = &mut vm.eng.desktop_layout;
        (d.console, d.console_hold, d.console_view_h) = (false, false, 0.0);
        (d.page_pan, d.list_pan, d.body_pan) = (0.0, 0.0, 0.0);
        d.bars.clear();
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
        let d = &mut vm.eng.desktop_layout;
        (d.page_pan, d.body_pan) = (0.0, 0.0);
    }

    /// Scrolls whatever part holds the selection so that it is fully on screen.
    fn follow(&mut self, vm: &mut Vm) {
        let Some(r) = vm.eng.focus_rect else { return };
        let d = &vm.eng.desktop_layout;
        let (lo, hi, which) = if self.hold {
            (0.0, d.console_view_h, 0)
        } else if let Some(b) = d.body_view.filter(|b| r[0] >= b[0] && r[2] <= b[2] + 1.0 && r[1] >= b[1] - 1.0) {
            (b[1], b[3], 1)
        } else if r[0] >= 160.0 {
            (0.0, d.console_view_h, 2)
        } else {
            return;
        };
        let delta = if r[3] > hi - MARGIN { r[3] - (hi - MARGIN) } else if r[1] < lo + MARGIN { r[1] - (lo + MARGIN) } else { 0.0 };
        if delta.abs() < 0.5 {
            return;
        }
        self.pan_by(vm, which, delta);
        // the selection moved with its part
        if let Some(r) = vm.eng.focus_rect.as_mut() {
            r[1] -= delta;
            r[3] -= delta;
        }
    }

    /// Adds `delta` rows to a scroll offset (0 the menu list, 1 the menu body, 2 the page); returns whether it moved.
    fn pan_by(&mut self, vm: &mut Vm, which: u8, delta: f32) -> bool {
        let d = &mut vm.eng.desktop_layout;
        let (pan, extra) = match which {
            0 => (&mut d.list_pan, d.list_extra),
            1 => (&mut d.body_pan, d.body_extra),
            _ => (&mut d.page_pan, d.page_extra),
        };
        let next = (*pan + delta).clamp(0.0, extra);
        let moved = (next - *pan).abs() > 0.01;
        *pan = next;
        if moved {
            relayout(vm);
        }
        moved
    }

    /// The part the right stick scrolls: the menu body if there is one, else the page.
    fn stick_target(&self, vm: &Vm) -> Option<u8> {
        let d = &vm.eng.desktop_layout;
        if self.hold {
            (d.list_extra > 0.0).then_some(0)
        } else if d.body_extra > 0.0 {
            Some(1)
        } else {
            (d.page_extra > 0.0).then_some(2)
        }
    }

    /// Applies a controller input; returns the click to perform, if any.
    pub fn input(&mut self, vm: &mut Vm, pad: Pad) -> Option<Click> {
        let modal = match pad {
            Pad::Dir('u') => 'u',
            Pad::Dir('d') => 'd',
            Pad::Activate => 'a',
            Pad::Back => 'b',
            _ => ' ',
        };
        if vm.dropdown_navigate(modal) {
            return None;
        }
        let click = |vm: &mut Vm, button| crate::focus::activation_point(vm).map(|(x, y)| Click { x, y, button });
        match pad {
            Pad::Dir(d) => {
                let before = vm.eng.focus.clone();
                crate::focus::navigate(vm, d);
                // at the edge of a part that scrolls: scroll it and look again (the next rows were clipped)
                if matches!(d, 'u' | 'd') && same(&before, &vm.eng.focus) {
                    let step = if d == 'd' { 40.0 } else { -40.0 };
                    let moved = match self.stick_target(vm) {
                        Some(which) => self.pan_by(vm, which, step),
                        None if crate::focus::in_scroll_view(vm) => {
                            crate::focus::scroll(vm, 0.0, step);
                            true
                        }
                        None => false,
                    };
                    if moved {
                        crate::focus::navigate(vm, d);
                    }
                }
                self.follow(vm);
                None
            }
            Pad::Activate => click(vm, 0),
            Pad::Secondary => click(vm, 1),
            Pad::Back => {
                if self.hold {
                    return None;
                }
                if !vm.back_from_desktop_picker() {
                    crate::focus::clear(vm);
                }
                None
            }
            Pad::Prev | Pad::Next if !self.hold => {
                self.step_menu(vm, if pad == Pad::Prev { -1 } else { 1 });
                None
            }
            Pad::Prev | Pad::Next => None,
            Pad::HoldStart => {
                self.hold = true;
                vm.eng.desktop_layout.list_pan = 0.0;
                self.apply(vm);
                relayout(vm);
                let Some((menus, _)) = self.columns.clone() else { return None };
                let now = current_menu(vm);
                if let Some((go, _)) = menu_buttons(vm, &menus).into_iter().find(|(_, id)| *id == now) {
                    crate::focus::select_control(vm, &go);
                    self.follow(vm);
                }
                None
            }
            Pad::HoldEnd => {
                let chosen = self.columns.clone().and_then(|(menus, _)| {
                    menu_buttons(vm, &menus).into_iter().find(|(go, _)| same(go, &vm.eng.focus))
                });
                self.hold = false;
                self.apply(vm);
                crate::focus::clear(vm);
                if let Some((go, _)) = chosen {
                    open(vm, &go);
                    let d = &mut vm.eng.desktop_layout;
                    (d.page_pan, d.body_pan) = (0.0, 0.0);
                }
                None
            }
            Pad::Scroll(dx, dy) => {
                // a list of the game (its own scroll view) under the selection scrolls first
                if crate::focus::in_scroll_view(vm) && vm.eng.desktop_layout.body_extra <= 0.0 {
                    crate::focus::scroll(vm, dx, dy);
                } else if let Some(which) = self.stick_target(vm) {
                    self.pan_by(vm, which, dy);
                } else {
                    crate::focus::scroll(vm, dx, dy);
                }
                None
            }
        }
    }
}

/// Turns the console presentation on or off to match the setting (the shell calls this every frame).
pub fn sync(vm: &mut Vm) -> bool {
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

/// Click on one of the switch buttons; `true` when handled.
pub(crate) fn click(vm: &mut Vm, go: &Value) -> bool {
    let Some(s) = &vm.eng.console_switch else { return false };
    let choice = if same(go, &s.on) { true } else if same(go, &s.off) { false } else { return false };
    set_enabled(vm, choice);
    true
}
