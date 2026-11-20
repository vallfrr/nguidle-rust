//! Minimal Unity engine: GameObject/Transform/UI components, scene loading, life cycle.
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
    pub options: Vec<(String, Value)>,
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
    TextAsset { name: String, key: String },
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
    pub method: String,
    pub next: f64,
    pub repeat: f64,
}

#[derive(Default)]
pub struct Engine {
    pub keys: HashMap<String, Value>,
    pub gos: Vec<Value>,
    pub time: f64,
    pub dt: f32,
    pub frame: u64,
    pub coroutines: Vec<Co>,
    pub invokes: Vec<Inv>,
    pub screen: (i32, i32),
    pub prefs: HashMap<String, Value>,
    pub assets_dir: String,
    pub save_dir: String,
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
    /// index of the last menu button activated with L/R
    pub menu_idx: usize,
    /// objects created by binary deserialization (no constructor): shell address
    pub deser: std::collections::HashSet<usize>,
    pub mcache: HashMap<(u32, &'static str), Option<u32>>,
    pub destroyed: std::collections::HashSet<usize>,
    pub wants_text: bool,
    /// User MonoBehaviour components, in scene file order.
    pub user_comps: Vec<Value>,
    pub ptr: crate::input::PointerState,
    pub update_list: Option<(Vec<Value>, Vec<Value>)>,
    pub hover_always: bool,
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

/// Signals that a layout may have changed (if the node is under a layout).
pub fn bump_layout(v: &Value) {
    let g = go_of(v);
    if with_go(&g, |x| x.in_layout).unwrap_or(false) {
        LAYOUT_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
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

fn default_sel(kind: SelKind) -> SelD {
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

// ---------------------------------------------------------------- scene loading
impl Vm {
    /// Loads scene.json and builds all objects (parsed object by object: no global DOM).
    pub fn load_scene(&mut self, dir: &str) -> Result<(), String> {
        self.eng.assets_dir = dir.to_string();
        crate::assets::init(dir);
        let txt = crate::assets::read_string("scene.json").ok_or("scene.json not found")?;
        #[derive(serde::Deserialize)]
        struct Root<'a> {
            #[serde(borrow)]
            objects: indexmap::IndexMap<&'a str, &'a serde_json::value::RawValue>,
        }
        let root: Root = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
        let objs = &root.objects;
        let kind = |raw: &serde_json::value::RawValue| -> String {
            let s = raw.get();
            match s.strip_prefix("{\"t\":\"") {
                Some(r) => r.split('"').next().unwrap_or("").to_string(),
                None => String::new(),
            }
        };
        let pj = |raw: &serde_json::value::RawValue| -> J { serde_json::from_str(raw.get()).unwrap_or(J::Null) };

        // 1. passive objects: textures, fonts, texts, GameObjects
        for (k, raw) in objs.iter() {
            let t = kind(raw);
            let k = k.to_string();
            match t.as_str() {
                "Texture2D" => {
                    let v = pj(raw);
                    let o = new_comp(self, "UnityEngine.Texture2D", Value::Null, CK::Texture(Box::new(TexD { name: v["name"].as_str().unwrap_or("").into(), key: k.clone(), w: v["w"].as_i64().unwrap_or(0) as i32, h: v["h"].as_i64().unwrap_or(0) as i32 })));
                    self.eng.keys.insert(k, o);
                }
                "Font" => {
                    let v = pj(raw);
                    let o = new_comp(self, "UnityEngine.Font", Value::Null, CK::Font { name: v["name"].as_str().unwrap_or("").into(), file: v["file"].as_str().unwrap_or("").into() });
                    self.eng.keys.insert(k, o);
                }
                "TextAsset" => {
                    let v = pj(raw);
                    let o = new_comp(self, "UnityEngine.TextAsset", Value::Null, CK::TextAsset { name: v["name"].as_str().unwrap_or("").into(), key: k.clone() });
                    self.eng.keys.insert(k, o);
                }
                "GameObject" => {
                    let v = pj(raw);
                    let d = &v["d"];
                    let o = new_native(
                        self,
                        "UnityEngine.GameObject",
                        Box::new(GoData {
                            name: d["m_Name"].as_str().unwrap_or("").into(),
                            active: jb(&d["m_IsActive"]),
                            layer: d["m_Layer"].as_i64().unwrap_or(0) as i32,
                            tag: tag_name(d["m_Tag"].as_i64().unwrap_or(0)),
                            comps: vec![],
                            transform: Value::Null,
                            destroyed: false,
                            dirty: true,
                            cache: None,
                            in_layout: false,
                        }),
                    );
                    self.eng.keys.insert(k, o.clone());
                    self.eng.gos.push(o);
                }
                _ => {}
            }
        }
        // sprites (after textures)
        for (k, raw) in objs.iter() {
            if kind(raw) == "Sprite" {
                let v = pj(raw);
                let d = &v["d"];
                let r = &d["m_Rect"];
                let tex = self.eng.get(&d["m_RD"]["texture"]);
                let b = &d["m_Border"];
                let o = new_comp(
                    self,
                    "UnityEngine.Sprite",
                    Value::Null,
                    CK::Sprite(Box::new(SpriteD {
                        name: d["m_Name"].as_str().unwrap_or("").into(),
                        tex,
                        rect: [jf(&r["x"]), jf(&r["y"]), jf(&r["width"]), jf(&r["height"])],
                        border: [jf(&b["x"]), jf(&b["y"]), jf(&b["z"]), jf(&b["w"])],
                        pivot: jv2(&d["m_Pivot"]),
                        ppu: jf(&d["m_PixelsToUnits"]),
                    })),
                );
                self.eng.keys.insert(k.to_string(), o);
            }
        }
        // 2. components (empty objects first for cross references)
        #[derive(serde::Deserialize)]
        struct ScriptH {
            ns: String,
            cls: String,
            asm: String,
        }
        #[derive(serde::Deserialize, Default)]
        struct HeadD {
            #[serde(default)]
            m_GameObject: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct Head {
            #[serde(default)]
            script: Option<ScriptH>,
            d: HeadD,
        }
        let mut comp_list: Vec<(String, Value)> = vec![];
        for (k, raw) in objs.iter() {
            let t = kind(raw);
            if !matches!(t.as_str(), "Transform" | "RectTransform" | "CanvasRenderer" | "Canvas" | "Camera" | "MonoBehaviour") {
                continue;
            }
            let head: Head = match serde_json::from_str(raw.get()) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let goref = match &head.d.m_GameObject {
                Some(s) => self.eng.keys.get(s).cloned().unwrap_or(Value::Null),
                None => Value::Null,
            };
            let o = match t.as_str() {
                "Transform" | "RectTransform" => new_comp(
                    self,
                    if t == "RectTransform" { "UnityEngine.RectTransform" } else { "UnityEngine.Transform" },
                    goref,
                    CK::Transform(Box::new(Tr { parent: Value::Null, children: vec![], pos: [0.0; 3], scale: [1.0; 3], rot_z: 0.0, amin: [0.5; 2], amax: [0.5; 2], pivot: [0.5; 2], apos: [0.0; 2], size: [0.0; 2], is_rect: t == "RectTransform" })),
                ),
                "CanvasRenderer" => new_comp(self, "UnityEngine.CanvasRenderer", goref, CK::Other("CanvasRenderer".into())),
                "Canvas" => new_comp(self, "UnityEngine.Canvas", goref, CK::Canvas(Box::new(CanvasD { mode: 0, sorting: 0, override_sorting: false, scale_factor: 1.0 }))),
                "Camera" => new_comp(self, "UnityEngine.Camera", goref, CK::Other("Camera".into())),
                _ => {
                    let s = match &head.script {
                        Some(s) => s,
                        None => continue,
                    };
                    let (asm, ns, cls) = (s.asm.as_str(), s.ns.as_str(), s.cls.as_str());
                    if asm == "Assembly-CSharp.dll" {
                        match user_type_id(self, ns, cls) {
                            Some(id) => {
                                let ov = self.construct(id);
                                *ov.x.borrow_mut() = Ext::Native(Box::new(Comp { go: goref, enabled: true, awoken: false, started: false, k: CK::User }));
                                self.eng.user_comps.push(Value::Obj(ov.clone()));
                                Value::Obj(ov)
                            }
                            None => new_comp(self, "UnityEngine.MonoBehaviour", goref, CK::Other(format!("?{}", cls))),
                        }
                    } else {
                        let full = format!("{}.{}", ns, cls);
                        let kk = unity_kind(&full);
                        new_comp(self, &full, goref, kk)
                    }
                }
            };
            self.eng.keys.insert(k.to_string(), o.clone());
            comp_list.push((k.to_string(), o));
        }
        // 3. attach components to GameObjects (m_Component order)
        for (k, raw) in objs.iter() {
            if kind(raw) != "GameObject" {
                continue;
            }
            let v = pj(raw);
            let go = self.eng.keys[*k].clone();
            let mut list = vec![];
            let mut tr = Value::Null;
            for c in v["d"]["m_Component"].as_array().cloned().unwrap_or_default() {
                let cv = self.eng.get(&c["component"]);
                if cv.is_null() {
                    continue;
                }
                if with_tr(&cv, |_| ()).is_some() {
                    tr = cv.clone();
                }
                list.push(cv);
            }
            with_go(&go, |g| {
                g.comps = list;
                g.transform = tr;
            });
        }
        // 4. component data (one full parse per object, transient)
        for (k, o) in &comp_list {
            if let Some(raw) = objs.get(k.as_str()) {
                let v = pj(raw);
                self.fill_component(o, &v);
            }
        }
        for (_, o) in &comp_list {
            if with_comp(o, |c| matches!(&c.k, CK::Sel(s) if s.kind == SelKind::Slider || s.kind == SelKind::Scrollbar)).unwrap_or(false) {
                crate::unity_api::slider_visuals(self, o);
            }
        }
        // subtrees under a layout
        for go in self.eng.gos.clone() {
            let has = with_go(&go, |g| g.comps.clone()).unwrap_or_default().iter().any(|c| with_comp(c, |k| matches!(k.k, CK::Layout { .. })).unwrap_or(false));
            if !has {
                continue;
            }
            let mut stack = vec![go.clone()];
            while let Some(g) = stack.pop() {
                with_go(&g, |x| x.in_layout = true);
                let tr = with_go(&g, |x| x.transform.clone()).unwrap_or(Value::Null);
                for c in with_tr(&tr, |t| t.children.clone()).unwrap_or_default() {
                    stack.push(go_of(&c));
                }
            }
        }
        // Resources.Load index by name
        let keys: Vec<(String, Value)> = self.eng.keys.iter().filter(|(k, _)| k.starts_with("resources.assets:")).map(|(k, v)| (k.clone(), v.clone())).collect();
        for (_, v) in keys {
            let n = with_comp(&v, |c| match &c.k {
                CK::Sprite(s) => Some(s.name.clone()),
                CK::TextAsset { name, .. } => Some(name.clone()),
                _ => None,
            })
            .flatten();
            if let Some(n) = n {
                self.eng.keys.insert(format!("res:{}", n), v);
            }
        }
        Ok(())
    }

    fn fill_component(&mut self, o: &Value, v: &J) {
        let d = &v["d"];
        let enabled = d.get("m_Enabled").map(jb).unwrap_or(true);
        let t = v["t"].as_str().unwrap_or("");
        let _ = with_comp(o, |c| c.enabled = enabled);
        match t {
            "Transform" | "RectTransform" => {
                let parent = self.eng.get(&d["m_Father"]);
                let children: Vec<Value> = d["m_Children"].as_array().map(|a| a.iter().map(|x| self.eng.get(x)).collect()).unwrap_or_default();
                with_tr(o, |t| {
                    t.parent = parent;
                    t.children = children;
                    t.pos = jv3(&d["m_LocalPosition"]);
                    t.scale = jv3(&d["m_LocalScale"]);
                    let q = &d["m_LocalRotation"];
                    let (z, w) = (jf(&q["z"]), jf(&q["w"]));
                    t.rot_z = 2.0 * z.atan2(w).to_degrees();
                    if t.is_rect {
                        t.amin = jv2(&d["m_AnchorMin"]);
                        t.amax = jv2(&d["m_AnchorMax"]);
                        t.pivot = jv2(&d["m_Pivot"]);
                        t.apos = jv2(&d["m_AnchoredPosition"]);
                        t.size = jv2(&d["m_SizeDelta"]);
                    }
                });
            }
            "Canvas" => {
                with_comp(o, |c| {
                    if let CK::Canvas(cd) = &mut c.k {
                        cd.mode = d["m_RenderMode"].as_i64().unwrap_or(0) as i32;
                        cd.sorting = d["m_SortingOrder"].as_i64().unwrap_or(0) as i32;
                        cd.override_sorting = jb(&d["m_OverrideSorting"]);
                    }
                });
            }
            "MonoBehaviour" => {
                let s = &v["script"];
                let (asm, cls) = (s["asm"].as_str().unwrap_or(""), s["cls"].as_str().unwrap_or(""));
                if asm == "Assembly-CSharp.dll" {
                    if let Value::Obj(ob) = o {
                        let ty = ob.ty;
                        if let Some(m) = d.as_object() {
                            set_fields(self, o, ty, m);
                        }
                    }
                } else {
                    self.fill_ui(o, cls, d);
                }
            }
            _ => {}
        }
    }

    fn fill_ui(&mut self, o: &Value, cls: &str, d: &J) {
        let sprite = self.eng.get(&d["m_Sprite"]);
        let colors = &d["m_Colors"];
        let sel_colors = [jcol(&colors["m_NormalColor"]), jcol(&colors["m_HighlightedColor"]), jcol(&colors["m_PressedColor"]), jcol(&colors["m_SelectedColor"]), jcol(&colors["m_DisabledColor"])];
        match cls {
            "Image" | "RawImage" => {
                let im = ImageD {
                    sprite,
                    color: jcol(&d["m_Color"]),
                    typ: d["m_Type"].as_i64().unwrap_or(0) as i32,
                    fill_center: d.get("m_FillCenter").map(jb).unwrap_or(true),
                    fill_method: d["m_FillMethod"].as_i64().unwrap_or(0) as i32,
                    fill_amount: d.get("m_FillAmount").map(jf).unwrap_or(1.0),
                    fill_origin: d["m_FillOrigin"].as_i64().unwrap_or(0) as i32,
                    clockwise: d.get("m_FillClockwise").map(jb).unwrap_or(true),
                    preserve_aspect: jb(&d["m_PreserveAspect"]),
                    raycast: d.get("m_RaycastTarget").map(jb).unwrap_or(true),
                };
                with_comp(o, |c| c.k = if cls == "Image" { CK::Image(Box::new(im)) } else { CK::RawImage(Box::new(im)) });
            }
            "Text" => {
                let fd = &d["m_FontData"];
                let font = self.eng.get(&fd["m_Font"]);
                let t = TextD {
                    text: d["m_Text"].as_str().unwrap_or("").into(),
                    font,
                    size: fd["m_FontSize"].as_i64().unwrap_or(14) as i32,
                    style: fd["m_FontStyle"].as_i64().unwrap_or(0) as i32,
                    best_fit: jb(&fd["m_BestFit"]),
                    min_size: fd["m_MinSize"].as_i64().unwrap_or(10) as i32,
                    max_size: fd["m_MaxSize"].as_i64().unwrap_or(40) as i32,
                    align: fd["m_Alignment"].as_i64().unwrap_or(0) as i32,
                    rich: fd.get("m_RichText").map(jb).unwrap_or(true),
                    hov: fd["m_HorizontalOverflow"].as_i64().unwrap_or(0) as i32,
                    vov: fd["m_VerticalOverflow"].as_i64().unwrap_or(0) as i32,
                    line_spacing: fd.get("m_LineSpacing").map(jf).unwrap_or(1.0),
                    color: jcol(&d["m_Color"]),
                    raycast: d.get("m_RaycastTarget").map(jb).unwrap_or(true),
                    layout: None,
                };
                with_comp(o, |c| c.k = CK::Text(Box::new(t)));
            }
            "Button" | "Slider" | "Toggle" | "InputField" | "Dropdown" | "Scrollbar" => {
                let kind = match cls {
                    "Button" => SelKind::Button,
                    "Slider" => SelKind::Slider,
                    "Toggle" => SelKind::Toggle,
                    "InputField" => SelKind::InputField,
                    "Dropdown" => SelKind::Dropdown,
                    _ => SelKind::Scrollbar,
                };
                let mut s = default_sel(kind);
                s.interactable = d.get("m_Interactable").map(jb).unwrap_or(true);
                s.transition = d["m_Transition"].as_i64().unwrap_or(1) as i32;
                s.colors = sel_colors;
                s.target_graphic = self.eng.get(&d["m_TargetGraphic"]);
                match kind {
                    SelKind::Button => s.on_click = parse_event(self, &d["m_OnClick"]),
                    SelKind::Slider => {
                        s.min = d.get("m_MinValue").map(jf).unwrap_or(0.0);
                        s.max = d.get("m_MaxValue").map(jf).unwrap_or(1.0);
                        s.whole = jb(&d["m_WholeNumbers"]);
                        s.value = jf(&d["m_Value"]);
                        s.direction = d["m_Direction"].as_i64().unwrap_or(0) as i32;
                        s.fill_rect = self.eng.get(&d["m_FillRect"]);
                        s.handle_rect = self.eng.get(&d["m_HandleRect"]);
                        s.on_change = parse_event(self, &d["m_OnValueChanged"]);
                    }
                    SelKind::Toggle => {
                        s.is_on = jb(&d["m_IsOn"]);
                        s.toggle_graphic = self.eng.get(&d["graphic"]);
                        s.on_change = parse_event(self, &d["onValueChanged"]);
                    }
                    SelKind::InputField => {
                        s.text = d["m_Text"].as_str().unwrap_or("").into();
                        s.text_component = self.eng.get(&d["m_TextComponent"]);
                        s.placeholder = self.eng.get(&d["m_Placeholder"]);
                        s.char_limit = d["m_CharacterLimit"].as_i64().unwrap_or(0) as i32;
                        s.content_type = d["m_ContentType"].as_i64().unwrap_or(0) as i32;
                        s.on_change = parse_event(self, &d["m_OnValueChanged"]);
                        s.on_end = parse_event(self, &d["m_OnEndEdit"]);
                    }
                    SelKind::Dropdown => {
                        s.value = d["m_Value"].as_i64().unwrap_or(0) as f32;
                        s.on_change = parse_event(self, &d["m_OnValueChanged"]);
                        if let Some(a) = d["m_Options"]["m_Options"].as_array() {
                            s.options = a.iter().map(|x| (x["m_Text"].as_str().unwrap_or("").to_string(), self.eng.get(&x["m_Image"]))).collect();
                        }
                    }
                    SelKind::Scrollbar => {
                        s.value = jf(&d["m_Value"]);
                        s.size = jf(&d["m_Size"]);
                        s.direction = d["m_Direction"].as_i64().unwrap_or(0) as i32;
                        s.handle_rect = self.eng.get(&d["m_HandleRect"]);
                        s.on_change = parse_event(self, &d["m_OnValueChanged"]);
                    }
                }
                with_comp(o, |c| c.k = CK::Sel(Box::new(s)));
            }
            "EventTrigger" => {
                let mut list = vec![];
                if let Some(a) = d["m_Delegates"].as_array() {
                    for e in a {
                        let ev = parse_event(self, &e["callback"]);
                        list.push((e["eventID"].as_i64().unwrap_or(0) as i32, ev));
                    }
                }
                with_comp(o, |c| c.k = CK::EventTrigger(list));
            }
            "ScrollRect" => {
                let k = CK::ScrollRect {
                    content: self.eng.get(&d["m_Content"]),
                    viewport: self.eng.get(&d["m_Viewport"]),
                    hscroll: self.eng.get(&d["m_HorizontalScrollbar"]),
                    vscroll: self.eng.get(&d["m_VerticalScrollbar"]),
                    horizontal: jb(&d["m_Horizontal"]),
                    vertical: jb(&d["m_Vertical"]),
                    norm: [0.0, 1.0],
                    sens: d.get("m_ScrollSensitivity").map(jf).unwrap_or(1.0),
                };
                with_comp(o, |c| c.k = k);
                self.eng.scrollrects.push(o.clone());
            }
            "Mask" => {
                with_comp(o, |c| c.k = CK::Mask { show_graphic: jb(&d["m_ShowMaskGraphic"]) });
            }
            "GridLayoutGroup" | "VerticalLayoutGroup" | "HorizontalLayoutGroup" | "ContentSizeFitter" | "LayoutElement" => {
                let mut m = HashMap::new();
                if let Some(obj) = d.as_object() {
                    flatten_nums(obj, "", &mut m);
                }
                with_comp(o, |c| c.k = CK::Layout { kind: cls.to_string(), d: m });
            }
            "Outline" | "Shadow" => {
                let k = CK::Outline { color: jcol(&d["m_EffectColor"]), dist: jv2(&d["m_EffectDistance"]) };
                with_comp(o, |c| c.k = k);
            }
            "CanvasScaler" => {
                let k = CK::CanvasScaler { mode: d["m_UiScaleMode"].as_i64().unwrap_or(0) as i32, scale: d.get("m_ScaleFactor").map(jf).unwrap_or(1.0), reference: jv2(&d["m_ReferenceResolution"]), match_: jf(&d["m_MatchWidthOrHeight"]) };
                with_comp(o, |c| c.k = k);
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- life cycle
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
            if let Err(e) = self.invoke(m, vec![comp.clone()]) {
                self.report_exc(name, comp, &e);
            }
        }
        Ok(())
    }

    pub fn report_exc(&mut self, ctx: &str, comp: &Value, e: &Value) {
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
        let comps = self.eng.user_comps.clone();
        for c in &comps {
            let go = go_of(c);
            let st = with_comp(c, |k| k.enabled && !k.started).unwrap_or(false);
            if st && active_in_hierarchy(&go) {
                with_comp(c, |k| k.started = true);
                let _ = self.call_msg(c, "Start");
            }
        }
    }

    pub fn set_active(&mut self, go: &Value, on: bool) {
        let was = active_in_hierarchy(go);
        let self_changed = with_go(go, |g| std::mem::replace(&mut g.active, on) != on).unwrap_or(false);
        let now = active_in_hierarchy(go);
        if self_changed {
            mark_dirty(go);
            bump_layout(go);
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
        self.eng.dt = dt;
        self.eng.time += dt as f64;
        self.eng.frame += 1;
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
        for c in &ups {
            if with_comp(c, |k| k.enabled).unwrap_or(false) && active_in_hierarchy(&go_of(c)) {
                let _ = self.call_msg(c, "Update");
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
        self.eng.mouse_down_edge = false;
        self.eng.mouse_up_edge = false;
        self.eng.wheel = 0.0;
        self.eng.keys_edge.clear();
    }

    fn run_invokes(&mut self) {
        let now = self.eng.time;
        let mut due = vec![];
        for (i, inv) in self.eng.invokes.iter().enumerate() {
            if inv.next <= now {
                due.push(i);
            }
        }
        let mut remove = vec![];
        for i in due {
            let (owner, method, repeat) = {
                let v = &mut self.eng.invokes[i];
                let r = (v.owner.clone(), v.method.clone(), v.repeat);
                if v.repeat > 0.0 {
                    v.next += v.repeat;
                } else {
                    remove.push(i);
                }
                r
            };
            let _ = repeat;
            if let Value::Obj(o) = &owner {
                if let Some(m) = self.find_method(o.ty, &method, 1) {
                    if let Err(e) = self.invoke(m, vec![owner.clone()]) {
                        self.report_exc(&method, &owner, &e);
                    }
                }
            }
        }
        for i in remove.into_iter().rev() {
            self.eng.invokes.remove(i);
        }
    }

    pub fn start_coroutine(&mut self, owner: &Value, e: Value) -> Value {
        self.eng.coroutines.push(Co { owner: owner.clone(), stack: vec![e], wait: Wait::Frame, dead: false });
        let id = self.type_id("UnityEngine.Coroutine");
        // first immediate advance (like Unity)
        let idx = self.eng.coroutines.len() - 1;
        self.advance_co(idx);
        Value::Obj(Obj::new(id, 0, Ext::None))
    }

    fn run_coroutines(&mut self) {
        let n = self.eng.coroutines.len();
        for i in 0..n {
            if i >= self.eng.coroutines.len() {
                break;
            }
            if self.eng.coroutines[i].dead {
                continue;
            }
            let ready = match self.eng.coroutines[i].wait {
                Wait::Frame => true,
                Wait::Until(t) => self.eng.time >= t,
                Wait::Done => true,
            };
            if ready {
                self.advance_co(i);
            }
        }
        self.eng.coroutines.retain(|c| !c.dead);
    }

    fn advance_co(&mut self, i: usize) {
        loop {
            let top = match self.eng.coroutines[i].stack.last() {
                Some(t) => t.clone(),
                None => {
                    self.eng.coroutines[i].dead = true;
                    return;
                }
            };
            let ty = match &top {
                Value::Obj(o) => o.ty,
                _ => {
                    self.eng.coroutines[i].dead = true;
                    return;
                }
            };
            let mn = self.find_virtual_by_name(ty, "MoveNext", 1);
            let cur = self.find_virtual_by_name(ty, "get_Current", 1);
            let (mn, cur) = match (mn, cur) {
                (Some(a), Some(b)) => (a, b),
                _ => {
                    self.eng.coroutines[i].dead = true;
                    return;
                }
            };
            let r = self.invoke(mn, vec![top.clone()]);
            match r {
                Err(e) => {
                    let owner = self.eng.coroutines[i].owner.clone();
                    self.report_exc("coroutine", &owner, &e);
                    self.eng.coroutines[i].dead = true;
                    return;
                }
                Ok(v) => {
                    if !v.as_bool() {
                        self.eng.coroutines[i].stack.pop();
                        if self.eng.coroutines[i].stack.is_empty() {
                            self.eng.coroutines[i].dead = true;
                            return;
                        }
                        continue;
                    }
                    let c = self.invoke(cur, vec![top.clone()]).unwrap_or(Value::Null);
                    // what must be waited for?
                    match &c {
                        Value::Obj(o) => {
                            let secs = if let Ext::Native(b) = &*o.x.borrow() {
                                b.downcast_ref::<Comp>().and_then(|k| if let CK::Wait(s) = k.k { Some(s) } else { None })
                            } else {
                                None
                            };
                            if let Some(s) = secs {
                                self.eng.coroutines[i].wait = Wait::Until(self.eng.time + s as f64);
                                return;
                            }
                            // nested enumerator
                            if self.find_virtual_by_name(o.ty, "MoveNext", 1).is_some() {
                                self.eng.coroutines[i].stack.push(c.clone());
                                continue;
                            }
                            self.eng.coroutines[i].wait = Wait::Frame;
                            return;
                        }
                        _ => {
                            self.eng.coroutines[i].wait = Wait::Frame;
                            return;
                        }
                    }
                }
            }
        }
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
                self.invoke_pcall(&target, &method, mode, aobj, ai, af, &as_, ab);
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
    fn invoke_pcall(&mut self, target: &Value, method: &str, mode: i32, aobj: Value, ai: i32, af: f32, as_: &str, ab: bool) {
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
                self.set_active(target, ab);
            }
            return;
        }
        let ty = match target {
            Value::Obj(o) => o.ty,
            _ => return,
        };
        let name = method.strip_prefix("set_").map(|m| format!("set_{}", m)).unwrap_or(method.to_string());
        let argc = 1 + arg.is_some() as usize;
        if ty < self.n_user {
            if let Some(m) = self.find_method(ty, &name, argc) {
                let mut a = vec![target.clone()];
                if let Some(x) = arg {
                    a.push(x);
                }
                if let Err(e) = self.invoke(m, a) {
                    self.report_exc(method, target, &e);
                }
                return;
            }
        }
        crate::elog!("[unity] unresolved pcall: {}.{} (mode {})", self.type_name_of(target), method, mode);
    }
}

fn tag_name(i: i64) -> String {
    match i {
        0 => "Untagged".into(),
        _ => format!("Tag{}", i),
    }
}

fn flatten_nums(m: &serde_json::Map<String, J>, prefix: &str, out: &mut HashMap<String, f32>) {
    for (k, v) in m {
        let key = format!("{}{}", prefix, k);
        match v {
            J::Number(n) => {
                out.insert(key, n.as_f64().unwrap_or(0.0) as f32);
            }
            J::Bool(b) => {
                out.insert(key, *b as i32 as f32);
            }
            J::Object(o) => flatten_nums(o, &format!("{}.", key), out),
            _ => {}
        }
    }
}

fn unity_kind(full: &str) -> CK {
    CK::Other(full.to_string())
}

/// Text snapshot of the game's visible state (one line per GameObject, active or not): used to compare
/// the interpreter (oracle) and the generated Rust on the same scene after N frames.
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
