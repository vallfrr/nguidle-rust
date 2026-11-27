//! Minimal Unity engine: GameObject/Transform/UI components, scene loading, life cycle.
mod scene;
mod schedule;

use crate::program::Ty;
use crate::value::*;
use crate::Vm;
use serde_json::Value as J;
use std::collections::HashMap;
use std::rc::Rc;

pub type Col = [f32; 4];

pub struct Tr {
    pub parent: Value,
    pub children: Vec<Value>,
    pub pos: [f32; 3],
    pub scale: [f32; 3],
    pub rot_z: f32,
    pub amin: [f32; 2],
    pub amax: [f32; 2],
    pub pivot: [f32; 2],
    pub apos: [f32; 2],
    pub size: [f32; 2],
    pub is_rect: bool,
}

#[derive(Clone)]
pub struct ImageD {
    pub sprite: Value,
    pub color: Col,
    pub typ: i32,
    pub fill_center: bool,
    pub fill_method: i32,
    pub fill_amount: f32,
    pub fill_origin: i32,
    pub clockwise: bool,
    pub preserve_aspect: bool,
    pub raycast: bool,
}

#[derive(Clone)]
pub struct TextD {
    pub text: String,
    pub font: Value,
    pub size: i32,
    pub style: i32,
    pub best_fit: bool,
    pub min_size: i32,
    pub max_size: i32,
    pub align: i32,
    pub rich: bool,
    pub hov: i32,
    pub vov: i32,
    pub line_spacing: f32,
    pub color: Col,
    pub raycast: bool,
    pub layout: Option<(u64, Rc<crate::render::TextLayout>)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SelKind {
    Button,
    Slider,
    Toggle,
    InputField,
    Dropdown,
    Scrollbar,
}

pub struct SelD {
    pub kind: SelKind,
    pub interactable: bool,
    pub transition: i32,
    pub colors: [Col; 5],
    pub target_graphic: Value,
    pub on_click: Value,
    pub on_change: Value,
    pub on_end: Value,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub whole: bool,
    pub direction: i32,
    pub fill_rect: Value,
    pub handle_rect: Value,
    pub is_on: bool,
    pub toggle_graphic: Value,
    pub text: String,
    pub text_component: Value,
    pub placeholder: Value,
    pub char_limit: i32,
    pub content_type: i32,
    pub size: f32,
    /// Dropdown options: `UnityEngine.UI.Dropdown/OptionData` objects (native `OptD`), shared with the game code
    pub options: Vec<Value>,
    /// Dropdown caption Text component
    pub caption: Value,
    pub focused: bool,
    pub state: i32, // 0 normal 1 highlighted 2 pressed
}

pub struct CanvasD {
    pub mode: i32,
    pub sorting: i32,
    pub override_sorting: bool,
    pub scale_factor: f32,
}

pub struct PCall {
    pub target: Value,
    pub method: String,
    pub mode: i32,
    pub arg_obj: Value,
    pub arg_i: i32,
    pub arg_f: f32,
    pub arg_s: String,
    pub arg_b: bool,
}

#[derive(Default)]
pub struct UEvent {
    pub persistent: Vec<PCall>,
    pub runtime: Vec<Deleg>,
}

pub enum CK {
    User,
    Other(String),
    Transform(Box<Tr>),
    Canvas(Box<CanvasD>),
    Image(Box<ImageD>),
    Text(Box<TextD>),
    RawImage(Box<ImageD>),
    Sel(Box<SelD>),
    EventTrigger(Vec<(i32, Value)>),
    ScrollRect { content: Value, viewport: Value, hscroll: Value, vscroll: Value, horizontal: bool, vertical: bool, norm: [f32; 2], sens: f32 },
    Mask { show_graphic: bool },
    Layout { kind: String, d: HashMap<String, f32> },
    Outline { color: Col, dist: [f32; 2] },
    CanvasScaler { mode: i32, scale: f32, reference: [f32; 2], match_: f32 },
    Event(UEvent),
    Sprite(Box<SpriteD>),
    Texture(Box<TexD>),
    Font { name: String, file: String },
    TextAsset { name: String, key: String, text: Option<Rc<str>> },
    AudioClip(String),
    Wait(f32),
    Coroutine,
}

pub struct SpriteD {
    pub name: String,
    pub tex: Value,
    pub rect: [f32; 4],
    pub border: [f32; 4],
    pub pivot: [f32; 2],
    pub ppu: f32,
}

pub struct TexD {
    pub name: String,
    pub key: String,
    pub w: i32,
    pub h: i32,
}

pub struct Comp {
    pub go: Value,
    pub enabled: bool,
    pub awoken: bool,
    pub started: bool,
    pub k: CK,
}

pub struct GoData {
    pub port_hidden: bool,
    pub name: String,
    pub active: bool,
    pub layer: i32,
    pub tag: String,
    pub comps: Vec<Value>,
    pub transform: Value,
    pub destroyed: bool,
    /// render cache of the subtree (see render.rs)
    pub dirty: bool,
    pub cache: Option<Box<crate::render::NodeCache>>,
    /// inside a subtree managed by a layout (layout invalidation)
    pub in_layout: bool,
    /// cached `active_in_hierarchy`: (epoch it was computed in, result)
    pub hier: (u32, bool),
}

/// Bumped whenever an activity flag, a destruction or a parent changes: invalidates every cached `active_in_hierarchy`.
static HIER_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
pub fn bump_hierarchy() {
    HIER_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub enum Wait {
    Frame,
    Until(f64),
    Done,
}

pub struct Co {
    pub owner: Value,
    pub stack: Vec<Value>,
    pub wait: Wait,
    pub dead: bool,
}

pub struct Inv {
    pub owner: Value,
    pub method: Rc<str>,
    method_id: Option<u32>,
    pub next: f64,
    pub repeat: f64,
}

/// Update components still to run in the step in progress.
pub struct TickSlice {
    ups: Vec<Value>,
    idx: usize,
    /// the visual-motion clock of the step, parked while the frames in between are drawn
    clock: Option<(f64, f64)>,
}

#[derive(Default)]
pub struct Engine {
    pub(crate) dropdown: crate::dropdown::Dropdown,
    pub(crate) save_slots: crate::save_slots::SaveSlots,
    pub desktop_panels: bool,
    /// Lets the save screens show an "Open saves folder" button. Set by hosts that have a file manager.
    pub open_folder: Option<fn(&str)>,
    /// Game exceptions reported since the start (the full-game script checks that it stays at zero).
    pub exceptions: u32,
    /// Wall clock at the previous step (seconds since 1970), see `resume`.
    pub wall_seen: Option<f64>,
    /// Real time the host could not play and that is waiting to be applied as offline progress, see `resume`.
    pub away_pending: f64,
    /// Do not measure the time the host was away (tools that move the clock by hand).
    pub resume_off: bool,
    /// How many times time the host was away has been applied.
    pub resumed: u32,
    pub(crate) achievements: crate::achievements::Book,
    pub(crate) motion: crate::motion::Motion,
    pub keys: HashMap<String, Value>,
    pub(crate) resources: HashMap<String, Vec<Value>>,
    pub gos: Vec<Value>,
    pub(crate) canvases: Vec<(i32, Value)>,
    pub time: f64,
    pub dt: f32,
    pub frame: u64,
    pub coroutines: Vec<Co>,
    pub invokes: Vec<Inv>,
    pub screen: (i32, i32),
    pub desktop_layout: crate::desktop_layout::DesktopLayout,
    pub prefs: HashMap<String, Value>,
    pub assets_dir: String,
    pub save_dir: String,
    pub(crate) autosave_writer: Option<crate::host_platform::BackgroundAutosave>,
    pub mouse: (f32, f32),
    pub mouse_down: bool,
    pub mouse_down_edge: bool,
    pub mouse_up_edge: bool,
    /// button the `mouse_down*` flags describe: 0 left, 1 right, 2 middle (one button at a time)
    pub mouse_button: i32,
    /// mouse wheel notches since the last frame (positive = up)
    pub wheel: f32,
    pub keys_down: std::collections::HashSet<i32>,
    pub keys_edge: std::collections::HashSet<i32>,
    pub rng: [u32; 4],
    /// focused element (gamepad/keyboard navigation) and its rectangle (virtual px, top-left) for drawing
    pub focus: Value,
    pub focus_rect: Option<[f32; 4]>,
    pub(crate) focus_cache: Option<crate::focus::ControlCache>,
    pub(crate) console_switch: Option<crate::console_mode::Switch>,
    /// console presentation state (hold, pan), present while the console mode is on
    pub console: Option<Box<crate::console_mode::Console>>,
    pub(crate) numpad: Option<Box<crate::numpad::NumPad>>,
    pub(crate) glyph_sprites: HashMap<String, Value>,
    /// the console mode's free cursor, in rows from the top left (moved with the right stick); `None` when hidden
    pub console_cursor: Option<(f32, f32)>,
    pub(crate) cursor_node: Option<Value>,
    pub(crate) cache_focus_controls: bool,
    pub(crate) parked_menu_canvases: crate::vm::FxMap<usize, Value>,
    /// last `Screen.SetResolution(w, h, fullscreen)` request and the frame it was made at
    /// clip name per AudioSource object (by address) and the sounds to play (drained by the shell)
    pub audio_clips: HashMap<usize, String>,
    pub sound_queue: Vec<String>,
    pub resolution_req: Option<(i32, i32, bool, u64)>,
    /// index of the last menu button activated with L/R
    pub menu_idx: usize,
    /// objects created by binary deserialization (no constructor): shell address
    pub deser: std::collections::HashSet<usize>,
    pub mcache: HashMap<(u32, &'static str), Option<u32>>,
    pub destroyed: std::collections::HashSet<usize>,
    pub wants_text: bool,
    /// User MonoBehaviour components, in scene file order.
    pub user_comps: Vec<Value>,
    start_list: Option<Vec<Value>>,
    pub ptr: crate::input::PointerState,
    pub update_list: Option<(Vec<Value>, Vec<Value>)>,
    /// a simulation step in progress, run in time slices over several display frames (`Vm::frame_start` / `frame_run`)
    pub slice: Option<TickSlice>,
    pub hover_always: bool,
    pub pad_hover: bool,
    pub scrollrects: Vec<Value>,
    pub layout_gos: Option<Vec<Value>>,
    pub layout_seen: u64,
    pub text_req: Option<TextReq>,
}

pub struct TextReq {
    pub comp: Value,
    pub text: String,
    pub max: i32,
    pub numeric: bool,
    pub pending: bool,
}

// ---------------------------------------------------------------- access helpers
pub fn with_comp<R>(v: &Value, f: impl FnOnce(&mut Comp) -> R) -> Option<R> {
    if let Value::Obj(o) = v {
        if let Ext::Native(b) = &mut *o.x.borrow_mut() {
            if let Some(c) = b.downcast_mut::<Comp>() {
                return Some(f(c));
            }
        }
    }
    None
}

pub fn with_go<R>(v: &Value, f: impl FnOnce(&mut GoData) -> R) -> Option<R> {
    if let Value::Obj(o) = v {
        if let Ext::Native(b) = &mut *o.x.borrow_mut() {
            if let Some(c) = b.downcast_mut::<GoData>() {
                return Some(f(c));
            }
        }
    }
    None
}

pub fn with_tr<R>(v: &Value, f: impl FnOnce(&mut Tr) -> R) -> Option<R> {
    with_comp(v, |c| match &mut c.k {
        CK::Transform(t) => Some(f(t)),
        _ => None,
    })
    .flatten()
}

pub fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Obj(x), Value::Obj(y)) => Rc::ptr_eq(x, y),
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}

pub static LAYOUT_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
pub(crate) static INPUT_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

pub(crate) fn invalidate_input() {
    INPUT_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Signals that a layout may have changed (if the node is under a layout).
pub fn bump_layout(v: &Value) {
    let g = go_of(v);
    if with_go(&g, |x| x.in_layout).unwrap_or(false) {
        LAYOUT_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if layout_log() {
            crate::elog!("[layout] bump {}", with_go(&g, |x| x.name.clone()).unwrap_or_default());
        }
    }
}

/// `NGU_LAYOUT_LOG=1`: names the nodes whose change forces a full relayout (dev aid for slow frames).
fn layout_log() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NGU_LAYOUT_LOG").is_some())
}

pub static DIRTYLOG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Name of a sprite (dev tools).
pub fn sprite_name(v: &Value) -> String {
    if v.is_null() {
        return "-".into();
    }
    with_comp(v, |k| if let CK::Sprite(s) = &k.k { s.name.clone() } else { "?".into() }).unwrap_or_else(|| "?".into())
}

/// A Selectable's state changed (interactable, press, hover): invalidates its target graphic (ColorTint tint).
pub fn mark_sel_dirty(v: &Value) {
    let go = go_of(v);
    for c in with_go(&go, |g| g.comps.clone()).unwrap_or_default() {
        if let Some(t) = with_comp(&c, |k| if let CK::Sel(s) = &k.k { Some(s.target_graphic.clone()) } else { None }).flatten() {
            mark_dirty(if t.is_null() { &go } else { &t });
        }
    }
}

/// Invalidates the render caches of the node and all its ancestors.
pub fn mark_dirty(v: &Value) {
    if DIRTYLOG.load(std::sync::atomic::Ordering::Relaxed) {
        let g = go_of(v);
        let n = with_go(&g, |g| g.name.clone()).unwrap_or_default();
        crate::elog!("  dirty: {}", n);
    }
    let mut cur = go_of(v);
    while !cur.is_null() {
        let (tr, active) = match with_go(&cur, |g| {
            g.dirty = true;
            (g.transform.clone(), g.active)
        }) {
            Some(t) => t,
            None => return,
        };
        // an inactive node is never drawn: its ancestors do not change
        if !active {
            return;
        }
        let parent = with_tr(&tr, |t| t.parent.clone()).unwrap_or(Value::Null);
        cur = if parent.is_null() { Value::Null } else { go_of(&parent) };
    }
}

pub fn is_go(v: &Value) -> bool {
    if let Value::Obj(o) = v {
        if let Ext::Native(b) = &*o.x.borrow() {
            return b.is::<GoData>();
        }
    }
    false
}

pub fn go_of(v: &Value) -> Value {
    if is_go(v) {
        return v.clone();
    }
    with_comp(v, |c| c.go.clone()).unwrap_or(Value::Null)
}

pub fn transform_of(v: &Value) -> Value {
    let g = go_of(v);
    with_go(&g, |g| g.transform.clone()).unwrap_or(Value::Null)
}

pub fn active_in_hierarchy(go: &Value) -> bool {
    let epoch = HIER_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
    if let Some(r) = with_go(go, |g| (g.hier.0 == epoch).then_some(g.hier.1)).flatten() {
        return r;
    }
    let r = active_in_hierarchy_walk(go);
    with_go(go, |g| g.hier = (epoch, r));
    r
}

fn active_in_hierarchy_walk(go: &Value) -> bool {
    let mut cur = go.clone();
    loop {
        let (a, tr, dead) = match with_go(&cur, |g| (g.active, g.transform.clone(), g.destroyed)) {
            Some(x) => x,
            None => return false,
        };
        if !a || dead {
            return false;
        }
        let parent = with_tr(&tr, |t| t.parent.clone()).unwrap_or(Value::Null);
        if parent.is_null() {
            return true;
        }
        cur = go_of(&parent);
    }
}

// ---------------------------------------------------------------- JSON
fn jf(j: &J) -> f32 {
    j.as_f64().unwrap_or(0.0) as f32
}
fn jv2(j: &J) -> [f32; 2] {
    [jf(&j["x"]), jf(&j["y"])]
}
fn jv3(j: &J) -> [f32; 3] {
    [jf(&j["x"]), jf(&j["y"]), jf(&j["z"])]
}
fn jcol(j: &J) -> Col {
    if j.is_null() {
        return [1.0; 4];
    }
    [jf(&j["r"]), jf(&j["g"]), jf(&j["b"]), jf(&j["a"])]
}
fn jb(j: &J) -> bool {
    j.as_bool().unwrap_or_else(|| j.as_i64().unwrap_or(0) != 0)
}

impl Engine {
    pub fn get(&self, j: &J) -> Value {
        match j.as_str() {
            Some(k) => self.keys.get(k).cloned().unwrap_or(Value::Null),
            None => Value::Null,
        }
    }
}

fn new_native(vm: &mut Vm, tname: &str, payload: Box<dyn std::any::Any>) -> Value {
    let id = vm.type_id(tname);
    Value::Obj(Obj::new(id, 0, Ext::Native(payload)))
}

pub fn new_comp(vm: &mut Vm, tname: &str, go: Value, k: CK) -> Value {
    new_native(vm, tname, Box::new(Comp { go, enabled: true, awoken: false, started: false, k }))
}

fn user_type_id(vm: &Vm, ns: &str, cls: &str) -> Option<u32> {
    let full = if ns.is_empty() { cls.to_string() } else { format!("{}.{}", ns, cls) };
    vm.type_ids.get(&full).copied().filter(|&i| i < vm.n_user)
}

pub(crate) fn default_sel(kind: SelKind) -> SelD {
    SelD {
        kind,
        interactable: true,
        transition: 1,
        colors: [[1.0; 4], [0.96, 0.96, 0.96, 1.0], [0.78, 0.78, 0.78, 1.0], [0.96, 0.96, 0.96, 1.0], [0.78, 0.78, 0.78, 0.5]],
        target_graphic: Value::Null,
        on_click: Value::Null,
        on_change: Value::Null,
        on_end: Value::Null,
        value: 0.0,
        min: 0.0,
        max: 1.0,
        whole: false,
        direction: 0,
        fill_rect: Value::Null,
        handle_rect: Value::Null,
        is_on: false,
        toggle_graphic: Value::Null,
        text: String::new(),
        text_component: Value::Null,
        placeholder: Value::Null,
        char_limit: 0,
        content_type: 0,
        size: 0.2,
        options: vec![],
        caption: Value::Null,
        focused: false,
        state: 0,
    }
}

pub fn new_event(vm: &mut Vm, e: UEvent) -> Value {
    new_native(vm, "UnityEngine.Events.UnityEvent", Box::new(Comp { go: Value::Null, enabled: true, awoken: true, started: true, k: CK::Event(e) }))
}

fn parse_event(vm: &mut Vm, j: &J) -> Value {
    let mut ev = UEvent::default();
    if let Some(calls) = j["m_PersistentCalls"]["m_Calls"].as_array() {
        for c in calls {
            let a = &c["m_Arguments"];
            ev.persistent.push(PCall {
                target: vm.eng.get(&c["m_Target"]),
                method: c["m_MethodName"].as_str().unwrap_or("").to_string(),
                mode: c["m_Mode"].as_i64().unwrap_or(1) as i32,
                arg_obj: vm.eng.get(&a["m_ObjectArgument"]),
                arg_i: a["m_IntArgument"].as_i64().unwrap_or(0) as i32,
                arg_f: jf(&a["m_FloatArgument"]),
                arg_s: a["m_StringArgument"].as_str().unwrap_or("").to_string(),
                arg_b: jb(&a["m_BoolArgument"]),
            });
        }
    }
    new_event(vm, ev)
}

/// Converts a serialized JSON field to a CLR value according to the declared type.
pub fn from_json(vm: &mut Vm, j: &J, ty: &Ty, elem: Option<&Ty>) -> Value {
    match ty {
        Ty::Bool => Value::I32(jb(j) as i32),
        Ty::Char | Ty::I8 | Ty::U8 | Ty::I16 | Ty::U16 | Ty::I32 | Ty::U32 => Value::I32(j.as_i64().unwrap_or_else(|| j.as_f64().unwrap_or(0.0) as i64) as i32),
        Ty::I64 | Ty::U64 | Ty::IntPtr | Ty::UIntPtr => Value::I64(j.as_i64().or_else(|| j.as_u64().map(|x| x as i64)).unwrap_or(0)),
        Ty::F32 => Value::F32(jf(j)),
        Ty::F64 => Value::F64(j.as_f64().unwrap_or(0.0)),
        Ty::String => match j.as_str() {
            Some(s) => Value::str(s),
            None => Value::Null,
        },
        Ty::Array(e) => match j.as_array() {
            Some(a) => {
                let items: Vec<Value> = a.iter().map(|x| from_json(vm, x, e, None)).collect();
                crate::bcl_core::new_array(vm, items)
            }
            None => Value::Null,
        },
        Ty::User(id) => {
            if let Some(k) = j.as_str() {
                return vm.eng.keys.get(k).cloned().unwrap_or(Value::Null);
            }
            if let Some(d) = j.as_object() {
                if vm.types[*id as usize].is_enum {
                    return Value::I32(0);
                }
                let vals = compute_fields(vm, *id, d);
                if vm.types[*id as usize].is_value {
                    let mut sv = vm.default_value(&Ty::User(*id));
                    if let Value::Struct(st) = &mut sv {
                        for (s, v) in vals {
                            st.f[s] = v;
                        }
                    }
                    return sv;
                }
                let o = vm.construct(*id);
                {
                    let mut f = o.f.borrow_mut();
                    for (s, v) in vals {
                        f[s] = v;
                    }
                }
                return Value::Obj(o);
            }
            if vm.types[*id as usize].is_enum {
                return Value::I32(j.as_i64().unwrap_or(0) as i32);
            }
            Value::Null
        }
        Ty::Ext(name, _) => from_json_ext(vm, j, name, elem),
        Ty::Object | Ty::Generic | Ty::Pointer => match j {
            J::String(s) => vm.eng.keys.get(s).cloned().unwrap_or_else(|| Value::str(s)),
            _ => Value::Null,
        },
    }
}

fn from_json_ext(vm: &mut Vm, j: &J, name: &str, elem: Option<&Ty>) -> Value {
    let mk = |vm: &mut Vm, n: &str, f: Vec<Value>| {
        let id = vm.type_id(n);
        Value::Struct(Box::new(Struct { ty: id, f }))
    };
    match name {
        "UnityEngine.Color" => {
            let c = jcol(j);
            mk(vm, name, c.iter().map(|x| Value::F32(*x)).collect())
        }
        "UnityEngine.Vector2" => {
            let c = jv2(j);
            mk(vm, name, c.iter().map(|x| Value::F32(*x)).collect())
        }
        "UnityEngine.Vector3" => {
            let c = jv3(j);
            mk(vm, name, c.iter().map(|x| Value::F32(*x)).collect())
        }
        "UnityEngine.Rect" => mk(vm, name, vec![Value::F32(jf(&j["x"])), Value::F32(jf(&j["y"])), Value::F32(jf(&j["width"])), Value::F32(jf(&j["height"]))]),
        "System.Collections.Generic.List`1" => match j.as_array() {
            Some(a) => {
                let ety = elem.cloned().unwrap_or(Ty::Object);
                let items: Vec<Value> = a.iter().map(|x| from_json(vm, x, &ety, None)).collect();
                crate::bcl_core::new_list(vm, items)
            }
            None => Value::Null,
        },
        "UnityEngine.Events.UnityEvent" | "UnityEngine.UI.Button/ButtonClickedEvent" => parse_event(vm, j),
        _ => match j {
            J::String(k) => vm.eng.keys.get(k).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        },
    }
}

pub fn compute_fields(vm: &mut Vm, ty: u32, d: &serde_json::Map<String, J>) -> Vec<(usize, Value)> {
    let mut out = vec![];
    for (name, val) in d {
        if matches!(name.as_str(), "m_GameObject" | "m_Enabled" | "m_Name" | "m_EditorHideFlags" | "m_EditorClassIdentifier" | "m_Script") {
            continue;
        }
        let t = &vm.types[ty as usize];
        let slot = match t.fields.iter().position(|n| n == name) {
            Some(s) => s,
            None => continue,
        };
        let fty = t.field_tys[slot].clone();
        let felem = t.field_elems[slot].clone();
        let v = from_json(vm, val, &fty, felem.as_ref());
        out.push((slot, v));
    }
    out
}

pub fn set_fields(vm: &mut Vm, obj: &Value, ty: u32, d: &serde_json::Map<String, J>) {
    let vals = compute_fields(vm, ty, d);
    if let Value::Obj(o) = obj {
        let mut f = o.f.borrow_mut();
        for (s, v) in vals {
            f[s] = v;
        }
    }
}

impl Vm {
    // ---------------------------------------------------------------- life cycle
    fn profile_frame(&self) -> bool {
        static PROFILE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *PROFILE.get_or_init(|| std::env::var_os("NGU_PROFILE").is_some())
            && (self.eng.frame < 5 || self.eng.frame % 30 == 0)
    }

