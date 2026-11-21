//! Local save screens. The game still owns serialization and progression.
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
    sidebar_theme: Option<(Value, Value)>,
    from_title: bool,
    selected: usize,
    pending: Option<Pending>,
    status: String,
    import_files: Option<Vec<PathBuf>>,
    page: usize,
    entries: Vec<Entry>,
    ui: Option<Screen>,
    returned_title: Value,
}

struct Home {
    load: Value,
    autosave: Value,
    create: Value,
    details: Value,
    default: Value,
}

struct Screen {
    root: Value,
    panel: Value,
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
    back: (Value, Value),
    previous: (Value, Value),
    next: (Value, Value),
    main_menu: (Value, Value),
    achievements: AchievementGrid,
    frame_junctions: [Value; 2],
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

fn member<'a>(node: &'a N, field: &str) -> Option<&'a N> {
    let N::Obj(obj) = node else { return None };
    obj.meta
        .names
        .iter()
        .position(|n| n == field)
        .and_then(|i| obj.vals.get(i))
}

fn meta(save: &str) -> Result<Metadata, String> {
    let outer = nrbf::parse(&b64_dec(save)).map_err(|_| "Unsupported save format.")?;
    let (Some(N::Str(_, inner)), Some(N::Str(_, checksum))) = (
        member(&outer.root, "playerData"),
        member(&outer.root, "checksum"),
    ) else {
        return Err("Missing save data or checksum.".into());
    };
    let bytes = b64_dec(inner);
    if b64_enc(&Md5::digest(&bytes)) != checksum.as_ref() {
        return Err("Save checksum does not match.".into());
    }
    let game = nrbf::parse(&bytes).map_err(|_| "Could not read the saved character.")?;
    if !matches!(&game.root, N::Obj(obj) if obj.meta.name == "PlayerData") {
        return Err("This file does not contain a saved character.".into());
    }
    let resolve = |node: &N| match node {
        N::Ref(i) => game.index.get(i).cloned(),
        other => Some(other.clone()),
    };
    let player = match member(&game.root, "playerName").and_then(resolve) {
        Some(N::Str(_, n)) => clean_label(&n, 34),
        _ => "".into(),
    };
    let time = member(&game.root, "totalPlaytime").and_then(resolve);
    let seconds = match time.as_ref().and_then(|t| member(t, "totalseconds")) {
        Some(N::P(Pv::F64(seconds))) if seconds.is_finite() && *seconds >= 0.0 => Some(*seconds),
        _ => None,
    };
    Ok(Metadata { player, seconds })
}

