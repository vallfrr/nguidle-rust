//! The console mode's number pad (`docs/M5-SPEC.md`, R11): pressing cross on a number field opens a pad drawn in the game's
//! own style (the panels and buttons of the settings screens) instead of the system keyboard. Digits, K / M / B (adds
//! three, six or nine zeros), x2, /2, backspace, clear, cancel and OK; Start is OK, circle cancels, square is backspace and
//! triangle clears.
use crate::console_mode::Pad;
use crate::save_slots::{attach, child, image, label, named_go, new_node, style, text, tr};
use crate::unity::*;
use crate::value::Value;
use crate::Vm;

const WIDTH: f32 = 372.0;
const HEIGHT: f32 = 352.0;
const KEY_W: f32 = 64.0;
const KEY_H: f32 = 46.0;
const GAP: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Key {
    Digit(u8),
    DoubleZero,
    Thousand,
    Million,
    Billion,
    Double,
    Half,
    Backspace,
    Clear,
    Cancel,
    Ok,
}

pub(crate) struct NumPad {
    root: Value,
    panel: Value,
    display: Value,
    keys: Vec<(Value, Key)>,
    /// the three sources the look follows (the settings screen's themes)
    themed: Vec<(Value, Value)>,
    open: Option<Entry>,
}

struct Entry {
    field: Value,
    text: String,
    max: usize,
    /// the field shows a percentage ("100%"): the sign is kept out of the way while editing and put back on commit
    percent: bool,
    focus_before: Value,
    /// nothing typed yet: the first digit replaces what the field held
    fresh: bool,
}

/// Whether the console mode takes this field: number fields, and the game's amount boxes that are typed as text.
pub(crate) fn wants(vm: &Vm, content_type: i32, text: &str) -> bool {
    if !crate::console_mode::enabled(vm) {
        return false;
    }
    let digits = text.trim_end_matches('%');
    content_type == 2 || content_type == 3 || (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
}

/// The pad's root object while it is open: nothing outside it can be selected.
pub(crate) fn modal_root(vm: &Vm) -> Option<Value> {
    vm.eng.numpad.as_ref().filter(|p| p.open.is_some()).map(|p| p.root.clone())
}

pub(crate) fn is_open(vm: &Vm) -> bool {
    vm.eng.numpad.as_ref().is_some_and(|p| p.open.is_some())
}

fn build(vm: &mut Vm) -> Option<NumPad> {
    let settings = named_go(vm, "Settings Menu")?;
    let panel_source = child(&settings, "Settings Panel 2");
    let button_source = child(&settings, "Back to Info n Stuff");
    let title_source = child(&settings, "Title");
    let image_style = |go: &Value| style(go, |k| match k { CK::Image(i) => Some(i.as_ref().clone()), _ => None });
    let text_style = |go: &Value| style(go, |k| match k { CK::Text(t) => Some(t.as_ref().clone()), _ => None });
    let (panel_frame, button_frame) = (image_style(&panel_source), image_style(&button_source));
    let heading = text_style(&child(&panel_source, "Text"));
    let body = text_style(&child(&button_source, "Text"));
    let subtitle = text_style(&child(&title_source, "subtitle"));
    let tint = style(&button_source, |k| match k { CK::Sel(s) => Some((s.colors, s.transition)), _ => None });

    let root = new_node(vm, "Number Pad Canvas", &Value::Null, [0.0, 1.0], 0.0, 600.0);
    attach(vm, &root, "UnityEngine.Canvas", CK::Canvas(Box::new(CanvasD { mode: 0, sorting: 10001, override_sorting: true, scale_factor: 1.0 })));
    vm.eng.canvases.push((10001, tr(&root)));
    vm.eng.canvases.sort_by_key(|(sorting, _)| *sorting);
    // an invisible full-screen layer: nothing behind the pad can be reached while it is open
    let block = new_node(vm, "Number Pad Blocker", &tr(&root), [0.0, 1.0], 0.0, 600.0);
    let shade = image(vm, &block, None, true);
    with_comp(&shade, |c| {
        if let CK::Image(i) = &mut c.k {
            i.color = [0.0, 0.0, 0.0, 0.004];
        }
    });
    let panel = new_node(vm, "Number Pad", &tr(&root), [0.0, 0.0], 0.0, HEIGHT);
    with_tr(&tr(&panel), |t| t.size[0] = WIDTH);
    image(vm, &panel, panel_frame.as_ref(), true);
    let mut themed = vec![(panel.clone(), panel_source)];
    let title = new_node(vm, "Title", &tr(&panel), [0.05, 0.95], 10.0, 36.0);
    label(&text(vm, &title, heading.as_ref(), Some(26), 4), "HOW MUCH?".into());
    let sub = new_node(vm, "Subtitle", &tr(&panel), [0.05, 0.95], 44.0, 20.0);
    label(&text(vm, &sub, subtitle.as_ref().or(body.as_ref()), Some(13), 4), "(MATH IS HARD)".into());
    let screen = new_node(vm, "Display", &tr(&panel), [0.0, 0.0], 72.0, 44.0);
    with_tr(&tr(&screen), |t| {
        t.apos[0] = 14.0;
        t.size[0] = WIDTH - 28.0;
    });
    image(vm, &screen, button_frame.as_ref(), false);
    themed.push((screen.clone(), button_source.clone()));
    let shown = new_node(vm, "Text", &tr(&screen), [0.04, 0.96], 0.0, 44.0);
    let display = text(vm, &shown, body.as_ref(), Some(24), 5);

    let mut keys = vec![];
    let mut key = |vm: &mut Vm, caption: &str, k: Key, column: f32, row: f32, columns: f32, size: i32| {
        let go = new_node(vm, "Number Pad Key", &tr(&panel), [0.0, 0.0], 126.0 + row * (KEY_H + GAP), KEY_H);
        let width = columns * KEY_W + (columns - 1.0) * GAP;
        with_tr(&tr(&go), |t| {
            t.apos[0] = 14.0 + column * (KEY_W + GAP);
            t.size[0] = width;
        });
        let graphic = image(vm, &go, button_frame.as_ref(), true);
        let mut sel = default_sel(SelKind::Button);
        if let Some((colors, transition)) = tint {
            sel.colors = colors;
            sel.transition = transition;
        }
        sel.target_graphic = graphic;
        attach(vm, &go, "UnityEngine.UI.Button", CK::Sel(Box::new(sel)));
        let caption_node = new_node(vm, "Text", &tr(&go), [0.0, 1.0], 0.0, KEY_H);
        label(&text(vm, &caption_node, body.as_ref(), Some(size), 4), caption.into());
        themed.push((go.clone(), button_source.clone()));
        keys.push((go, k));
    };
    let rows: [[(&str, Key); 5]; 3] = [
        [("7", Key::Digit(7)), ("8", Key::Digit(8)), ("9", Key::Digit(9)), ("K", Key::Thousand), ("x2", Key::Double)],
        [("4", Key::Digit(4)), ("5", Key::Digit(5)), ("6", Key::Digit(6)), ("M", Key::Million), ("/2", Key::Half)],
        [("1", Key::Digit(1)), ("2", Key::Digit(2)), ("3", Key::Digit(3)), ("B", Key::Billion), ("<-", Key::Backspace)],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, (caption, k)) in row.iter().enumerate() {
            key(vm, caption, *k, c as f32, r as f32, 1.0, 22);
        }
    }
    key(vm, "0", Key::Digit(0), 0.0, 3.0, 1.0, 22);
    key(vm, "00", Key::DoubleZero, 1.0, 3.0, 1.0, 22);
    key(vm, "Clear", Key::Clear, 2.0, 3.0, 1.0, 14);
    key(vm, "Cancel", Key::Cancel, 3.0, 3.0, 1.0, 13);
    key(vm, "OK", Key::Ok, 4.0, 3.0, 1.0, 20);
    let pad = NumPad { root: root.clone(), panel, display, keys, themed, open: None };
    crate::console_mode::set_hidden(&root, true);
    Some(pad)
}