    pub fn call_msg(&mut self, comp: &Value, name: &'static str) -> R<()> {
        let ty = match comp {
            Value::Obj(o) => o.ty,
            _ => return Ok(()),
        };
        if ty >= self.n_user {
            return Ok(());
        }
        let m = match self.eng.mcache.get(&(ty, name)) {
            Some(m) => *m,
            None => {
                let m = self.find_method(ty, name, 1);
                self.eng.mcache.insert((ty, name), m);
                m
            }
        };
        if let Some(m) = m {
            let profile = self.profile_frame().then(|| (self.steps, std::time::Instant::now()));
            if let Err(e) = self.invoke(m, [comp.clone()]) {
                self.report_exc(name, comp, &e);
            }
            if let Some((steps, start)) = profile {
                let elapsed = start.elapsed();
                if elapsed.as_millis() >= 4 {
                    crate::elog!("[slow] frame={} {}::{name} {elapsed:?} instr={}", self.eng.frame, self.types[ty as usize].name, self.steps - steps);
                }
            }
        }
        Ok(())
    }

    pub fn report_exc(&mut self, ctx: &str, comp: &Value, e: &Value) {
        self.eng.exceptions += 1;
        let n = self.type_name_of(comp);
        let t = self.type_name_of(e);
        let msg = if let Value::Obj(o) = e { o.f.borrow().first().cloned() } else { None };
        let msg = msg.map(|m| crate::bcl_core::to_string(self, &m).unwrap_or_default()).unwrap_or_default();
        let trace = if let Value::Obj(o) = e {
            match &*o.x.borrow() {
                Ext::Native(b) => b.downcast_ref::<String>().cloned().unwrap_or_default(),
                _ => String::new(),
            }
        } else {
            String::new()
        };
        crate::elog!("[unity] exception in {}::{}: {}: {}   @ {}", n, ctx, t, msg, trace);
    }