fn clean_label(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

fn read_save(path: &Path) -> Result<(String, Metadata), String> {
    let mut data = String::new();
    File::open(path)
        .and_then(|f| f.take(MAX_SAVE_BYTES + 1).read_to_string(&mut data))
        .map_err(|e| format!("Could not read the file: {e}"))?;
    if data.len() as u64 > MAX_SAVE_BYTES {
        return Err("The save file is too large.".into());
    }
    let metadata = meta(&data)?;
    Ok((data, metadata))
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_save_file(path, bytes, true)
}

fn write_save_file(path: &Path, bytes: &[u8], replace: bool) -> std::io::Result<()> {
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = dir.join(format!(".save-{}-{stamp}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let written = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    let result = written.and_then(|_| {
        if replace {
            fs::rename(&temp, path)
        } else {
            // Publishing with a link fails atomically if another process filled the slot.
            fs::hard_link(&temp, path)
        }
    });
    if !replace || result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn slot_path(dir: &Path, slot: usize) -> PathBuf {
    if slot == 0 {
        ["NGUSaveSteam.txt", "NGUSave.txt"]
            .into_iter()
            .map(|n| dir.join(n))
            .filter(|p| p.is_file())
            .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
            .unwrap_or_else(|| dir.join("NGUSaveSteam.txt"))
    } else {
        dir.join("slots").join(format!("slot-{slot}.txt"))
    }
}

fn record_autosave_slot(dir: &Path, slot: usize, data: &[u8]) {
    let metadata = format!("{slot}\n{:x}", Md5::digest(data));
    if let Err(error) = atomic_write(&dir.join("autosave-slot.txt"), metadata.as_bytes()) {
        crate::elog!("Could not remember the autosave slot: {error}");
    }
}

pub(crate) fn autosave_written(vm: &Vm, path: &Path, data: &[u8]) {
    let state = &vm.eng.save_slots;
    if let Some(slot) = state.active_slot.filter(|_| state.home.is_some()) {
        let dir = Path::new(&vm.eng.save_dir);
        if path.parent() == Some(dir)
            && matches!(path.file_name().and_then(|name| name.to_str()), Some("NGUSaveSteam.txt" | "NGUSave.txt"))
            && !data.is_empty() {
            record_autosave_slot(dir, slot, data);
        }
    }
}

fn autosave_slot(dir: &Path, path: &Path) -> Option<usize> {
    if fs::metadata(path).ok()?.len() > MAX_SAVE_BYTES { return None; }
    let data = fs::read(path).ok()?;
    if let Ok(info) = fs::read_to_string(dir.join("autosave-slot.txt")) {
        if let Some((slot, digest)) = info.split_once('\n') {
            if let Ok(index @ 0..=SLOT_COUNT) = slot.parse::<usize>() {
                if digest == format!("{:x}", Md5::digest(&data)) {
                    return (index > 0 && slot_path(dir, index).is_file()).then_some(index);
                }
            }
        }
    }
    // Older profiles can be associated only when exactly one slot matches.
    let mut matches = (1..ROW_COUNT).filter(|&index| {
        let slot = slot_path(dir, index);
        fs::metadata(&slot).is_ok_and(|m| m.len() == data.len() as u64)
            && fs::read(slot).is_ok_and(|save| save == data)
    });
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}

fn play_time(seconds: f64) -> String {
    let s = seconds as u64;
    if s >= 86400 {
        format!("{}d {}h {:02}m", s / 86400, s / 3600 % 24, s / 60 % 60)
    } else if s >= 3600 {
        format!("{}h {:02}m", s / 3600, s / 60 % 60)
    } else {
        format!("{}m {:02}s", s / 60, s % 60)
    }
}

fn date(time: SystemTime) -> String {
    let s = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let (year, month, day) = civil_from_days(s / 86400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        s / 3600 % 24,
        s / 60 % 60
    )
}

fn load_entries(dir: &Path) -> Vec<Entry> {
    (0..ROW_COUNT)
        .map(|i| {
            let path = slot_path(dir, i);
            if !path.exists() {
                return Entry::Empty;
            }
            match read_save(&path) {
                Ok((_, info)) => {
                    Entry::Save(info, fs::metadata(&path).and_then(|m| m.modified()).ok())
                }
                Err(_) => Entry::Invalid,
            }
        })
        .collect()
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

fn new_node(vm: &mut Vm, name: &str, parent: &Value, x: [f32; 2], y: f32, height: f32) -> Value {
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

fn image(vm: &mut Vm, go: &Value, sample: Option<&ImageD>, raycast: bool) -> Value {
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
                s.interactable = enabled;
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

fn text(vm: &mut Vm, go: &Value, sample: Option<&TextD>, size: Option<i32>, align: i32) -> Value {
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

fn tr(go: &Value) -> Value {
    with_go(go, |g| g.transform.clone()).unwrap_or_default()
}

fn named_go(vm: &Vm, name: &str) -> Option<Value> {
    vm.eng.gos.iter().find(|go| {
        with_go(go, |g| g.name == name).unwrap_or(false)
    }).cloned()
}

fn child(parent: &Value, name: &str) -> Value {
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

fn style<T>(go: &Value, f: impl Fn(&CK) -> Option<T>) -> Option<T> {
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

fn build(vm: &mut Vm) -> Screen {
    let settings = named_go(vm, "Settings Menu").unwrap_or_default();
    let image_style = |go: &Value| {
        style(go, |k| {
            if let CK::Image(i) = k {
                Some(i.as_ref().clone())
            } else {
                None
            }
        })
    };
    let text_style = |go: &Value| {
        style(go, |k| {
            if let CK::Text(t) = k {
                Some(t.as_ref().clone())
            } else {
                None
            }
        })
    };
    let settings_title = child(&settings, "Title");
    let settings_panel = child(&settings, "Settings Panel 2");
    let settings_back = child(&settings, "Back to Info n Stuff");
    let background = image_style(&settings);
    let section_frame = image_style(&settings_panel);
    let button_frame = image_style(&settings_back);
    let title_style = text_style(&settings_title);
    let subtitle_style = text_style(&child(&settings_title, "subtitle"));
    let section_style = text_style(&child(&settings_panel, "Text"));
    let body_style = text_style(&child(&settings_back, "Text"));
    let button_tint = style(&settings_back, |k| {
        if let CK::Sel(s) = k {
            Some((s.colors, s.transition))
        } else {
            None
        }
    });
    let root = new_node(
        vm,
        "Save Management Canvas",
        &Value::Null,
        [0.0, 1.0],
        0.0,
        600.0,
    );
    attach(
        vm,
        &root,
        "UnityEngine.Canvas",
        CK::Canvas(Box::new(CanvasD {
            mode: 0,
            sorting: 10000,
            override_sorting: true,
            scale_factor: 1.0,
        })),
    );
    vm.eng.canvases.push((10000, tr(&root)));
    vm.eng.canvases.sort_by_key(|(sorting, _)| *sorting);
    let panel = new_node(vm, "Save Management", &tr(&root), [0.0, 0.0], 0.0, 600.0);
    with_tr(&tr(&panel), |t| {
        t.apos[0] = CONTENT_LEFT;
        t.size[0] = vm.eng.desktop_layout.width() - CONTENT_LEFT;
    });
    image(vm, &panel, background.as_ref(), true);
    let title_sidebar = new_node(vm, "Load Game Sidebar", &tr(&root), [0.0, 0.0], 0.0, 600.0);
    with_tr(&tr(&title_sidebar), |t| t.size[0] = CONTENT_LEFT);
    image(vm, &title_sidebar, background.as_ref(), true);
    let main_menu = named_go(vm, "Main Menu Screen").unwrap_or_default();
    let chicken = image_style(&child(&main_menu, "Chicken"));
    if let Some(mut chicken) = chicken {
        chicken.preserve_aspect = true;
        let art = new_node(vm, "Load Game Illustration", &tr(&title_sidebar), [0.15, 0.85], 178.0, 244.0);
        image(vm, &art, Some(&chicken), false);
    }
    let locked_sidebar = new_node(vm, "Locked Sidebar", &tr(&root), [0.0, 0.0], 0.0, 600.0);
    with_tr(&tr(&locked_sidebar), |t| t.size[0] = CONTENT_LEFT);
    let shade = image(vm, &locked_sidebar, None, false);
    with_comp(&shade, |comp| {
        if let CK::Image(image) = &mut comp.k {
            image.color = [0.17, 0.22, 0.28, 0.28];
        }
    });
    let parent = tr(&panel);
    let banner = new_node(vm, "Title", &parent, [0.19, 0.81], 14.0, 58.0);
    let title = text(vm, &banner, title_style.as_ref(), Some(40), 4);
    let intro = new_node(vm, "Subtitle", &parent, [0.06, 0.94], 72.0, 26.0);
    let detail = text(
        vm,
        &intro,
        subtitle_style.as_ref().or(body_style.as_ref()),
        Some(14),
        4,
    );

    let left = new_node(
        vm,
        "Save Slots Panel",
        &parent,
        [0.035, 0.624],
        110.0,
        418.0,
    );
    image(vm, &left, section_frame.as_ref(), false);
    let mut themed = vec![
        (panel.clone(), settings.clone()),
        (title_sidebar.clone(), settings),
        (left.clone(), settings_panel.clone()),
    ];
    let left = tr(&left);
    let right = new_node(
        vm,
        "Selected Save Panel",
        &parent,
        [0.64, 0.965],
        110.0,
        418.0,
    );
    image(vm, &right, section_frame.as_ref(), false);
    themed.push((right.clone(), settings_panel));
    let right = tr(&right);
    let headings = [(&left, "Save Slots"), (&right, "Selected Save")].map(|(p, name)| {
        let heading = new_node(vm, name, p, [0.05, 0.95], 17.0, 34.0);
        text(vm, &heading, section_style.as_ref(), Some(23), 4)
    });
    let summary = new_node(
        vm,
        "Selected Save Details",
        &right,
        [0.08, 0.92],
        61.0,
        133.0,
    );
    let selection = text(vm, &summary, body_style.as_ref(), Some(16), 4);
    let name_panel = new_node(vm, "Character Name", &right, [0.08, 0.92], 155.0, 88.0);
    let name_parent = tr(&name_panel);
    let name_label = new_node(vm, "Name Label", &name_parent, [0.0, 1.0], 0.0, 29.0);
    label(&text(vm, &name_label, body_style.as_ref(), Some(16), 3), "Character Name".into());
    let name_field = new_node(vm, "Character Name Input", &name_parent, [0.0, 1.0], 34.0, 37.0);
    let graphic = image(vm, &name_field, button_frame.as_ref(), true);
    themed.push((name_field.clone(), settings_back.clone()));
    let name_text = new_node(vm, "Text", &tr(&name_field), [0.06, 0.94], 0.0, 37.0);
    let name_placeholder = new_node(vm, "Placeholder", &tr(&name_field), [0.06, 0.94], 0.0, 37.0);
    let placeholder = text(vm, &name_placeholder, body_style.as_ref(), Some(16), 3);
    with_comp(&placeholder, |c| {
        if let CK::Text(text) = &mut c.k { text.color = [0.34, 0.34, 0.34, 1.0]; }
    });
    label(&placeholder, DEFAULT_NAME.into());
    let mut field = default_sel(SelKind::InputField);
    field.target_graphic = graphic;
    field.text_component = text(vm, &name_text, body_style.as_ref(), Some(16), 3);
    field.placeholder = placeholder;
    field.char_limit = NAME_LIMIT as i32;
    let name_input = attach(vm, &name_field, "UnityEngine.UI.InputField", CK::Sel(Box::new(field)));
    let mut actions = Vec::new();
    let mut button = |vm: &mut Vm, p: &Value, action, x, y, h, size| {
        let go = new_node(vm, "Save Screen Button", p, x, y, h);
        let graphic = image(vm, &go, button_frame.as_ref(), true);
        let mut sel = default_sel(SelKind::Button);
        if let Some((colors, transition)) = button_tint {
            sel.colors = colors;
            sel.transition = transition;
        }
        sel.target_graphic = graphic;
        attach(vm, &go, "UnityEngine.UI.Button", CK::Sel(Box::new(sel)));
        themed.push((go.clone(), settings_back.clone()));
        let caption = new_node(vm, "Text", &tr(&go), [0.025, 0.975], 0.0, h);
        let label = text(vm, &caption, body_style.as_ref(), Some(size), 4);
        actions.push((go.clone(), action));
        (go, label)
    };
    let mut rows = Vec::with_capacity(ROW_COUNT);
    let mut row_info = Vec::with_capacity(ROW_COUNT);
    let mut delete_buttons = Vec::with_capacity(SLOT_COUNT);
    let trash = vm.eng.keys.get("sharedassets0.assets:3370").cloned().unwrap_or_default();
    let checkmark = vm.eng.keys.get("sharedassets0.assets:2493").cloned().unwrap_or_default();
    for i in 0..ROW_COUNT {
        let y = 65.0 + i as f32 * 49.0;
        rows.push(button(
            vm,
            &left,
            Action::Row(i),
            [0.066, 0.32],
            y,
            37.0,
            16,
        ));
        let info = new_node(vm, "Slot Details", &left, [0.36, 0.83], y, 37.0);
        row_info.push(text(vm, &info, body_style.as_ref(), Some(16), 3));
        if i > 0 {
            let trash_button = button(vm, &left, Action::Delete(i), [0.86, 0.95], y, 37.0, 12);
            button_icon(vm, &trash_button, &trash);
            delete_buttons.push(trash_button);
        }
    }
    let delete_row = new_node(vm, "Delete Slot Confirmation", &left, [0.066, 0.95], 65.0, 37.0);
    let delete_parent = tr(&delete_row);
    let delete_message = new_node(vm, "Delete Prompt", &delete_parent, [0.0, 0.75], 0.0, 37.0);
    let delete_prompt = text(vm, &delete_message, body_style.as_ref(), Some(13), 3);
    let delete_cancel = button(vm, &delete_parent, Action::CancelDelete, [0.76, 0.87], 0.0, 37.0, 20);
    let delete_confirm = button(vm, &delete_parent, Action::ConfirmDelete, [0.89, 1.0], 0.0, 37.0, 18);
    button_icon(vm, &delete_confirm, &checkmark);
    let message = new_node(
        vm,
        "Save Screen Status",
        &parent,
        [0.035, 0.965],
        540.0,
        39.0,
    );
    let status = text(vm, &message, body_style.as_ref(), Some(16), 4);
    let primary = button(vm, &right, Action::Primary, [0.08, 0.92], 213.0, 41.0, 16);
    let secondary = button(vm, &right, Action::Secondary, [0.08, 0.92], 266.0, 41.0, 16);
    let main_menu = button(vm, &parent, Action::MainMenu, [0.035, 0.168], 18.0, 37.0, 16);
    let back = button(vm, &parent, Action::Back, [0.035, 0.168], 18.0, 37.0, 16);
    let previous = button(vm, &left, Action::Previous, [0.066, 0.35], 366.0, 34.0, 15);
    let next = button(vm, &left, Action::Next, [0.65, 0.934], 366.0, 34.0, 15);
    let grid = new_node(vm, "Achievement Grid", &left, [0.0, 1.0], 53.0, 306.0);
    let tiles = (0..ACHIEVEMENTS_PER_PAGE).map(|i| {
        let x = 0.09 + 0.82 * (i % ACHIEVEMENT_COLUMNS) as f32 / (ACHIEVEMENT_COLUMNS - 1) as f32;
        let y = 18.0 + 67.0 * (i / ACHIEVEMENT_COLUMNS) as f32;
        let tile = button(vm, &tr(&grid), Action::Achievement(i), [x, x], y, 43.0, 9);
        with_tr(&tr(&tile.0), |t| {
            t.apos[0] = -21.5;
            t.size[0] = 43.0;
        });
        for comp in with_go(&tile.0, |g| g.comps.clone()).unwrap_or_default() {
            with_comp(&comp, |c| {
                if let CK::Sel(s) = &mut c.k { s.transition = 0; }
            });
        }
        let icon = icon_in_frame(vm, &tile.0, 43.0);
        (tile, icon)
    }).collect();
    let details = new_node(vm, "Achievement Details", &right, [0.0, 1.0], 56.0, 340.0);
    let details_parent = tr(&details);
    let icon_frame = new_node(vm, "Selected Achievement Frame", &details_parent, [0.5, 0.5], 14.0, 102.0);
    with_tr(&tr(&icon_frame), |t| {
        t.apos[0] = -51.0;
        t.size[0] = 102.0;
    });
    image(vm, &icon_frame, button_frame.as_ref(), false);
    themed.push((icon_frame.clone(), settings_back));
    let icon = icon_in_frame(vm, &icon_frame, 102.0);
    let title_node = new_node(vm, "Selected Achievement Name", &details_parent, [0.09, 0.91], 125.0, 57.0);
    let name = text(vm, &title_node, section_style.as_ref(), Some(20), 4);
    let description_node = new_node(vm, "Condition", &details_parent, [0.09, 0.91], 190.0, 98.0);
    let description = text(vm, &description_node, body_style.as_ref(), Some(16), 4);
    let state_node = new_node(vm, "Unlock State", &details_parent, [0.09, 0.91], 301.0, 30.0);
    let state = text(vm, &state_node, subtitle_style.as_ref(), Some(15), 4);
    let canvas = named_go(vm, "Canvas").unwrap_or_default();
    let frame_junctions = ["Middle Bolt", "Middle Bolt (3)"].map(|name| tr(&child(&canvas, name)));
    Screen {
        root,
        panel,
        title_sidebar,
        locked_sidebar,
        title,
        detail,
        status,
        headings,
        rows,
        row_info,
        delete_buttons,
        delete_row,
        delete_prompt,
        delete_cancel,
        delete_confirm,
        selection,
        name_panel,
        name_input,
        primary,
        secondary,
        back,
        previous,
        next,
        main_menu,
        achievements: AchievementGrid {
            root: grid, tiles, details, icon_frame, icon, name, description, state,
            sprites: RefCell::default(),
        },
        frame_junctions,
        actions,
        themed,
    }
}

fn refresh_theme(ui: &Screen) {
    for (target, source) in &ui.themed {
        copy_theme_sprite(target, source);
    }
}

fn copy_theme_sprite(target: &Value, source: &Value) {
    let Some(sprite) = style(source, |k| match k {
        CK::Image(image) => Some(image.sprite.clone()),
        _ => None,
    }) else { return };
    let changed = with_go(target, |g| {
        let mut changed = false;
        for comp in &g.comps {
            with_comp(comp, |c| {
                if let CK::Image(image) = &mut c.k {
                    if !same(&image.sprite, &sprite) {
                        image.sprite = sprite.clone();
                        changed = true;
                    }
                }
            });
        }
        changed
    });
    if changed == Some(true) {
        mark_dirty(target);
    }
}

fn label(comp: &Value, value: String) {
    with_comp(comp, |c| {
        if let CK::Text(t) = &mut c.k {
            t.text = value;
            t.layout = None;
        }
    });
    mark_dirty(comp);
}

pub(crate) fn prepare_title(vm: &mut Vm) {
    if !vm.eng.desktop_panels || vm.eng.save_dir.is_empty() {
        return;
    }
    let Some(title) = named_go(vm, "Main Menu Screen") else {
        return;
    };
    let button = child(&title, "Load File Button");
    let caption = child(&button, "Button Text");
    for comp in with_go(&caption, |g| g.comps.clone()).unwrap_or_default() {
        if with_comp(&comp, |c| matches!(c.k, CK::Text(_))) == Some(true) {
            label(&comp, "Load Game".into());
        }
    }
    let Some(panel) = named_go(vm, "Player Panel") else { return };
    if !child(&panel, "Port Achievements").is_null() {
        return;
    }
    for name in ["Online Save", "Online Load"] {
        let original = child(&panel, name);
        with_go(&original, |g| g.port_hidden = true);
        mark_dirty(&original);
    }
    let original = child(&panel, "Standalone Save");
    let load = child(&panel, "Standalone Load");
    let background = style(&original, |k| match k {
        CK::Image(i) => Some(i.as_ref().clone()),
        _ => None,
    });
    let typeface = style(&child(&original, "Text"), |k| match k {
        CK::Text(t) => Some(t.as_ref().clone()),
        _ => None,
    });
    let button = new_node(vm, "Port Achievements", &tr(&panel), [0.0, 0.0], 498.0, 29.0);
    let panel_rect = crate::ui::xf(vm, &tr(&panel));
    let save_rect = crate::ui::xf(vm, &tr(&original));
    let load_rect = crate::ui::xf(vm, &tr(&load));
    let panel_left = panel_rect.origin[0] - panel_rect.pivot[0] * panel_rect.w;
    let save_left = save_rect.origin[0] - save_rect.pivot[0] * save_rect.w;
    let load_right = load_rect.origin[0] + (1.0 - load_rect.pivot[0]) * load_rect.w;
    with_tr(&tr(&button), |t| {
        t.apos[0] = save_left - panel_left;
        t.size[0] = load_right - save_left;
    });
    let graphic = image(vm, &button, background.as_ref(), true);
    let mut selectable = default_sel(SelKind::Button);
    selectable.target_graphic = graphic;
    attach(vm, &button, "UnityEngine.UI.Button", CK::Sel(Box::new(selectable)));
    let caption = new_node(vm, "Text", &tr(&button), [0.025, 0.975], 0.0, 29.0);
    label(&text(vm, &caption, typeface.as_ref(), Some(14), 4), "Achievements".into());
    vm.eng.save_slots.sidebar_theme = Some((button, original));
}

pub(crate) fn refresh_achievements(vm: &mut Vm) {
    if vm.eng.save_slots.mode == Some(Mode::Achievements) {
        let state = std::mem::take(&mut vm.eng.save_slots);
        draw_state(vm, &state);
        vm.eng.save_slots = state;
    }
}

fn achievement_sprite(vm: &mut Vm, grid: &AchievementGrid, id: i32, selected: Option<bool>) -> Value {
    let mut sprites = grid.sprites.borrow_mut();
    if let Some(sprite) = sprites.get(&(id, selected)) {
        return sprite.clone();
    }
    let ty = vm.type_id("SteamManager/Achievement");
    let sprite = crate::bcl_core::enum_name(vm, ty, id)
        .map(|name| format!("steam-achievements/{name}"))
        .filter(|key| crate::assets::read(&format!("tex/{key}.png")).is_some())
        .map(|mut key| {
            if let Some(earned) = selected {
                key.push_str(if earned { "#selected" } else { "#selected-locked" });
            }
            let texture = new_comp(vm, "UnityEngine.Texture2D", Value::Null, CK::Texture(Box::new(TexD {
                name: key.clone(), key: key.clone(), w: 64, h: 64,
            })));
            new_comp(vm, "UnityEngine.Sprite", Value::Null, CK::Sprite(Box::new(SpriteD {
                name: key, tex: texture, rect: [0.0, 0.0, 64.0, 64.0],
                border: [0.0; 4], pivot: [0.5, 0.5], ppu: 100.0,
            })))
        }).unwrap_or_default();
    sprites.insert((id, selected), sprite.clone());
    sprite
}

fn paint_achievement(comp: &Value, sprite: Value, earned: bool) {
    let brightness = if earned { 1.0 } else { 0.45 };
    let alpha = if sprite.is_null() { 0.0 } else { 1.0 };
    with_comp(comp, |c| {
        if let CK::Image(image) = &mut c.k {
            image.sprite = sprite;
            image.color = [brightness, brightness, brightness, alpha];
        }
    });
    mark_dirty(comp);
}

fn draw_achievements(vm: &mut Vm, state: &SaveSlots, ui: &Screen) {
    let grid = &ui.achievements;
    let start = state.page * ACHIEVEMENTS_PER_PAGE;
    let count = vm.eng.achievements.entries.len();
    let earned = vm.eng.achievements.entries.iter().filter(|e| e.earned).count();
    let pages = count.div_ceil(ACHIEVEMENTS_PER_PAGE).max(1);
    label(&ui.title, "ACHIEVEMENTS".into());
    label(&ui.detail, format!("{earned} of {count} unlocked"));
    label(&ui.status, if vm.eng.achievements.write_failed {
        "Could not save achievements. The game will try again.".into()
    } else { format!("Page {} of {pages}", state.page + 1) });
    for (row, info) in ui.rows.iter().zip(&ui.row_info) {
        vm.set_active(&row.0, false);
        vm.set_active(&go_of(info), false);
    }
    for (button, _) in &ui.delete_buttons { vm.set_active(button, false); }
    vm.set_active(&ui.delete_row, false);
    for (i, (tile, icon)) in grid.tiles.iter().enumerate() {
        let entry = vm.eng.achievements.entries.get(start + i).map(|e| (e.id, e.name.clone(), e.earned));
        vm.set_active(&tile.0, entry.is_some());
        if let Some((id, name, earned)) = entry {
            let selected = state.selected == start + i;
            let sprite = achievement_sprite(vm, grid, id, selected.then_some(earned));
            let has_icon = !sprite.is_null();
            caption(tile, &name, !has_icon && selected);
            // Retain an input label for focus and scripted navigation.
            with_comp(&tile.1, |comp| {
                if let CK::Text(text) = &mut comp.k {
                    text.color[3] = if has_icon { 0.0 } else { 1.0 };
                }
            });
            paint_achievement(icon, sprite, earned || selected);
        }
    }
    let selected = vm.eng.achievements.entries.get(state.selected)
        .map(|e| (e.id, e.name.clone(), e.description.clone(), e.earned));
    if let Some((id, name, description, earned)) = selected {
        let icon = achievement_sprite(vm, grid, id, None);
        vm.set_active(&grid.icon_frame, !icon.is_null());
        paint_achievement(&grid.icon, icon, earned);
        label(&grid.name, name);
        label(&grid.description, description);
        label(&grid.state, if earned { "Unlocked" } else { "Locked" }.into());
    } else {
        vm.set_active(&grid.icon_frame, false);
        label(&grid.name, "Choose an achievement".into());
        label(&grid.description, String::new());
        label(&grid.state, String::new());
    }
    for button in [&ui.primary, &ui.secondary, &ui.main_menu] {
        vm.set_active(&button.0, false);
    }
    vm.set_active(&ui.previous.0, state.page > 0);
    vm.set_active(&ui.next.0, state.page + 1 < pages);
    caption(&ui.previous, "Previous", false);
    caption(&ui.next, "Next", false);
}

fn caption(button: &(Value, Value), value: &str, selected: bool) {
    label(&button.1, value.into());
    for c in with_go(&button.0, |g| g.comps.clone()).unwrap_or_default() {
        with_comp(&c, |c| {
            if let CK::Image(image) = &mut c.k {
                image.color = if selected {
                    SELECTION_COLOR
                } else {
                    [1.0; 4]
                };
            }
        });
    }
    mark_dirty(&button.0);
}

fn draw_state(vm: &mut Vm, state: &SaveSlots) {
    let (Some(ui), Some(mode)) = (&state.ui, state.mode) else {
        return;
    };
    let choosing = state.from_title && state.home.is_some();
    let creating = mode == Mode::Create;
    let achievements = mode == Mode::Achievements;
    vm.set_active(&ui.locked_sidebar, choosing);
    let importing = state.import_files.is_some();
    let cancelling = state.pending.is_some() || importing;
    vm.set_active(&ui.back.0, !achievements && (state.from_title || cancelling));
    vm.set_active(&ui.name_panel, creating && state.selected < ROW_COUNT);
    vm.set_active(&ui.achievements.root, achievements);
    vm.set_active(&ui.achievements.details, achievements);
    vm.set_active(&go_of(&ui.selection), !achievements);
    if achievements {
        vm.set_active(&ui.title_sidebar, false);
        draw_achievements(vm, state, ui);
        return;
    }
    vm.set_active(&ui.title_sidebar, state.from_title && !choosing);
    label(
        &ui.title,
        if importing {
            "IMPORT A SAVE"
        } else if creating {
            "CREATE NEW GAME"
        } else if mode == Mode::Save {
            "SAVE GAME"
        } else {
            "LOAD GAME"
        }
        .into(),
    );
    label(
        &ui.detail,
        if importing {
            "Place .txt saves in save/import, then choose a file below."
        } else if creating {
            "Select an empty slot for your new game."
        } else if mode == Mode::Save {
            "Choose a slot for a snapshot of your current game."
        } else if state.from_title {
            "Choose a saved game to continue."
        } else {
            "Choose a saved game to continue. Your current game is backed up before loading."
        }
        .into(),
    );
    label(&ui.status, state.status.clone());
    let row_y = |i: usize| 65.0 + i as f32 * 49.0 - if choosing { 24.5 } else { 0.0 };
    vm.set_active(&ui.delete_row, state.deleting.is_some());
    if let Some(delete) = &state.deleting {
        with_tr(&tr(&ui.delete_row), |t| t.apos[1] = -row_y(delete.index));
        mark_dirty(&ui.delete_row);
        label(&ui.delete_prompt, if delete.final_confirmation {
            format!("Delete Slot {} for good? The trash can has no undo.", delete.index)
        } else {
            format!("Delete the save in Slot {}?", delete.index)
        });
        let (left, right) = if delete.final_confirmation {
            (&ui.delete_confirm, &ui.delete_cancel)
        } else {
            (&ui.delete_cancel, &ui.delete_confirm)
        };
        for (button, anchors) in [(left, [0.76, 0.87]), (right, [0.89, 1.0])] {
            with_tr(&tr(&button.0), |t| { t.amin[0] = anchors[0]; t.amax[0] = anchors[1]; });
            mark_dirty(&button.0);
        }
        caption(&ui.delete_cancel, "×", false);
        caption(&ui.delete_confirm, "✓", false);
    }
    for (i, row) in ui.rows.iter().enumerate() {
        let confirming = state.deleting.as_ref().is_some_and(|delete| delete.index == i);
        let visible = (!choosing || i > 0) && !confirming;
        let available = !choosing || slot_available(mode, state.entries.get(i));
        set_interactable(&row.0, available);
        vm.set_active(&go_of(&ui.row_info[i]), visible);
        if let Some(button) = i.checked_sub(1).and_then(|i| ui.delete_buttons.get(i)) {
            let occupied = matches!(state.entries.get(i), Some(Entry::Save(..) | Entry::Invalid));
            vm.set_active(&button.0, creating && occupied && !confirming);
            with_tr(&tr(&button.0), |t| t.apos[1] = -row_y(i));
            caption(button, &format!("Delete Slot {i}"), false);
        }
        if let Some(files) = &state.import_files {
            let file = files.get(state.page * ROW_COUNT + i);
            vm.set_active(&row.0, file.is_some());
            caption(
                row,
                &format!("File {}", state.page * ROW_COUNT + i + 1),
                false,
            );
            label(
                &ui.row_info[i],
                file.map_or(String::new(), |f| {
                    clean_label(&f.file_name().unwrap_or_default().to_string_lossy(), 40)
                }),
            );
            continue;
        }
        for node in [&row.0, &go_of(&ui.row_info[i])] {
            with_tr(&tr(node), |t| {
                t.apos[1] = -row_y(i);
            });
            mark_dirty(node);
        }
        vm.set_active(&row.0, visible);
        let name = if i == 0 {
            "Autosave".into()
        } else {
            format!("Slot {i}")
        };
        let contents = match state.entries.get(i).unwrap_or(&Entry::Empty) {
            Entry::Empty => "Empty".into(),
            Entry::Invalid => "Unreadable save".into(),
            Entry::Save(info, _) => {
                let time = info
                    .seconds
                    .map(play_time)
                    .unwrap_or_else(|| "Unknown play time".into());
                if info.player.is_empty() {
                    time
                } else {
                    format!("{}   |   {time}", clean_label(&info.player, 20))
                }
            }
        };
        caption(row, &name, state.selected == i);
        label(&ui.row_info[i], contents);
    }
    let slot_name = if state.selected == 0 {
        "Autosave".into()
    } else {
        format!("Slot {}", state.selected)
    };
    let selection = match state.entries.get(state.selected) {
        Some(Entry::Save(info, modified)) => {
            let player = if info.player.is_empty() {
                "Unknown character"
            } else {
                &info.player
            };
            let played = info
                .seconds
                .map(play_time)
                .unwrap_or_else(|| "Unknown".into());
            let saved = modified.map(date).unwrap_or_else(|| "Unknown".into());
            format!("{slot_name}\n\n{player}\nPlayed: {played}\n\nLast saved\n{saved}")
        }
        Some(Entry::Invalid) => format!("{slot_name}\n\nUnreadable save"),
        None if choosing => "No slot selected".into(),
        None => "Choose a save".into(),
        _ if creating => format!("{slot_name} — Empty"),
        _ => format!("{slot_name}\n\nEmpty"),
    };
    label(&ui.selection, selection);
    vm.set_active(&ui.primary.0, (!importing || state.pending.is_some()) && state.deleting.is_none());
    with_tr(&tr(&ui.primary.0), |t| t.apos[1] = if creating { -251.0 } else { -213.0 });
    set_interactable(&ui.primary.0, !choosing || slot_available(mode, state.entries.get(state.selected)));
    vm.set_active(&ui.secondary.0, !importing && !choosing);
    vm.set_active(&ui.main_menu.0, !state.from_title && !cancelling);
    caption(&ui.main_menu, "Main Menu", false);
    let pages = state
        .import_files
        .as_ref()
        .map_or(0, |files| files.len().div_ceil(ROW_COUNT));
    vm.set_active(
        &ui.previous.0,
        importing && state.pending.is_none() && state.page > 0,
    );
    vm.set_active(
        &ui.next.0,
        importing && state.pending.is_none() && state.page + 1 < pages,
    );
    caption(&ui.previous, "Previous", false);
    caption(&ui.next, "Next", false);
    caption(
        &ui.primary,
        if state.pending.is_some() {
            "Confirm"
        } else if creating {
            "Create New Game"
        } else if mode == Mode::Save {
            "Save Current Game"
        } else {
            "Load Selected Game"
        },
        false,
    );
    caption(
        &ui.secondary,
        if mode == Mode::Save {
            "Import Save"
        } else {
            "Export Save"
        },
        false,
    );
    caption(
        &ui.back,
        if cancelling {
            "Cancel"
        } else {
            "Main Menu"
        },
        false,
    );
}

pub(crate) fn owns(go: &Value) -> bool {
    with_go(go, |g| g.tag == "PortSaveScreen").unwrap_or(false)
}

pub(crate) fn is_open(vm: &Vm) -> bool {
    vm.eng.save_slots.mode.is_some()
}

pub(crate) fn frame_junctions(vm: &Vm) -> Option<[Value; 2]> {
    vm.eng.save_slots.ui.as_ref().map(|ui| ui.frame_junctions.clone())
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
    let title = game_component(vm, "MainMenuController").ok_or("The main menu is unavailable.")?;
    game_call(vm, &title, "startMainMenu", vec![])?;
    restore_idle_stats(vm, state);
    close(vm, state);
    state.returned_title = Value::Null;
    state.home_active = true;
    let home = state.home.as_mut().ok_or("The home screen is unavailable.")?;
    let entries = load_entries(dir);
    let autosave_available = matches!(entries.first(), Some(Entry::Save(..)));
    let numbered_available = entries[1..].iter().any(|entry| matches!(entry, Entry::Save(..)));
    let description = match entries.first() {
        Some(Entry::Save(info, _)) => {
            let name = if info.player.is_empty() { "Unknown character" } else { &info.player };
            let played = info.seconds.map(play_time).unwrap_or_else(|| "Unknown".into());
            format!("<b>{name}</b>\nPlayed: {played}")
        }
        Some(Entry::Invalid) => "Autosave unreadable".into(),
        _ => "No autosave yet".into(),
    };
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
                invoke.method != "initialStart" || !same(&invoke.owner, &character)
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
    if let Some((target, source)) = &vm.eng.save_slots.sidebar_theme {
        copy_theme_sprite(target, source);
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
    state.mode = None;
    state.from_title = false;
    state.pending = None;
    state.deleting = None;
    state.import_files = None;
    if let Some(ui) = &state.ui {
        if vm.eng.text_req.as_ref().is_some_and(|input| same(&input.comp, &ui.name_input)) {
            vm.eng.text_req = None;
        }
        vm.set_active(&ui.root, false);
    }
}

fn open(vm: &mut Vm, state: &mut SaveSlots, dir: &Path, mode: Mode, from_title: bool) {
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
    label(&ui.headings[0], if achievements { "Steam Milestones" } else { "Save Slots" }.into());
    label(&ui.headings[1], if achievements { "Details" } else { "Selected Save" }.into());
    refresh_theme(ui);
    vm.set_active(&ui.root, true);
}

fn perform(
    vm: &mut Vm,
    state: &mut SaveSlots,
    dir: &Path,
    action: Pending,
) -> Result<String, String> {
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
            Ok(format!("Saved to Slot {index}."))
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
            state.active_slot = if index == 0 { autosave_slot(dir, &path) } else { Some(index) };
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
            Ok("Game loaded. The previous game is in save/before-load.txt.".into())
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
            if state.from_title && state.home.is_some() && (index == 0 || !slot_available(mode, state.entries.get(index))) {
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
                    state.status = "Load this save and replace the current game? The current game will be backed up.".into();
                }
            } else {
                state.status = "Choose a valid saved game.".into();
            }
        }
        Action::Secondary => {
            state.pending = None;
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
        "Port Achievements" => Some((Mode::Achievements, false)),
        _ => None,
    })
    .flatten();
    if mode.is_none() && vm.eng.save_slots.mode.is_none() {
        return false;
    }
    let mut state = std::mem::take(&mut vm.eng.save_slots);
    let dir = PathBuf::from(&vm.eng.save_dir);
    let handled = if let Some((mode, from_title)) = mode {
        open(vm, &mut state, &dir, mode, from_title);
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
        if x < CONTENT_LEFT && vm.has_handler(go, crate::input::Ev::Click) {
            close(vm, &mut state);
        }
        false
    };
    draw_state(vm, &state);
    vm.eng.save_slots = state;
    handled
}
