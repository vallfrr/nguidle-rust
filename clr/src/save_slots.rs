//! Local save screens. The game still owns serialization and progression.
use crate::bcl_core::{b64_dec, b64_enc, civil_from_days};
use crate::nrbf::{self, Pv, N};
use crate::unity::*;
use crate::value::{Ext, Obj, Value};
use crate::Vm;
use md5::{Digest, Md5};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SLOT_COUNT: usize = 5;
const ROW_COUNT: usize = SLOT_COUNT + 1;
const NO_SLOT: usize = ROW_COUNT;
pub(crate) const CONTENT_LEFT: f32 = 292.0;
const MAX_SAVE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Save,
    Load,
    Achievements,
}

#[derive(Clone)]
enum Pending {
    Save(usize),
    Load(usize),
    Import(PathBuf, usize),
    MainMenu,
}

#[derive(Clone, Copy)]
enum Action {
    Row(usize),
    Primary,
    Secondary,
    Back,
    Previous,
    Next,
    MainMenu,
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

struct Screen {
    root: Value,
    panel: Value,
    title_sidebar: Value,
    title: Value,
    detail: Value,
    status: Value,
    headings: [Value; 2],
    rows: Vec<(Value, Value)>,
    row_info: Vec<Value>,
    selection: Value,
    primary: (Value, Value),
    secondary: (Value, Value),
    back: (Value, Value),
    previous: (Value, Value),
    next: (Value, Value),
    main_menu: (Value, Value),
    actions: Vec<(Value, Action)>,
    themed: Vec<(Value, Value)>,
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
    let result = written.and_then(|_| fs::rename(&temp, path));
    if result.is_err() {
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
    let c = game_component(vm, "ImportExport").ok_or("The save controller is unavailable.")?;
    let Value::Str(data) = game_call(vm, &c, "getBase64Data", vec![])? else {
        return Err("The game produced no save data.".into());
    };
    meta(&data)?;
    atomic_write(path, data.as_bytes()).map_err(|e| format!("Could not write the save: {e}"))
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
        let info = new_node(vm, "Slot Details", &left, [0.36, 0.94], y, 37.0);
        row_info.push(text(vm, &info, body_style.as_ref(), Some(16), 3));
    }
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
    let main_menu = button(vm, &right, Action::MainMenu, [0.08, 0.92], 347.0, 40.0, 16);
    let back = button(vm, &parent, Action::Back, [0.035, 0.168], 18.0, 37.0, 16);
    let previous = button(vm, &left, Action::Previous, [0.066, 0.35], 366.0, 34.0, 15);
    let next = button(vm, &left, Action::Next, [0.65, 0.934], 366.0, 34.0, 15);
    Screen {
        root,
        panel,
        title_sidebar,
        title,
        detail,
        status,
        headings,
        rows,
        row_info,
        selection,
        primary,
        secondary,
        back,
        previous,
        next,
        main_menu,
        actions,
        themed,
    }
}

fn refresh_theme(ui: &Screen) {
    for (target, source) in &ui.themed {
        let Some(sprite) = style(source, |k| match k {
            CK::Image(image) => Some(image.sprite.clone()),
            _ => None,
        }) else {
            continue;
        };
        for comp in with_go(target, |g| g.comps.clone()).unwrap_or_default() {
            let changed = with_comp(&comp, |c| match &mut c.k {
                CK::Image(image) if !same(&image.sprite, &sprite) => {
                    image.sprite = sprite.clone();
                    true
                }
                _ => false,
            });
            if changed == Some(true) {
                mark_dirty(&comp);
            }
        }
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
    let background = style(&original, |k| match k {
        CK::Image(i) => Some(i.as_ref().clone()),
        _ => None,
    });
    let typeface = style(&child(&original, "Text"), |k| match k {
        CK::Text(t) => Some(t.as_ref().clone()),
        _ => None,
    });
    let button = new_node(vm, "Port Achievements", &tr(&panel), [0.06, 0.94], 498.0, 29.0);
    let graphic = image(vm, &button, background.as_ref(), true);
    let mut selectable = default_sel(SelKind::Button);
    selectable.target_graphic = graphic;
    attach(vm, &button, "UnityEngine.UI.Button", CK::Sel(Box::new(selectable)));
    let caption = new_node(vm, "Text", &tr(&button), [0.025, 0.975], 0.0, 29.0);
    label(&text(vm, &caption, typeface.as_ref(), Some(14), 4), "Achievements".into());
}

pub(crate) fn refresh_achievements(vm: &mut Vm) {
    if vm.eng.save_slots.mode == Some(Mode::Achievements) {
        let state = std::mem::take(&mut vm.eng.save_slots);
        draw_state(vm, &state);
        vm.eng.save_slots = state;
    }
}

fn draw_achievements(vm: &mut Vm, state: &SaveSlots, ui: &Screen) {
    let start = state.page * ROW_COUNT;
    let entries = &vm.eng.achievements.entries;
    let count = entries.len();
    let earned = entries.iter().filter(|e| e.earned).count();
    let pages = count.div_ceil(ROW_COUNT).max(1);
    label(&ui.title, "ACHIEVEMENTS".into());
    label(&ui.detail, format!("{earned} of {count} unlocked"));
    label(&ui.status, if vm.eng.achievements.write_failed {
        "Could not save achievements. The game will try again.".into()
    } else { format!("Page {} of {pages}", state.page + 1) });
    for (i, row) in ui.rows.iter().enumerate() {
        let entry = vm.eng.achievements.entries.get(start + i).map(|e| (e.name.clone(), e.earned));
        vm.set_active(&row.0, entry.is_some());
        let (name, status) = entry.as_ref().map_or(("", ""), |(name, earned)| {
            (name.as_str(), if *earned { "Unlocked" } else { "Locked" })
        });
        caption(row, name, state.selected == start + i);
        label(&ui.row_info[i], status.into());
    }
    let selected = vm.eng.achievements.entries.get(state.selected);
    label(&ui.selection, selected.map_or_else(|| "Choose an achievement".into(), |e| {
        format!("{}\n\n{}", e.name, e.description)
    }));
    for button in [&ui.primary, &ui.secondary, &ui.main_menu] {
        vm.set_active(&button.0, false);
    }
    vm.set_active(&ui.previous.0, state.page > 0);
    vm.set_active(&ui.next.0, state.page + 1 < pages);
    caption(&ui.previous, "Previous", false);
    caption(&ui.next, "Next", false);
    caption(&ui.back, "Back", false);
}

fn caption(button: &(Value, Value), value: &str, selected: bool) {
    label(&button.1, value.into());
    for c in with_go(&button.0, |g| g.comps.clone()).unwrap_or_default() {
        with_comp(&c, |c| {
            if let CK::Image(image) = &mut c.k {
                image.color = if selected {
                    [0.93, 0.93, 0.68, 1.0]
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
    if mode == Mode::Achievements {
        vm.set_active(&ui.title_sidebar, false);
        draw_achievements(vm, state, ui);
        return;
    }
    let importing = state.import_files.is_some();
    vm.set_active(&ui.title_sidebar, state.from_title);
    label(
        &ui.title,
        if importing {
            "IMPORT A SAVE"
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
    for (i, row) in ui.rows.iter().enumerate() {
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
        vm.set_active(&row.0, true);
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
        None => "Choose a save".into(),
        _ => format!("{slot_name}\n\nEmpty"),
    };
    label(&ui.selection, selection);
    vm.set_active(&ui.primary.0, !importing || state.pending.is_some());
    vm.set_active(&ui.secondary.0, !importing);
    vm.set_active(&ui.main_menu.0, !state.from_title && !importing && state.pending.is_none());
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
        if state.pending.is_some() {
            "Cancel"
        } else {
            "Back"
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

pub(crate) fn pause_on_title(vm: &mut Vm) -> bool {
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
    state.import_files = None;
    if let Some(ui) = &state.ui {
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
        Mode::Achievements => usize::MAX,
        Mode::Save => (1..ROW_COUNT)
            .find(|&i| matches!(state.entries[i], Entry::Empty))
            .unwrap_or(1),
        Mode::Load if from_title => NO_SLOT,
        Mode::Load => state
            .entries
            .iter()
            .position(|e| matches!(e, Entry::Save(..)))
            .unwrap_or(0),
    };
    state.pending = None;
    state.page = 0;
    state.import_files = None;
    state.status.clear();
    let ui = state.ui.as_ref().unwrap();
    let achievements = mode == Mode::Achievements;
    label(&ui.headings[0], if achievements { "Steam Milestones" } else { "Save Slots" }.into());
    label(&ui.headings[1], if achievements { "Details" } else { "Selected Save" }.into());
    for ((row, _), info) in ui.rows.iter().zip(&ui.row_info) {
        with_tr(&tr(row), |t| t.amax[0] = if achievements { 0.65 } else { 0.32 });
        with_tr(&transform_of(info), |t| t.amin[0] = if achievements { 0.70 } else { 0.36 });
        mark_dirty(row);
        mark_dirty(info);
    }
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
        Pending::Save(index) => {
            write_current(vm, &slot_path(dir, index))?;
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
            apply_save(vm, dir, &slot_path(dir, index), state.from_title)?;
            if state.from_title {
                let controller = game_component(vm, "MainMenuController")
                    .ok_or("The main menu is unavailable.")?;
                game_call(vm, &controller, "finishMainMenu", vec![])?;
                state.returned_title = Value::Null;
            }
            if let Err(error) = write_current(vm, &slot_path(dir, 0)) {
                return Ok(format!(
                    "Game loaded. Autosave could not be updated: {error}"
                ));
            }
            close(vm, state);
            Ok("Game loaded. The previous game is in save/before-load.txt.".into())
        }
        Pending::MainMenu => {
            state.returned_title = return_to_title(vm, dir)?;
            close(vm, state);
            Ok(String::new())
        }
    }
}

fn handle(vm: &mut Vm, state: &mut SaveSlots, dir: &Path, action: Action) {
    let Some(mode) = state.mode else { return };
    if mode == Mode::Achievements {
        match action {
            Action::Back => close(vm, state),
            Action::Row(i) => state.selected = state.page * ROW_COUNT + i,
            Action::Previous => state.page = state.page.saturating_sub(1),
            Action::Next if (state.page + 1) * ROW_COUNT < vm.eng.achievements.entries.len() => {
                state.page += 1;
            }
            _ => {}
        }
        return;
    }
    match action {
        Action::Back => {
            if state.pending.take().is_some() {
                state.status.clear();
            } else if state.import_files.take().is_some() {
                state.status.clear();
            } else {
                close(vm, state);
            }
        }
        Action::Row(index) => {
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
            state.status = "Save the current game and return to the main menu?".into();
        }
        Action::Primary => {
            if let Some(pending) = state.pending.take() {
                state.status = perform(vm, state, dir, pending).unwrap_or_else(|e| e);
                state.entries = load_entries(dir);
            } else if mode == Mode::Save {
                if state.selected == 0 {
                    state.status = "The game manages Autosave. Choose a manual slot.".into();
                } else if !slot_path(dir, state.selected).exists() {
                    state.status = perform(vm, state, dir, Pending::Save(state.selected))
                        .unwrap_or_else(|e| e);
                    state.entries = load_entries(dir);
                } else {
                    state.pending = Some(Pending::Save(state.selected));
                    state.status = format!("Replace the existing save in Slot {}?", state.selected);
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
