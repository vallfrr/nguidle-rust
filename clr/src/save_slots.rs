//! Local save screens. The game still owns serialization and progression.
mod hosted;
mod storage;
mod view;

pub(crate) use view::{label, prepare_title, refresh_achievements};
use view::*;

pub(crate) use crate::host_platform::before_managed_read;
use crate::host_platform::flush_pending_autosave;
pub(crate) use storage::{atomic_write, write_managed_file};
#[cfg(target_os = "vita")]
pub(crate) use storage::record_autosave_slot;
use storage::*;

use crate::bcl_core::{b64_dec, b64_enc, civil_from_days};
use crate::nrbf::{self, Pv, N};
use crate::unity::*;
use crate::value::{Ext, Obj, Value};
use crate::Vm;
use md5::{Digest, Md5};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

const SLOT_COUNT: usize = 5;
const ROW_COUNT: usize = SLOT_COUNT + 1;
const NO_SLOT: usize = ROW_COUNT;
const DEFAULT_NAME: &str = "Butthead";
const NAME_LIMIT: usize = 24;
pub(crate) const SELECTION_COLOR: [f32; 4] = [0.93, 0.93, 0.68, 1.0];
const ACHIEVEMENT_COLUMNS: usize = 8;
const ACHIEVEMENTS_PER_PAGE: usize = ACHIEVEMENT_COLUMNS * 4;
pub(crate) const CONTENT_LEFT: f32 = 292.0;
const MAX_SAVE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Create,
    Save,
    Load,
    Achievements,
}

#[derive(Clone)]
enum Pending {
    Create(usize),
    Save { index: usize, replace: bool },
    Load(usize),
    Import(PathBuf, usize),
    MainMenu,
}

#[derive(Clone, Copy)]
enum Action {
    Row(usize),
    Delete(usize),
    CancelDelete,
    ConfirmDelete,
    Achievement(usize),
    Primary,
    Secondary,
    HostImport,
    Back,
    Previous,
    Next,
    MainMenu,
}

struct DeleteSlot {
    index: usize,
    modified: SystemTime,
    size: u64,
    final_confirmation: bool,
}

struct Metadata {
    player: String,
    seconds: Option<f64>,
}

enum Entry {
    Empty,
    Invalid,
    Save(Metadata, Option<SystemTime>),
}

#[derive(Default)]
pub(crate) struct SaveSlots {
    mode: Option<Mode>,
    home: Option<Home>,
    home_active: bool,
    new_game: Option<Rc<str>>,
    active_slot: Option<usize>,
    deleting: Option<DeleteSlot>,
    idle_labels: Vec<(Value, String)>,
    idle_fills: Vec<Value>,
    steam_tab: Option<SteamTab>,
    from_title: bool,
    selected: usize,
    pending: Option<Pending>,
    status: String,
    import_files: Option<Vec<PathBuf>>,
    page: usize,
    entries: Vec<Entry>,
    ui: Option<Screen>,
    returned_title: Value,
    hosted: Option<HostedSlots>,
}

struct HostedSlots {
    online: bool,
    locked: u8,
    granted: Option<usize>,
    requested: Option<(usize, Pending)>,
    timer_label: Value,
    file_request: Option<(&'static str, Option<usize>)>,
}

impl SaveSlots {
    #[cfg(target_os = "vita")]
    pub(crate) fn has_home(&self) -> bool {
        self.home.is_some()
    }

    #[cfg(target_os = "vita")]
    pub(crate) fn active_slot(&self) -> Option<usize> {
        self.active_slot
    }

    fn blocked(&self, index: usize) -> bool {
        self.hosted.as_ref().is_some_and(|hosted| {
            !hosted.online || (index < ROW_COUNT && hosted.locked & (1 << index) != 0)
        })
    }

