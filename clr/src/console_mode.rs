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

fn set_hidden(go: &Value, hidden: bool) {
    let changed = with_go(go, |g| std::mem::replace(&mut g.port_hidden, hidden) != hidden).unwrap_or(false);
    if changed {
        mark_dirty(go);
        invalidate_input();
        bump_layout(go);
    }
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
        vm.eng.desktop_layout.console = true;
        vm.eng.desktop_layout.console_hold = self.hold;
    }

    /// Restores the PC presentation.
    fn release(&mut self, vm: &mut Vm) {
        if let Some((menus, player)) = self.columns.take() {
            set_hidden(&menus, false);
            set_hidden(&player, false);
        }
        vm.eng.desktop_layout.console = false;
        vm.eng.desktop_layout.console_hold = false;
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
                // at the edge of a list: scroll it and look again (the next rows are outside its mask)
                if matches!(d, 'u' | 'd') && same(&before, &vm.eng.focus) && crate::focus::in_scroll_view(vm) {
                    crate::focus::scroll(vm, 0.0, if d == 'd' { 40.0 } else { -40.0 });
                    crate::focus::navigate(vm, d);
                }
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
                self.apply(vm);
                crate::render::Renderer::new(0, 0).prepare_layout(vm);
                let Some((menus, _)) = self.columns.clone() else { return None };
                let now = current_menu(vm);
                if let Some((go, _)) = menu_buttons(vm, &menus).into_iter().find(|(_, id)| *id == now) {
                    crate::focus::select_control(vm, &go);
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
                }
                None
            }
            Pad::Scroll(dx, dy) => {
                crate::focus::scroll(vm, dx, dy);
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