fn show(vm: &mut Vm, pad: &NumPad, visible: bool) {
    crate::console_mode::set_hidden(&pad.root, !visible);
    let _ = vm;
}

/// The panel is centred on the rows and the width shown now.
fn place(vm: &mut Vm, pad: &NumPad) {
    let d = &vm.eng.desktop_layout;
    let rows = if d.console_view_h > 1.0 && d.console_view_h < 599.5 { d.console_view_h } else { 600.0 };
    let (x, y) = (((d.width() - WIDTH) / 2.0).max(0.0), ((rows - HEIGHT) / 2.0).max(0.0));
    with_tr(&tr(&pad.panel), |t| {
        t.apos = [x, -y];
    });
    mark_dirty(&pad.panel);
    if std::env::var_os("NGU_CONSOLE_DEBUG").is_some() {
        eprintln!("[numpad] rows {rows} at ({x}, {y}) apos {:?}", with_tr(&tr(&pad.panel), |t| (t.apos, t.size, t.amin, t.amax)));
    }
}

fn refresh(pad: &NumPad) {
    let Some(entry) = &pad.open else { return };
    let shown = if entry.text.is_empty() { "0".to_string() } else { entry.text.clone() };
    label(&pad.display, if entry.percent { format!("{shown}%") } else { shown });
    mark_dirty(&go_of_comp(&pad.display));
}

fn go_of_comp(comp: &Value) -> Value {
    with_comp(comp, |c| c.go.clone()).unwrap_or_default()
}