    fn hosted_import_target(&self) -> Option<usize> {
        let index = self.selected;
        (self.hosted.is_some() && self.mode == Some(Mode::Load) && index > 0 && index < ROW_COUNT
            && !self.blocked(index) && matches!(self.entries.get(index), Some(Entry::Empty))
            && self.pending.is_none()).then_some(index)
    }
}

struct Home {
    load: Value,
    autosave: Value,
    create: Value,
    details: Value,
    default: Value,
}

struct SteamTab {
    button: (Value, Value),
    page_two: Value,
    menu: Value,
    bp_grid: Value,
}

struct Screen {
    root: Value,
    panel: Value,
    panel_art: Value,
    sections: [Value; 2],
    title_sidebar: Value,
    locked_sidebar: Value,
    title: Value,
    detail: Value,
    status: Value,
    headings: [Value; 2],
    rows: Vec<(Value, Value)>,
    row_info: Vec<Value>,
    delete_buttons: Vec<(Value, Value)>,
    delete_row: Value,
    delete_prompt: Value,
    delete_cancel: (Value, Value),
    delete_confirm: (Value, Value),
    selection: Value,
    name_panel: Value,
    name_input: Value,
    primary: (Value, Value),
    secondary: (Value, Value),
    hosted_import: (Value, Value),
    back: (Value, Value),
    previous: (Value, Value),
    next: (Value, Value),
    main_menu: (Value, Value),
    achievements: AchievementGrid,
    frame_junctions: [[Value; 2]; 2],
    actions: Vec<(Value, Action)>,
    themed: Vec<(Value, Value)>,
}

struct AchievementGrid {
    root: Value,
    tiles: Vec<((Value, Value), Value)>,
    details: Value,
    icon_frame: Value,
    icon: Value,
    name: Value,
    description: Value,
    state: Value,
    sprites: RefCell<HashMap<(i32, Option<bool>), Value>>,
}

pub(crate) fn game_component(vm: &Vm, class: &str) -> Option<Value> {
    vm.eng
        .user_comps
        .iter()
        .find(|v| matches!(v, Value::Obj(o) if vm.types[o.ty as usize].name == class))
        .cloned()
}

pub(crate) fn game_call(
    vm: &mut Vm,
    component: &Value,
    method: &str,
    args: Vec<Value>,
) -> Result<Value, String> {
    let Value::Obj(obj) = component else {
        return Err("The save controller is unavailable.".into());
    };
    let mid = vm
        .find_method(obj.ty, method, args.len() + 1)
        .ok_or_else(|| format!("The save controller has no {method} method."))?;
    let mut all = vec![component.clone()];
    all.extend(args);
    vm.invoke(mid, all).map_err(|e| {
        vm.report_exc("save screen", component, &e);
        "The game could not complete the save operation.".into()
    })
}

fn write_current(vm: &mut Vm, path: &Path) -> Result<(), String> {
    let data = current_snapshot(vm)?;
    atomic_write(path, data.as_bytes())
        .map_err(|e| format!("Could not write the save: {e}"))
}

fn current_snapshot(vm: &mut Vm) -> Result<Rc<str>, String> {
    let c = game_component(vm, "ImportExport").ok_or("The save controller is unavailable.")?;
    let Value::Str(data) = game_call(vm, &c, "getBase64Data", vec![])? else {
        return Err("The game produced no save data.".into());
    };
    meta(&data)?;
    Ok(data)
}

fn initial_snapshot(vm: &mut Vm, template: &str, name: &str) -> Result<String, String> {
    let epoch = vm.type_id("Epoch");
    let current = vm.find_method(epoch, "Current", 0).ok_or("The game clock is unavailable.")?;
    let now = vm.invoke(current, vec![]).map_err(|_| "Could not read the game clock.")?;
    // The draft objects are used only for serialization and will not become native game objects.
    let pending_native_objects = std::mem::take(&mut vm.eng.deser);
    let result = (|| {
        let Value::Obj(save) = crate::serial::deserialize(vm, &b64_dec(template))
            .map_err(|_| "Could not read the new save template.")? else {
            return Err("The new save template is invalid.");
        };
        let fields = &vm.types[save.ty as usize].fields;
        let data = fields.iter().position(|name| name == "playerData").ok_or("Missing player data.")?;
        let checksum = fields.iter().position(|name| name == "checksum").ok_or("Missing save checksum.")?;
        let Value::Str(encoded) = save.f.borrow()[data].clone() else {
            return Err("The new save has invalid player data.");
        };
        let player = crate::serial::deserialize(vm, &b64_dec(&encoded))
            .map_err(|_| "Could not read the new player data.")?;
        let Value::Obj(player_object) = &player else {
            return Err("The new save has invalid player data.");
        };
        let fields = &vm.types[player_object.ty as usize].fields;
        let last_time = fields.iter().position(|name| name == "lastTime").ok_or("The save timestamp is unavailable.")?;
        let player_name = fields.iter().position(|name| name == "playerName").ok_or("The character name is unavailable.")?;
        {
            let mut values = player_object.f.borrow_mut();
            values[last_time] = now;
            values[player_name] = Value::str(name);
        }
        let bytes = crate::serial::serialize(vm, &player).map_err(|_| "Could not write the new player data.")?;
        let mut fields = save.f.borrow_mut();
        fields[data] = Value::str(&b64_enc(&bytes));
        fields[checksum] = Value::str(&b64_enc(&Md5::digest(&bytes)));
        drop(fields);
        let bytes = crate::serial::serialize(vm, &Value::Obj(save))
            .map_err(|_| "Could not write the new save.")?;
        Ok(b64_enc(&bytes))
    })();
    vm.eng.deser = pending_native_objects;
    result.map_err(String::from)
}

fn apply_save(vm: &mut Vm, dir: &Path, path: &Path, from_title: bool) -> Result<(), String> {
    let (data, _) = read_save(path)?;
    let c = game_component(vm, "ImportExport").ok_or("The save controller is unavailable.")?;
    let decoded = game_call(
        vm,
        &c,
        "getSaveDataFromString",
        vec![Value::str(data.trim())],
    )?;
    if decoded.is_null() {
        return Err("The game rejected the selected save.".into());
    }
    if game_call(vm, &c, "getDataFromString", vec![Value::str(data.trim())])?.is_null() {
        return Err("The saved character could not be loaded.".into());
    }
    let loader = game_component(vm, "OpenFileDialog").ok_or("The save loader is unavailable.")?;
    let backup = dir.join("before-load.txt");
    let autosave = slot_path(dir, 0);
    if !from_title {
        write_current(vm, &backup)?;
    } else if autosave != path && autosave.is_file() {
        let previous = fs::read(&autosave).map_err(|e| format!("Could not read Autosave: {e}"))?;
        atomic_write(&backup, &previous)
            .map_err(|e| format!("Could not back up Autosave: {e}"))?;
    }
    let outcome = game_call(vm, &loader, "loadintoGame", vec![decoded]);
    if outcome.as_ref().is_ok_and(Value::as_bool) {
        return Ok(());
    }
    if !from_title {
        if let Ok((previous, _)) = read_save(&backup) {
            let _ = game_call(
                vm,
                &c,
                "loadBase64ToData",
                vec![Value::str(previous.trim())],
            );
        }
    }
    Err(outcome
        .err()
        .unwrap_or_else(|| "The game rejected the selected save.".into()))
}

fn return_to_title(vm: &mut Vm, dir: &Path) -> Result<Value, String> {
    let title = vm
        .eng
        .gos
        .iter()
        .find(|go| with_go(go, |g| g.name == "Main Menu Screen").unwrap_or(false))
        .cloned()
        .ok_or("The main menu is unavailable.")?;
    let controller =
        game_component(vm, "MainMenuController").ok_or("The main menu is unavailable.")?;
    let c = game_component(vm, "ImportExport").ok_or("The save controller is unavailable.")?;
    let backup = dir.join("before-main-menu.txt");
    write_current(vm, &backup)?;
    let (snapshot, _) = read_save(&backup)?;
    let save = game_call(
        vm,
        &c,
        "getSaveDataFromString",
        vec![Value::str(snapshot.trim())],
    )?;
    let player = game_call(
        vm,
        &c,
        "getDataFromString",
        vec![Value::str(snapshot.trim())],
    )?;
    if save.is_null() || player.is_null() {
        return Err("The game rejected the snapshot.".into());
    }
    atomic_write(&slot_path(dir, 0), snapshot.as_bytes())
        .map_err(|e| format!("Could not update Autosave: {e}"))?;
    game_call(vm, &controller, "setLocalSave", vec![save])?;
    game_call(vm, &controller, "setLocalPlayerData", vec![player])?;
    game_call(
        vm,
        &controller,
        "setLocalSaveValidity",
        vec![Value::bool(true)],
    )?;
    game_call(vm, &controller, "startMainMenu", vec![])?;
    Ok(title)
}

pub(crate) fn new_node(vm: &mut Vm, name: &str, parent: &Value, x: [f32; 2], y: f32, height: f32) -> Value {
    let ty = vm.type_id("UnityEngine.GameObject");
    let go = Value::Obj(Obj::new(
        ty,
        0,
        Ext::Native(Box::new(GoData {
            name: name.into(),
            active: true,
            port_hidden: false,
            layer: 0,
            tag: "PortSaveScreen".into(),
            comps: vec![],
            transform: Value::Null,
            destroyed: false,
            hier: (0, false),
            dirty: true,
            cache: None,
            in_layout: false,
        })),
    ));
    let tr = new_comp(
        vm,
        "UnityEngine.RectTransform",
        go.clone(),
        CK::Transform(Box::new(Tr {
            parent: parent.clone(),
            children: vec![],
            pos: [0.0; 3],
            scale: [1.0; 3],
            rot_z: 0.0,
            amin: [x[0], 1.0],
            amax: [x[1], 1.0],
            pivot: [0.0, 1.0],
            apos: [0.0, -y],
            size: [0.0, height],
            is_rect: true,
        })),
    );
    with_go(&go, |g| {
        g.transform = tr.clone();
        g.comps.push(tr.clone());
    });
    with_tr(parent, |p| p.children.push(tr));
    vm.eng.gos.push(go.clone());
    go
}

fn attach(vm: &mut Vm, go: &Value, name: &str, kind: CK) -> Value {
    let comp = new_comp(vm, name, go.clone(), kind);
    with_go(go, |g| g.comps.push(comp.clone()));
    comp
}

pub(crate) fn image(vm: &mut Vm, go: &Value, sample: Option<&ImageD>, raycast: bool) -> Value {
    let mut image = sample.cloned().unwrap_or(ImageD {
        typ: 0,
        sprite: Value::Null,
        color: [0.66, 0.76, 0.83, 1.0],
        raycast,
        fill_center: true,
        fill_method: 0,
        fill_amount: 1.0,
        fill_origin: 0,
        clockwise: false,
        preserve_aspect: false,
    });
    image.raycast = raycast;
    attach(vm, go, "UnityEngine.UI.Image", CK::Image(Box::new(image)))
}

fn set_interactable(go: &Value, enabled: bool) {
    for comp in with_go(go, |g| g.comps.clone()).unwrap_or_default() {
        with_comp(&comp, |c| {
            if let CK::Sel(s) = &mut c.k {
                if std::mem::replace(&mut s.interactable, enabled) != enabled {
                    invalidate_input();
                }
            }
        });
    }
    mark_sel_dirty(go);
}

fn icon_in_frame(vm: &mut Vm, frame: &Value, size: f32) -> Value {
    let inset = 2.8;
    let art = new_node(vm, "Achievement Icon", &tr(frame), [0.0, 1.0], inset, size - 2.0 * inset);
    with_tr(&tr(&art), |t| {
        t.apos[0] = inset;
        t.size[0] = -2.0 * inset;
    });
    image(vm, &art, None, false)
}

pub(crate) fn text(vm: &mut Vm, go: &Value, sample: Option<&TextD>, size: Option<i32>, align: i32) -> Value {
    let mut t = sample.cloned().unwrap_or(TextD {
        text: String::new(),
        font: Value::Null,
        size: 16,
        style: 0,
        best_fit: false,
        min_size: 12,
        max_size: 16,
        align,
        rich: false,
        hov: 0,
        vov: 0,
        line_spacing: 1.0,
        color: [0.0, 0.0, 0.0, 1.0],
        raycast: false,
        layout: None,
    });
    t.text.clear();
    t.size = size.unwrap_or(t.size);
    t.align = align;
    t.raycast = false;
    t.rich = false;
    t.best_fit = false;
    t.hov = 0;
    t.vov = 0;
    t.layout = None;
    attach(vm, go, "UnityEngine.UI.Text", CK::Text(Box::new(t)))
}

pub(crate) fn tr(go: &Value) -> Value {
    with_go(go, |g| g.transform.clone()).unwrap_or_default()
}

pub(crate) fn named_go(vm: &Vm, name: &str) -> Option<Value> {
    vm.eng.gos.iter().find(|go| {
        with_go(go, |g| g.name == name).unwrap_or(false)
    }).cloned()
}

pub(crate) fn child(parent: &Value, name: &str) -> Value {
    with_tr(&tr(parent), |p| {
        p.children.iter().find_map(|node| {
            let go = go_of(node);
            if with_go(&go, |g| g.name == name).unwrap_or(false) {
                Some(go)
            } else {
                None
            }
        })
    })
    .flatten()
    .unwrap_or_default()
}

pub(crate) fn style<T>(go: &Value, f: impl Fn(&CK) -> Option<T>) -> Option<T> {
    with_go(go, |g| {
        g.comps
            .iter()
            .find_map(|c| with_comp(c, |c| f(&c.k)).flatten())
    })
    .flatten()
}

fn set_image_sprite(comp: &Value, sprite: Value) {
    with_comp(comp, |c| {
        if let CK::Image(image) = &mut c.k {
            image.sprite = sprite;
            image.color = [1.0; 4];
            image.preserve_aspect = true;
        }
    });
}

fn button_icon(vm: &mut Vm, button: &(Value, Value), sprite: &Value) {
    if sprite.is_null() {
        return;
    }
    let icon = new_node(vm, "Button Icon", &tr(&button.0), [0.19, 0.81], 5.0, 27.0);
    let graphic = image(vm, &icon, None, false);
    set_image_sprite(&graphic, sprite.clone());
    with_comp(&button.1, |c| {
        if let CK::Text(text) = &mut c.k {
            text.color[3] = 0.0;
        }
    });
}

pub(crate) fn owns(go: &Value) -> bool {
    with_go(go, |g| g.tag == "PortSaveScreen").unwrap_or(false)
}

pub(crate) fn is_open(vm: &Vm) -> bool {
    vm.eng.save_slots.mode.is_some()
}

pub(crate) fn covers_game_content(vm: &Vm) -> bool {
    matches!(vm.eng.save_slots.mode, Some(Mode::Create | Mode::Save | Mode::Load))
}

pub(crate) fn content_panels(vm: &Vm) -> Option<(Value, Value)> {
    if !covers_game_content(vm) {
        return None;
    }
    vm.eng.save_slots.ui.as_ref().map(|ui| (tr(&ui.panel), tr(&ui.locked_sidebar)))
}

pub(crate) fn steam_achievements_panel(vm: &Vm) -> Option<Value> {
    let slots = &vm.eng.save_slots;
    (slots.mode == Some(Mode::Achievements)).then(|| slots.ui.as_ref().map(|ui| tr(&ui.panel))).flatten()
}

pub(crate) fn frame_junctions(vm: &Vm) -> Option<[Value; 2]> {
    let console = vm.eng.desktop_layout.console;
    vm.eng.save_slots.ui.as_ref().map(|ui| ui.frame_junctions[usize::from(console)].clone())
}

pub(crate) fn close_for_console_navigation(vm: &mut Vm) {
    if is_open(vm) && !choosing_save(vm) {
        let mut state = std::mem::take(&mut vm.eng.save_slots);
        close(vm, &mut state);
        vm.eng.save_slots = state;
    }
}

pub(crate) fn choosing_save(vm: &Vm) -> bool {
    vm.eng.save_slots.from_title && vm.eng.save_slots.home.is_some()
}

fn slot_available(mode: Mode, entry: Option<&Entry>) -> bool {
    match mode {
        Mode::Create => matches!(entry, Some(Entry::Empty)),
        _ => matches!(entry, Some(Entry::Save(..))),
    }
}

fn show_idle_stats(vm: &mut Vm, state: &mut SaveSlots) {
    let Some(panel) = named_go(vm, "Player Panel") else { return };
    let stats = child(&panel, "Stats Panel");
    let hp = child(&stats, "HP Bar");
    let timer = child(&panel, "Rebirth Time Text");
    for (node, value) in [
        (child(&panel, "Energy Bar Text"), "<b>Energy:</b> 0 Idle\n0 Cap"),
        (child(&panel, "Magic Bar Text"), "<b>Magic:</b> 0"),
        (child(&panel, "Res 3 Bar Text"), "<b>Resource 3:</b> 0"),
        (child(&hp, "Text"), "0 HP"),
        (child(&child(&stats, "Attack"), "Value"), "0"),
        (child(&child(&stats, "Defense"), "Value"), "0"),
        (child(&child(&stats, "Gold"), "Value"), "0"),
        (timer, "No game loaded"),
    ] {
        for comp in with_go(&node, |g| g.comps.clone()).unwrap_or_default() {
            if let Some(text) = with_comp(&comp, |c| match &c.k {
                CK::Text(text) => Some(text.text.clone()),
                _ => None,
            }).flatten() {
                label(&comp, value.into());
                state.idle_labels.push((comp, text));
            }
        }
    }
    for bar in [child(&panel, "Energy Bar"), child(&panel, "Magic Bar"), child(&panel, "Res 3 Bar"), hp] {
        let fill = child(&child(&bar, "Fill Area"), "Fill");
        if with_go(&fill, |g| g.active).unwrap_or(false) {
            vm.set_active(&fill, false);
            state.idle_fills.push(fill);
        }
    }
}

fn restore_idle_stats(vm: &mut Vm, state: &mut SaveSlots) {
    for (comp, previous) in state.idle_labels.drain(..) {
        label(&comp, previous);
    }
    for fill in state.idle_fills.drain(..) {
        vm.set_active(&fill, true);
    }
}

fn place_home_node(node: &Value, parent: &Value, position: [f32; 2], size: [f32; 2]) {
    let transform = tr(node);
    let old_parent = with_tr(&transform, |t| t.parent.clone()).unwrap_or_default();
    if !same(&old_parent, parent) {
        with_tr(&old_parent, |t| t.children.retain(|child| !same(child, &transform)));
        with_tr(parent, |t| t.children.push(transform.clone()));
    }
    crate::unity::bump_hierarchy();
    with_tr(&transform, |t| {
        t.parent = parent.clone();
        t.amin = [0.5; 2];
        t.amax = [0.5; 2];
        t.pivot = [0.5; 2];
        t.apos = position;
        t.size = size;
    });
    mark_dirty(node);
}

fn prepare_home(vm: &mut Vm) -> Result<Home, String> {
    let title = named_go(vm, "Main Menu Screen").ok_or("The home screen is unavailable.")?;
    let pod = child(&title, "Autosave Pod Border");
    let autosave = child(&pod, "Autosave Button");
    let load = child(&title, "Load File Button");
    let create = child(&title, "Start New Game");
    let details_node = child(&pod, "Autosave Details");
    let details = with_go(&details_node, |g| {
        g.comps.iter().find(|c| with_comp(c, |c| matches!(c.k, CK::Text(_))) == Some(true)).cloned()
    }).flatten().ok_or("The autosave details are unavailable.")?;
    if [&autosave, &load, &create].iter().any(|go| go.is_null()) {
        return Err("The home buttons are unavailable.".into());
    }
    let credits = child(&title, "By me");
    let credit_lines = with_go(&credits, |go| {
        go.comps.iter().find_map(|comp| {
            with_comp(comp, |c| match &c.k {
                CK::Text(text) => Some((comp.clone(), text.text.clone(), text.size as f32)),
                _ => None,
            }).flatten()
        })
    }).flatten();
    if let Some((caption, original, line_height)) = credit_lines {
        label(&caption, format!("{original}\n<b>Ported by vallfrr</b>"));
        // Expand upward so the last credit keeps the original bottom margin.
        let height = line_height + 3.0;
        with_tr(&tr(&credits), |transform| {
            transform.size[1] += height;
            transform.apos[1] += height * transform.pivot[1];
        });
    }
    for name in ["Cloud Save Border", "RightPic", "Text", "Discord Button (1)", "Kartridge Button", "Kong Button"] {
        let node = child(&title, name);
        with_go(&node, |g| g.port_hidden = true);
        mark_dirty(&node);
    }
    let banner = child(&pod, "LeftBanner");
    with_go(&banner, |g| g.port_hidden = true);
    let options = new_node(vm, "Desktop Home Options", &tr(&title), [0.5, 0.5], 0.0, 600.0);
    with_tr(&tr(&options), |t| {
        t.pivot[0] = 0.5;
        t.size[0] = 900.0;
    });
    let parent = tr(&options);
    for (name, x, size, bottom) in [
        ("Chicken", -255.0, [195.0, 117.0], -77.0),
        ("LeftPic", 0.0, [206.0, 190.0], -77.0),
        ("Brian", 255.0, [205.0, 190.0], -94.0),
    ] {
        place_home_node(&child(&title, name), &parent, [x, bottom + size[1] / 2.0], size);
    }
    place_home_node(&pod, &parent, [0.0, -207.0], [226.0, 82.0]);
    place_home_node(&details_node, &tr(&pod), [0.0, 0.0], [206.0, 70.0]);
    with_comp(&details, |c| {
        if let CK::Text(text) = &mut c.k {
            text.size = 17;
            text.align = 4;
        }
    });
    for (go, x, caption) in [
        (&load, -255.0, "Load Game"),
        (&autosave, 0.0, "Load Autosave"),
        (&create, 255.0, "Create New Game"),
    ] {
        let (size, font_size) = if x == 0.0 { ([252.0, 72.0], 22) } else { ([216.0, 60.0], 20) };
        place_home_node(go, &parent, [x, -106.0], size);
        let caption_node = child(go, "Button Text");
        place_home_node(&caption_node, &tr(go), [0.0, 0.0], [211.0, 52.0]);
        for comp in with_go(&caption_node, |g| g.comps.clone()).unwrap_or_default() {
            if with_comp(&comp, |c| {
                if let CK::Text(text) = &mut c.k {
                    text.size = font_size;
                    text.best_fit = false;
                    true
                } else {
                    false
                }
            }) == Some(true) {
                label(&comp, format!("<b>{caption}</b>"));
            }
        }
    }
    Ok(Home { load, autosave, create, details, default: Value::Null })
}

fn show_home(vm: &mut Vm, state: &mut SaveSlots, dir: &Path) -> Result<(), String> {
    flush_pending_autosave(vm);
    let title = game_component(vm, "MainMenuController").ok_or("The main menu is unavailable.")?;
    game_call(vm, &title, "startMainMenu", vec![])?;
    restore_idle_stats(vm, state);
    close(vm, state);
    state.returned_title = Value::Null;
    state.home_active = true;
    let entries = load_entries(dir);
    let autosave_available = !state.blocked(0) && matches!(entries.first(), Some(Entry::Save(..)));
    let numbered_available = entries[1..].iter().any(|entry| matches!(entry, Entry::Save(..)));
    let description = if state.blocked(0) {
        if state.hosted.as_ref().is_some_and(|b| b.online) { "Autosave in use on another device".into() }
        else { "Save server unavailable".into() }
    } else { match entries.first() {
        Some(Entry::Save(info, _)) => {
            let name = if info.player.is_empty() { "Unknown character" } else { &info.player };
            let played = info.seconds.map(play_time).unwrap_or_else(|| "Unknown".into());
            format!("<b>{name}</b>\nPlayed: {played}")
        }
        Some(Entry::Invalid) => "Autosave unreadable".into(),
        _ => "No autosave yet".into(),
    }};
    let home = state.home.as_mut().ok_or("The home screen is unavailable.")?;
    label(&home.details, description);
    set_interactable(&home.autosave, autosave_available);
    home.default = if autosave_available { home.autosave.clone() }
        else if numbered_available { home.load.clone() }
        else { home.create.clone() };
    crate::focus::clear(vm);
    Ok(())
}

fn open_picker(vm: &mut Vm, state: &mut SaveSlots, dir: &Path, mode: Mode) -> Result<(), String> {
    let title = game_component(vm, "MainMenuController").ok_or("The main menu is unavailable.")?;
    game_call(vm, &title, "hideMainMenu", vec![])?;
    state.home_active = false;
    crate::focus::clear(vm);
    show_idle_stats(vm, state);
    open(vm, state, dir, mode, true);
    Ok(())
}

impl Vm {
    pub fn start_desktop_save_picker(&mut self) -> Result<(), String> {
        if self.eng.save_slots.home.is_some() {
            return Ok(());
        }
        // Initialize the original scene before taking a clean, player-independent snapshot.
        self.start_pending();
        if let Some(character) = game_component(self, "Character") {
            // Local desktop startup owns the title and save discovery.
            self.eng.invokes.retain(|invoke| {
                invoke.method.as_ref() != "initialStart" || !same(&invoke.owner, &character)
            });
        }
        let snapshot = current_snapshot(self)?;
        let home = prepare_home(self)?;
        let dir = PathBuf::from(&self.eng.save_dir);
        let mut state = std::mem::take(&mut self.eng.save_slots);
        state.home = Some(home);
        state.new_game = Some(snapshot);
        let result = show_home(self, &mut state, &dir);
        self.eng.save_slots = state;
        result
    }