    /// Awake + OnEnable of all active components, in file order (Start deferred).
    pub fn awake_all(&mut self) {
        let comps = self.eng.user_comps.clone();
        for c in &comps {
            let go = go_of(c);
            if !active_in_hierarchy(&go) {
                continue;
            }
            let st = with_comp(c, |k| (k.enabled, k.awoken)).unwrap_or((false, true));
            if !st.1 {
                with_comp(c, |k| k.awoken = true);
                let _ = self.call_msg(c, "Awake");
            }
            if st.0 {
                let _ = self.call_msg(c, "OnEnable");
            }
        }
        crate::save_slots::prepare_title(self);
        crate::console_mode::prepare(self);
    }

    pub fn awake_go(&mut self, go: &Value) {
        let comps = with_go(go, |g| g.comps.clone()).unwrap_or_default();
        for c in &comps {
            let go_enabled = with_comp(c, |k| (k.enabled, k.awoken, matches!(k.k, CK::User))).unwrap_or((false, true, false));
            if !go_enabled.2 {
                continue;
            }
            if !go_enabled.1 {
                with_comp(c, |k| k.awoken = true);
                let _ = self.call_msg(c, "Awake");
            }
            if go_enabled.0 {
                let _ = self.call_msg(c, "OnEnable");
            }
        }
    }