/// Opens the pad for a number field (`text` is what the field holds, `max` its character limit, 0 = none).
pub(crate) fn open(vm: &mut Vm, field: &Value, text: &str, max: i32) {
    if vm.eng.numpad.is_none() {
        vm.eng.numpad = build(vm).map(Box::new);
    }
    let Some(mut pad) = vm.eng.numpad.take() else { return };
    let percent = text.ends_with('%');
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    pad.open = Some(Entry {
        field: field.clone(),
        text: if digits == "0" { String::new() } else { digits },
        max: if max > 0 { max as usize - usize::from(percent) } else { 18 },
        percent,
        focus_before: vm.eng.focus.clone(),
        fresh: true,
    });
    place(vm, &pad);
    for (go, source) in pad.themed.clone() {
        crate::save_slots::copy_theme_sprite(&go, &source);
    }
    show(vm, &pad, true);
    refresh(&pad);
    let first = pad.keys.iter().find(|(_, k)| *k == Key::Digit(5)).map(|(go, _)| go.clone());
    vm.eng.numpad = Some(pad);
    crate::desktop_layout::rebuild(vm);
    vm.eng.focus_cache = None;
    invalidate_input();
    if let Some(go) = first {
        crate::focus::select_control(vm, &go);
    }
}

/// Closes the pad; with `commit` the number goes to the field as if it had been typed.
pub(crate) fn close(vm: &mut Vm, commit: bool) {
    let Some(mut pad) = vm.eng.numpad.take() else { return };
    let entry = pad.open.take();
    show(vm, &pad, false);
    vm.eng.numpad = Some(pad);
    invalidate_input();
    let Some(entry) = entry else { return };
    if commit {
        let mut value = if entry.text.is_empty() { "0".to_string() } else { entry.text.clone() };
        if entry.percent {
            value.push('%');
        }
        vm.input_commit(&entry.field, &value);
    }
    vm.eng.focus_cache = None;
    if active_in_hierarchy(&entry.focus_before) {
        crate::focus::select_control(vm, &entry.focus_before);
    } else {
        crate::focus::clear(vm);
    }
}

fn press(entry: &mut Entry, key: Key) -> Option<bool> {
    if entry.fresh && matches!(key, Key::Digit(_) | Key::DoubleZero | Key::Thousand | Key::Million | Key::Billion) {
        entry.text.clear();
    }
    entry.fresh = false;
    let zeros = |entry: &mut Entry, n: usize| {
        let base = if entry.text.is_empty() { "1".to_string() } else { entry.text.clone() };
        if base.len() + n <= entry.max {
            entry.text = base + &"0".repeat(n);
        }
    };
    match key {
        Key::Digit(d) => {
            if entry.text.len() < entry.max {
                entry.text.push((b'0' + d) as char);
                if entry.text == "0" {
                    entry.text.clear();
                }
            }
        }
        Key::DoubleZero => {
            if !entry.text.is_empty() && entry.text.len() + 2 <= entry.max {
                entry.text.push_str("00");
            }
        }
        Key::Thousand => zeros(entry, 3),
        Key::Million => zeros(entry, 6),
        Key::Billion => zeros(entry, 9),
        Key::Double | Key::Half => {
            let now: u128 = entry.text.parse().unwrap_or(0);
            let next = if key == Key::Double { now.saturating_mul(2) } else { now / 2 };
            let text = if next == 0 { String::new() } else { next.to_string() };
            if text.len() <= entry.max {
                entry.text = text;
            }
        }
        Key::Backspace => {
            entry.text.pop();
        }
        Key::Clear => entry.text.clear(),
        Key::Cancel => return Some(false),
        Key::Ok => return Some(true),
    }
    None
}

fn key_pressed(vm: &mut Vm, key: Key) {
    let Some(mut pad) = vm.eng.numpad.take() else { return };
    let outcome = pad.open.as_mut().and_then(|entry| press(entry, key));
    refresh(&pad);
    vm.eng.numpad = Some(pad);
    if let Some(commit) = outcome {
        close(vm, commit);
    }
}

/// A click on one of the pad's keys; `true` when handled.
pub(crate) fn click(vm: &mut Vm, go: &Value) -> bool {
    let Some(key) = vm.eng.numpad.as_ref().filter(|p| p.open.is_some())
        .and_then(|p| p.keys.iter().find(|(k, _)| same(k, go)).map(|(_, key)| *key)) else { return false };
    key_pressed(vm, key);
    true
}

/// Controller input while the pad is open: the console's click on the selected key, circle cancels, Start is OK, square is
/// backspace and triangle clears.
pub(crate) fn input(vm: &mut Vm, pad: Pad) -> Option<crate::console_mode::Click> {
    match pad {
        Pad::Back => close(vm, false),
        Pad::Settings => key_pressed(vm, Key::Ok),
        Pad::Secondary => key_pressed(vm, Key::Backspace),
        Pad::Info => key_pressed(vm, Key::Clear),
        Pad::Dir(d) => {
            crate::focus::navigate(vm, d);
        }
        Pad::Activate => {
            return crate::focus::activation_point(vm).map(|(x, y)| crate::console_mode::Click { x, y, button: 0, phase: crate::console_mode::Phase::Full });
        }
        _ => {}
    }
    None
}