    pub fn at_desktop_start(&self) -> bool {
        self.eng.save_slots.home_active || choosing_save(self)
    }

    pub fn back_from_desktop_picker(&mut self) -> bool {
        if !choosing_save(self) {
            return false;
        }
        let mut state = std::mem::take(&mut self.eng.save_slots);
        let dir = PathBuf::from(&self.eng.save_dir);
        handle(self, &mut state, &dir, Action::Back);
        draw_state(self, &state);
        self.eng.save_slots = state;
        true
    }
}

pub(crate) fn home_default(vm: &Vm) -> Option<Value> {
    let state = &vm.eng.save_slots;
    state.home.as_ref().filter(|_| state.home_active).map(|home| home.default.clone())
}

pub(crate) fn pause_on_title(vm: &mut Vm) -> bool {
    if vm.at_desktop_start() {
        return true;
    }
    let title = &vm.eng.save_slots.returned_title;
    if title.is_null() {
        return false;
    }
    let visible = crate::ui::xf(vm, &tr(title)).origin[0].abs() < vm.eng.screen.0 as f32 * 2.0;
    if !visible {
        vm.eng.save_slots.returned_title = Value::Null;
    }
    visible
}

pub(crate) fn reflow(vm: &mut Vm) {
    if let Some(tab) = &vm.eng.save_slots.steam_tab {
        copy_theme_sprite(&tab.button.0, &tab.page_two);
        if vm.eng.save_slots.mode == Some(Mode::Achievements) &&
            crate::ui::xf(vm, &tr(&tab.menu)).origin[0].abs() > vm.eng.screen.0 as f32 {
            let mut state = std::mem::take(&mut vm.eng.save_slots);
            close(vm, &mut state);
            vm.eng.save_slots = state;
        }
    }
    let width = vm.eng.desktop_layout.width() - CONTENT_LEFT;
    if let Some(ui) = &vm.eng.save_slots.ui {
        if with_tr(&tr(&ui.panel), |t| {
            if t.size[0] == width {
                return false;
            }
            t.size[0] = width;
            true
        })
        .unwrap_or(false)
        {
            mark_dirty(&ui.panel);
        }
    }
}

fn close(vm: &mut Vm, state: &mut SaveSlots) {
    if state.mode == Some(Mode::Achievements) {
        if let Some(tab) = &state.steam_tab {
            vm.set_active(&tab.bp_grid, true);
            caption(&tab.button, "Steam Achievements", false);
        }
    }
    state.mode = None;
    state.from_title = false;
    state.pending = None;
    state.deleting = None;
    state.import_files = None;
    if let Some(hosted) = &mut state.hosted { hosted.requested = None; }
    if let Some(ui) = &state.ui {
        if vm.eng.text_req.as_ref().is_some_and(|input| same(&input.comp, &ui.name_input)) {
            vm.eng.text_req = None;
        }
        vm.set_active(&ui.root, false);
    }
}

fn open(vm: &mut Vm, state: &mut SaveSlots, dir: &Path, mode: Mode, from_title: bool) {
    flush_pending_autosave(vm);
    close(vm, state);
    if state.ui.is_none() {
        state.ui = Some(build(vm));
    }
    state.mode = Some(mode);
    state.from_title = from_title;
    if mode != Mode::Achievements {
        state.entries = load_entries(dir);
    }
    state.selected = match mode {
        Mode::Create => NO_SLOT,
        Mode::Achievements => 0,
        Mode::Save => state.active_slot.unwrap_or_else(|| (1..ROW_COUNT)
            .find(|&i| matches!(state.entries[i], Entry::Empty))
            .unwrap_or(1)),
        Mode::Load if from_title => NO_SLOT,
        Mode::Load => state
            .entries
            .iter()
            .position(|e| matches!(e, Entry::Save(..)))
            .unwrap_or(0),
    };
    state.pending = None;
    state.deleting = None;
    state.page = 0;
    state.import_files = None;
    state.status.clear();
    if from_title && state.home.is_some() && !state.entries[1..].iter().any(|entry| slot_available(mode, Some(entry))) {
        state.status = if mode == Mode::Create { "All numbered slots are occupied." }
            else { "No saved games in the numbered slots." }.into();
    }
    let ui = state.ui.as_ref().unwrap();
    if mode == Mode::Create { vm.input_commit(&ui.name_input, ""); }
    let achievements = mode == Mode::Achievements;
    if achievements {
        if let Some(tab) = &state.steam_tab {
            vm.set_active(&tab.bp_grid, false);
            caption(&tab.button, "Steam Achievements", true);
        }
    } else {
        label(&ui.headings[0], "Save Slots".into());
    }
    label(&ui.headings[1], if achievements { "Details" } else { "Selected Save" }.into());
    with_comp(&ui.panel_art, |c| c.enabled = !achievements);
    vm.set_active(&go_of(&ui.title), !achievements);
    vm.set_active(&go_of(&ui.detail), !achievements);
    for section in &ui.sections {
        with_tr(section, |t| t.apos[1] = if achievements { -133.0 } else { -110.0 });
        mark_dirty(section);
    }
    let footer = tr(&go_of(&ui.status));
    with_tr(&footer, |t| t.apos[1] = if achievements { -552.0 } else { -540.0 });
    mark_dirty(&footer);
    refresh_theme(ui);
    vm.set_active(&ui.root, true);
}

fn perform(
    vm: &mut Vm,
    state: &mut SaveSlots,
    dir: &Path,
    action: Pending,
) -> Result<String, String> {
    flush_pending_autosave(vm);
    let requested_slot = match &action {
        Pending::Load(0) => Some(autosave_slot(dir, &slot_path(dir, 0)).unwrap_or(0)),
        Pending::Load(slot) | Pending::Create(slot) => Some(*slot),
        _ => None,
    };
    if let Some(hosted) = &mut state.hosted {
        if !hosted.online && !matches!(action, Pending::MainMenu) {
            return Err("Save server unavailable.".into());
        }
        if let Some(slot) = requested_slot {
            if hosted.granted != Some(slot) && state.active_slot != Some(slot) {
                if hosted.locked & (1 << slot) != 0 {
                    return Err("Save in use on another device.".into());
                }
                hosted.requested = Some((slot, action));
                return Ok("Checking the save slot…".into());
            }
        }
    }
    match action {
        Pending::Create(index) => {
            let template = state.new_game.as_deref().ok_or("The new game template is unavailable.")?;
            let typed = state.ui.as_ref().and_then(|ui| with_comp(&ui.name_input, |c| {
                if let CK::Sel(s) = &c.k { s.text.clone() } else { String::new() }
            })).unwrap_or_default();
            let name: String = typed.chars().filter(|c| !c.is_control() && !matches!(c, '<' | '>'))
                .take(NAME_LIMIT).collect();
            let name = if name.trim().is_empty() { DEFAULT_NAME } else { name.trim() };
            let data = initial_snapshot(vm, template, name)?;
            let exporter = game_component(vm, "ImportExport").ok_or("The save controller is unavailable.")?;
            let character = game_component(vm, "Character").ok_or("The player is unavailable.")?;
            let menus = game_component(vm, "MenuSwapper").ok_or("The game menus are unavailable.")?;
            let title = game_component(vm, "MainMenuController").ok_or("The main menu is unavailable.")?;
            let intro = game_component(vm, "StartMenu").ok_or("The intro screen is unavailable.")?;
            let decoded = game_call(vm, &exporter, "getSaveDataFromString", vec![Value::str(&data)])?;
            if decoded.is_null() {
                return Err("The game rejected the new save.".into());
            }
            write_save_file(&slot_path(dir, index), data.as_bytes(), false).map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    "This slot already contains a save. Choose an empty slot.".to_string()
                } else {
                    format!("Could not create the save: {error}")
                }
            })?;
            restore_idle_stats(vm, state);
            let start = (|| {
                game_call(vm, &exporter, "loadData", vec![decoded])?;
                game_call(vm, &character, "refreshMenus", vec![])?;
                game_call(vm, &menus, "swapMenu", vec![Value::I32(0)])?;
                game_call(vm, &title, "finishMainMenu", vec![])?;
                game_call(vm, &intro, "intro", vec![])?;
                Ok::<_, String>(())
            })();
            if let Err(error) = start {
                show_idle_stats(vm, state);
                return Err(error);
            }
            state.active_slot = Some(index);
            close(vm, state);
            Ok(String::new())
        }
        Pending::Save { index, replace } => {
            let data = current_snapshot(vm)?;
            write_save_file(&slot_path(dir, index), data.as_bytes(), replace)
                .map_err(|e| format!("Could not write to Slot {index}: {e}"))?;
            state.active_slot.get_or_insert(index);
            if state.hosted.is_some() {
                Ok(format!("Saving Slot {index}…"))
            } else {
                Ok(format!("Saved to Slot {index}."))
            }
        }
        Pending::Import(file, index) => {
            let (data, _) = read_save(&file)?;
            atomic_write(&slot_path(dir, index), data.as_bytes())
                .map_err(|e| format!("Could not import the save: {e}"))?;
            state.import_files = None;
            Ok(format!("Imported into Slot {index}."))
        }
        Pending::Load(index) => {
            let choosing = state.from_title && state.home.is_some();
            let from_title = state.from_title || state.home_active;
            if choosing {
                restore_idle_stats(vm, state);
            }
            let path = slot_path(dir, index);
            if let Err(error) = apply_save(vm, dir, &path, from_title) {
                if choosing {
                    show_idle_stats(vm, state);
                }
                return Err(error);
            }
            if from_title {
                let controller = game_component(vm, "MainMenuController")
                    .ok_or("The main menu is unavailable.")?;
                game_call(vm, &controller, "finishMainMenu", vec![])?;
                state.returned_title = Value::Null;
            }
            state.active_slot = if index == 0 {
                autosave_slot(dir, &path).or_else(|| state.hosted.is_some().then_some(0))
            } else { Some(index) };
            let autosave = if index == 0 { Ok(()) } else {
                current_snapshot(vm).and_then(|data| {
                    atomic_write(&slot_path(dir, 0), data.as_bytes()).map_err(|e| e.to_string())?;
                    if state.home.is_some() { record_autosave_slot(dir, index, data.as_bytes()); }
                    Ok(())
                })
            };
            state.home_active = false;
            crate::focus::clear(vm);
            close(vm, state);
            if let Err(error) = autosave {
                crate::elog!("Game loaded. Autosave could not be updated: {error}");
            }
            if state.hosted.is_some() {
                Ok("Game loaded.".into())
            } else {
                Ok("Game loaded. The previous game is in save/before-load.txt.".into())
            }
        }
        Pending::MainMenu => {
            if state.home.is_some() {
                show_home(vm, state, dir)?;
                state.active_slot = None;
            } else {
                state.returned_title = return_to_title(vm, dir)?;
                close(vm, state);
            }
            Ok(String::new())
        }
    }
}