    pub fn start_pending(&mut self) {
        let mut pending = self.eng.start_list.take().unwrap_or_else(|| {
            self.eng.user_comps.iter().filter(|c| {
                matches!(c, Value::Obj(o) if self.find_method(o.ty, "Start", 1).is_some())
                    && with_comp(c, |k| !k.started).unwrap_or(false)
            }).cloned().collect()
        });
        pending.retain(|c| {
            let enabled_go = with_comp(c, |k| (k.enabled && !k.started).then(|| k.go.clone())).flatten();
            if enabled_go.is_some_and(|go| active_in_hierarchy(&go)) {
                with_comp(c, |k| k.started = true);
                let _ = self.call_msg(c, "Start");
                return false;
            }
            true
        });
        self.eng.start_list = Some(pending);
    }

    pub fn set_active(&mut self, go: &Value, on: bool) {
        let was = active_in_hierarchy(go);
        let self_changed = with_go(go, |g| std::mem::replace(&mut g.active, on) != on).unwrap_or(false);
        if self_changed {
            bump_hierarchy();
        }
        let now = active_in_hierarchy(go);
        if self_changed {
            mark_dirty(go);
            bump_layout(go);
            invalidate_input();
        }
        if was == now {
            return;
        }
        let mut stack = vec![go.clone()];
        while let Some(g) = stack.pop() {
            if !with_go(&g, |g| g.active).unwrap_or(false) {
                continue;
            }
            let comps = with_go(&g, |x| x.comps.clone()).unwrap_or_default();
            if now {
                self.awake_go(&g);
            } else {
                for c in &comps {
                    if with_comp(c, |k| matches!(k.k, CK::User) && k.enabled).unwrap_or(false) {
                        let _ = self.call_msg(c, "OnDisable");
                    }
                }
            }
            let tr = with_go(&g, |x| x.transform.clone()).unwrap_or(Value::Null);
            let ch = with_tr(&tr, |t| t.children.clone()).unwrap_or_default();
            for c in ch {
                stack.push(go_of(&c));
            }
        }
    }

    /// One frame: Update of all components, coroutines, Invoke.
    pub fn frame(&mut self, dt: f32) {
        if self.frame_start(dt) {
            self.frame_run(None);
        }
    }

    /// Starts a simulation step: time, Start, pointer events and the list of Update components. Returns false when nothing
    /// follows (the title pause handled the frame). `frame_run` then executes the step, possibly over several calls.
    pub fn frame_start(&mut self, dt: f32) -> bool {
        let away = self.away_seconds();
        if crate::save_slots::pause_on_title(self) {
            self.pointer_update();
            self.mouse_input_finished();
            return false;
        }
        if let Some(seconds) = away {
            self.apply_away(seconds);
        }
        self.eng.dt = dt;
        self.eng.time += dt as f64;
        self.eng.frame += 1;
        if self.eng.motion.enabled {
            self.eng.motion.clock = Some((self.eng.time, f64::from(dt)));
        }
        self.start_pending();
        self.pointer_update();
        // Update: only the components whose class defines Update
        if self.eng.update_list.is_none() {
            let (mut ups, mut lates) = (vec![], vec![]);
            for c in self.eng.user_comps.clone() {
                if let Value::Obj(o) = &c {
                    if self.find_method(o.ty, "Update", 1).is_some() {
                        ups.push(c.clone());
                    }
                    if self.find_method(o.ty, "LateUpdate", 1).is_some() {
                        lates.push(c.clone());
                    }
                }
            }
            self.eng.update_list = Some((ups, lates));
        }
        let ups = self.eng.update_list.as_ref().unwrap().0.clone();
        let clock = self.eng.motion.clock.take();
        self.eng.slice = Some(TickSlice { ups, idx: 0, clock });
        true
    }