fn begin_delete(state: &mut SaveSlots, dir: &Path, index: usize) -> Result<(), String> {
    let path = slot_path(dir, index);
    let file = fs::symlink_metadata(&path).map_err(|e| format!("Could not read Slot {index}: {e}"))?;
    if !file.is_file() { return Err(format!("Slot {index} does not contain a save.")); }
    let modified = file.modified().map_err(|e| format!("Could not check Slot {index}: {e}"))?;
    state.deleting = Some(DeleteSlot { index, modified, size: file.len(), final_confirmation: false });
    Ok(())
}

fn confirm_delete(state: &mut SaveSlots, dir: &Path) -> Result<String, String> {
    let Some(mut deletion) = state.deleting.take() else { return Ok(String::new()); };
    let index = deletion.index;
    let path = slot_path(dir, index);
    let unchanged = fs::symlink_metadata(&path).is_ok_and(|file| {
        file.is_file() && file.len() == deletion.size && file.modified().ok() == Some(deletion.modified)
    });
    if !unchanged { return Err(format!("Slot {index} changed. Review it before deleting.")); }
    if !deletion.final_confirmation {
        deletion.final_confirmation = true;
        state.deleting = Some(deletion);
        return Ok(String::new());
    }
    let auto = slot_path(dir, 0);
    let linked = autosave_slot(dir, &auto) == Some(index);
    fs::remove_file(&path).map_err(|error| format!("Could not delete Slot {index}: {error}"))?;
    if linked {
        if let Ok(data) = fs::read(&auto) { record_autosave_slot(dir, 0, &data); }
    }
    if state.selected == index { state.selected = NO_SLOT; }
    Ok(format!("Slot {index} is empty."))
}

fn handle(vm: &mut Vm, state: &mut SaveSlots, dir: &Path, action: Action) {
    let Some(mode) = state.mode else { return };
    if mode == Mode::Achievements {
        let previous_page = state.page;
        match action {
            Action::Back => close(vm, state),
            Action::Achievement(i) => state.selected = state.page * ACHIEVEMENTS_PER_PAGE + i,
            Action::Previous => state.page = state.page.saturating_sub(1),
            Action::Next if (state.page + 1) * ACHIEVEMENTS_PER_PAGE < vm.eng.achievements.entries.len() => {
                state.page += 1;
            }
            _ => {}
        }
        if previous_page != state.page {
            state.selected = state.page * ACHIEVEMENTS_PER_PAGE;
        }
        return;
    }
    match action {
        Action::Achievement(_) => {},
        Action::Delete(index) => {
            state.deleting = None;
            state.status = begin_delete(state, dir, index).err().unwrap_or_default();
        }
        Action::CancelDelete => { state.deleting = None; }
        Action::ConfirmDelete => {
            if state.deleting.as_ref().is_some_and(|d| state.blocked(d.index)) { return; }
            state.status = confirm_delete(state, dir).unwrap_or_else(|e| e);
            state.entries = load_entries(dir);
            crate::focus::clear(vm);
        }
        Action::Back => {
            if state.deleting.take().is_some() {
                state.status.clear();
            } else if state.from_title && state.home.is_some() {
                if let Err(e) = show_home(vm, state, dir) {
                    state.status = e;
                }
            } else if state.pending.take().is_some() {
                state.status.clear();
            } else if state.import_files.take().is_some() {
                state.status.clear();
            } else {
                close(vm, state);
            }
        }
        Action::Row(index) => {
            state.deleting = None;
            let hosted_load = state.hosted.is_some() && mode == Mode::Load;
            if state.from_title && state.home.is_some() &&
                (index == 0 || !hosted_load && !slot_available(mode, state.entries.get(index))) {
                return;
            }
            if let Some(files) = &state.import_files {
                if let Some(path) = files.get(state.page * ROW_COUNT + index) {
                    state.pending = Some(Pending::Import(path.clone(), state.selected));
                    state.status = format!(
                        "Import {} into Slot {}? Any save in that slot will be replaced.",
                        clean_label(&path.file_name().unwrap_or_default().to_string_lossy(), 40),
                        state.selected
                    );
                }
            } else {
                state.pending = None;
                state.selected = index;
                state.status.clear();
            }
        }
        Action::Previous => state.page = state.page.saturating_sub(1),
        Action::Next => state.page += 1,
        Action::HostImport => {
            if let Some(index) = state.hosted_import_target() {
                state.hosted.as_mut().unwrap().file_request = Some(("import", Some(index)));
            }
        }
        Action::MainMenu => {
            state.pending = Some(Pending::MainMenu);
            state.status = "Return to the main menu? Unsaved progress will be lost.".into();
        }
        Action::Primary => {
            if let Some(pending) = state.pending.take() {
                state.status = perform(vm, state, dir, pending).unwrap_or_else(|e| e);
                state.entries = load_entries(dir);
            } else if mode == Mode::Create {
                if state.selected > 0 && slot_available(mode, state.entries.get(state.selected)) {
                    state.status = perform(vm, state, dir, Pending::Create(state.selected))
                        .unwrap_or_else(|e| e);
                    state.entries = load_entries(dir);
                }
            } else if mode == Mode::Save {
                if state.selected == 0 {
                    state.status = "The game manages Autosave. Choose a manual slot.".into();
                } else {
                    let index = state.selected;
                    let same_slot = state.active_slot == Some(index);
                    let occupied = slot_path(dir, index).exists();
                    let action = Pending::Save { index, replace: same_slot || occupied };
                    if same_slot || (!occupied && state.active_slot.is_none()) {
                        state.status = perform(vm, state, dir, action).unwrap_or_else(|e| e);
                        state.entries = load_entries(dir);
                    } else {
                        state.pending = Some(action);
                        state.status = if occupied {
                            format!("Overwrite Slot {index}? The save in that slot will be replaced.")
                        } else {
                            format!("Save a copy in empty Slot {index}?")
                        };
                    }
                }
            } else if matches!(state.entries.get(state.selected), Some(Entry::Save(..))) {
                let action = Pending::Load(state.selected);
                if state.from_title {
                    state.status = perform(vm, state, dir, action).unwrap_or_else(|e| e);
                    state.entries = load_entries(dir);
                } else {
                    state.pending = Some(action);
                    state.status = if state.hosted.is_some() {
                        "Load this save? Unsaved progress in the current game will be lost.".into()
                    } else {
                        "Load this save and replace the current game? The current game will be backed up.".into()
                    };
                }
            } else {
                state.status = "Choose a valid saved game.".into();
            }
        }
        Action::Secondary => {
            state.pending = None;
            if let Some(hosted) = &mut state.hosted {
                hosted.file_request = Some(("export", (mode == Mode::Load).then_some(state.selected)));
                return;
            }
            if mode == Mode::Save {
                if state.selected == 0 {
                    state.status = "Choose a manual slot before importing.".into();
                    return;
                }
                let import_dir = dir.join("import");
                if let Err(e) = fs::create_dir_all(&import_dir) {
                    state.status = format!("Could not open the import folder: {e}");
                    return;
                }
                let mut files: Vec<_> = fs::read_dir(import_dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt")))
                    .collect();
                files.sort();
                state.status = if files.is_empty() {
                    "The import folder is empty. Return to Save Game to try again.".into()
                } else {
                    format!("Import destination: Slot {}", state.selected)
                };
                state.page = 0;
                state.import_files = Some(files);
            } else {
                if !matches!(state.entries.get(state.selected), Some(Entry::Save(..))) {
                    state.status = "Choose a valid saved game.".into();
                    return;
                }
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let name = format!("NGUSave-{stamp}.txt");
                state.status = read_save(&slot_path(dir, state.selected))
                    .and_then(|(data, _)| {
                        atomic_write(&dir.join("export").join(&name), data.as_bytes())
                            .map_err(|e| format!("Could not export the save: {e}"))?;
                        Ok(format!("Exported to save/export/{name}"))
                    })
                    .unwrap_or_else(|e| e);
            }
        }
    }
}

pub(crate) fn click(vm: &mut Vm, go: &Value, x: f32) -> bool {
    if !vm.eng.desktop_panels || vm.eng.save_dir.is_empty() {
        return false;
    }
    if choosing_save(vm) && !owns(go) {
        return true;
    }
    if let Some(ui) = &vm.eng.save_slots.ui {
        if same(&go_of(&ui.name_input), go) { return false; }
        if vm.eng.text_req.as_ref().is_some_and(|input| same(&ui.name_input, &input.comp)) {
            vm.eng.text_req = None;
        }
    }
    if vm.eng.save_slots.home_active {
        let mut state = std::mem::take(&mut vm.eng.save_slots);
        let dir = PathBuf::from(&vm.eng.save_dir);
        let home = state.home.as_ref().expect("desktop home layout");
        let result = if same(&home.autosave, go) {
            perform(vm, &mut state, &dir, Pending::Load(0)).map(|_| ())
        } else if same(&home.create, go) {
            open_picker(vm, &mut state, &dir, Mode::Create)
        } else if same(&home.load, go) {
            open_picker(vm, &mut state, &dir, Mode::Load)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            let _ = show_home(vm, &mut state, &dir);
            if let Some(home) = &state.home {
                label(&home.details, e);
            }
        }
        draw_state(vm, &state);
        vm.eng.save_slots = state;
        return true;
    }
    if !vm.eng.save_slots.returned_title.is_null()
        && with_go(go, |g| g.name == "Start New Game").unwrap_or(false)
    {
        return game_component(vm, "Character")
            .is_none_or(|c| game_call(vm, &c, "hardReset", vec![]).is_err());
    }
    let mode = with_go(go, |g| match g.name.as_str() {
        "Standalone Save" => Some((Mode::Save, false)),
        "Standalone Load" => Some((Mode::Load, false)),
        "Load File Button" => Some((Mode::Load, true)),
        "Steam Achievements Tab" => Some((Mode::Achievements, false)),
        _ => None,
    })
    .flatten();
    if mode.is_none() && vm.eng.save_slots.mode.is_none() {
        return false;
    }
    let mut state = std::mem::take(&mut vm.eng.save_slots);
    let dir = PathBuf::from(&vm.eng.save_dir);
    let handled = if let Some((mode, from_title)) = mode {
        if mode != Mode::Achievements || state.mode != Some(Mode::Achievements) {
            open(vm, &mut state, &dir, mode, from_title);
        }
        true
    } else if owns(go) {
        if let Some(action) = state.ui.as_ref().and_then(|ui| {
            ui.actions
                .iter()
                .find_map(|(node, a)| same(node, go).then_some(*a))
        }) {
            handle(vm, &mut state, &dir, action);
        }
        true
    } else {
        let leaves_steam = state.mode == Some(Mode::Achievements) && with_go(go, |g| {
            matches!(g.name.as_str(), "Page 1" | "Page 2" | "Back to Info n Stuff")
        }).unwrap_or(false);
        let content_left = vm.eng.desktop_layout.content_left();
        if leaves_steam || x < content_left && vm.has_handler(go, crate::input::Ev::Click) {
            close(vm, &mut state);
        }
        false
    };
    draw_state(vm, &state);
    vm.eng.save_slots = state;
    handled
}