    /// Runs the step started by `frame_start`. With a deadline the Update loop stops between two components once it has passed
    /// (at least one component always runs) and returns false: call again on a later display frame to continue. The order of
    /// every callback is the same as in one go, so the game state after the step does not depend on the slicing.
    pub fn frame_run(&mut self, deadline: Option<std::time::Instant>) -> bool {
        let Some(mut slice) = self.eng.slice.take() else { return true };
        self.eng.motion.clock = slice.clock.take();
        while slice.idx < slice.ups.len() {
            let c = slice.ups[slice.idx].clone();
            slice.idx += 1;
            if with_comp(&c, |k| k.enabled).unwrap_or(false) && active_in_hierarchy(&go_of(&c)) {
                let _ = self.call_msg(&c, "Update");
            }
            if slice.idx < slice.ups.len() && deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                slice.clock = self.eng.motion.clock.take();
                self.eng.slice = Some(slice);
                return false;
            }
        }
        self.run_invokes();
        self.run_coroutines();
        let lates = self.eng.update_list.as_ref().unwrap().1.clone();
        for c in &lates {
            if with_comp(c, |k| k.enabled).unwrap_or(false) && active_in_hierarchy(&go_of(c)) {
                let _ = self.call_msg(c, "LateUpdate");
            }
        }
        crate::achievements::tick(self);
        self.eng.motion.clock = None;
        self.mouse_input_finished();
        true
    }

    fn mouse_input_finished(&mut self) {
        self.eng.mouse_down_edge = false;
        self.eng.mouse_up_edge = false;
        self.eng.wheel = 0.0;
        self.eng.keys_edge.clear();
    }

    // ---------------------------------------------------------------- events
    pub fn invoke_event(&mut self, ev: &Value, args: Vec<Value>) {
        let (pcs, rts): (Vec<usize>, Vec<Deleg>) = match with_comp(ev, |c| match &c.k {
            CK::Event(e) => Some((e.persistent.len(), e.runtime.clone())),
            _ => None,
        })
        .flatten()
        {
            Some((n, r)) => ((0..n).collect(), r),
            None => return,
        };
        for i in pcs {
            let call = with_comp(ev, |c| match &c.k {
                CK::Event(e) => {
                    let p = &e.persistent[i];
                    Some((p.target.clone(), p.method.clone(), p.mode, p.arg_obj.clone(), p.arg_i, p.arg_f, p.arg_s.clone(), p.arg_b))
                }
                _ => None,
            })
            .flatten();
            if let Some((target, method, mode, aobj, ai, af, as_, ab)) = call {
                self.invoke_pcall(&target, &method, mode, aobj, ai, af, &as_, ab, &args);
            }
        }
        for d in rts {
            let m = &self.prog.methods[d.method as usize];
            let mut a = vec![];
            if !m.is_static || m.argc == args.len() + 1 {
                a.push(d.target.clone());
            }
            a.extend(args.iter().cloned());
            if let Err(e) = self.invoke(d.method, a) {
                self.report_exc("event", &d.target, &e);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn invoke_pcall(&mut self, target: &Value, method: &str, mode: i32, aobj: Value, ai: i32, af: f32, as_: &str, ab: bool, event_args: &[Value]) {
        if target.is_null() {
            return;
        }
        let arg = match mode {
            2 => Some(aobj),
            3 => Some(Value::I32(ai)),
            4 => Some(Value::F32(af)),
            5 => Some(Value::str(as_)),
            6 => Some(Value::I32(ab as i32)),
            _ => None,
        };
        // GameObject.SetActive etc.
        if is_go(target) {
            if method == "SetActive" {
                let active = if mode == 0 { event_args.first().map(Value::as_bool).unwrap_or(ab) } else { ab };
                self.set_active(target, active);
            }
            return;
        }
        let ty = match target {
            Value::Obj(o) => o.ty,
            _ => return,
        };
        let name = method.strip_prefix("set_").map(|m| format!("set_{}", m)).unwrap_or(method.to_string());
        let args = if mode == 0 { event_args } else { arg.as_slice() };
        if ty < self.n_user {
            if let Some(m) = self.find_method(ty, &name, args.len() + 1) {
                let mut a = vec![target.clone()];
                a.extend_from_slice(args);
                if let Err(e) = self.invoke(m, a) {
                    self.report_exc(method, target, &e);
                }
                return;
            }
        }
        crate::elog!("[unity] unresolved pcall: {}.{} (mode {})", self.type_name_of(target), method, mode);
    }
}

/// Tag name of a tag id: built-in tags, then the project's custom tags (id 20000 + index, listed in `tags.json`).
/// Text snapshot of the game's visible state (one line per GameObject, active or not): used to compare
/// the output of deterministic scene replays.
pub fn dump_state(vm: &Vm) -> String {
    let mut lines: Vec<String> = vec![];
    for go in &vm.eng.gos {
        let (name, active, tr, comps, destroyed) = match with_go(go, |g| (g.name.clone(), g.active, g.transform.clone(), g.comps.clone(), g.destroyed)) {
            Some(x) => x,
            None => continue,
        };
        // path = names of the parents
        let mut path = vec![name.clone()];
        let mut cur = tr.clone();
        loop {
            let p = with_tr(&cur, |t| t.parent.clone()).unwrap_or(Value::Null);
            if p.is_null() {
                break;
            }
            let pg = go_of(&p);
            match with_go(&pg, |g| g.name.clone()) {
                Some(n) => path.push(n),
                None => break,
            }
            cur = p;
        }
        path.reverse();
        let mut extra = String::new();
        for c in &comps {
            with_comp(c, |k| match &k.k {
                CK::Text(t) => extra.push_str(&format!(" text={:?}", t.text)),
                CK::Image(i) => extra.push_str(&format!(" img(fill={:.4},col={:.3}/{:.3}/{:.3}/{:.3})", i.fill_amount, i.color[0], i.color[1], i.color[2], i.color[3])),
                _ => {}
            });
        }
        if let Some((x, y, z)) = with_tr(&tr, |t| (t.pos[0], t.pos[1], t.pos[2])) {
            extra.push_str(&format!(" pos=({:.2},{:.2},{:.2})", x, y, z));
        }
        lines.push(format!("{}|{}|{}{}", path.join("/"), if active { "on" } else { "off" }, if destroyed { "X" } else { "" }, extra));
    }
    lines.join("\n")
}

/// Native type name of Dropdown options.
pub const OPTION_DATA: &str = "UnityEngine.UI.Dropdown/OptionData";

/// `Dropdown.OptionData`: text and image of one option.
pub struct OptD {
    pub text: String,
    pub image: Value,
}
