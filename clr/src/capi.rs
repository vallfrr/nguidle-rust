//! C API for the Vita backend (vitaGL): init, input, frame, triangle list, textures.
use crate::render::{Renderer, ATLAS_SIZE};
use crate::Vm;
use std::ffi::CStr;
use std::os::raw::c_char;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NguVertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    pub rgba: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NguBatch {
    pub tex: u32,
    pub nearest: u32,
    pub clip: [i32; 4],
    pub first: u32,
    pub count: u32,
}

struct State {
    vm: Vm,
    rend: Renderer,
    verts: Vec<NguVertex>,
    batches: Vec<NguBatch>,
    dev: (f32, f32),
    scale: f32,
    ox: f32,
    oy: f32,
    prev_down: bool,
    frames: u64,
}

static mut STATE: Option<State> = None;

#[allow(static_mut_refs)]
fn st() -> Option<&'static mut State> {
    unsafe { STATE.as_mut() }
}

fn pack(c: [f32; 4]) -> u32 {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    b(c[0]) | b(c[1]) << 8 | b(c[2]) << 16 | b(c[3]) << 24
}

/// data_dir: folder containing ngu.pak (or the files). dev_w/dev_h: screen size in pixels.
#[no_mangle]
pub extern "C" fn ngu_init(data_dir: *const c_char, dev_w: i32, dev_h: i32) -> i32 {
    let dir = unsafe { CStr::from_ptr(data_dir) }.to_string_lossy().to_string();
    crate::assets::set_log(&format!("{}/log.txt", dir));
    crate::assets::init(&dir);
    crate::elog!("[ngu] init dir={} dev={}x{}", dir, dev_w, dev_h);
    let dll = match crate::assets::read("Assembly-CSharp.dll") {
        Some(d) => d,
        None => {
            crate::elog!("[ngu] Assembly-CSharp.dll not found");
            return 1;
        }
    };
    let t0 = std::time::Instant::now();
    let mut vm = Vm::from_bytes(dll);
    crate::elog!("[ngu] VM prete en {:?}", t0.elapsed());
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = format!("{}/save", dir);
    let _ = std::fs::create_dir_all(&vm.eng.save_dir);
    crate::unity_api::load_prefs(&mut vm);
    let t1 = std::time::Instant::now();
    if let Err(e) = vm.load_scene(&dir) {
        crate::elog!("[ngu] load_scene: {}", e);
        return 2;
    }
    crate::elog!("[ngu] scene loaded in {:?} ({} GameObjects)", t1.elapsed(), vm.eng.gos.len());
    let t2 = std::time::Instant::now();
    vm.awake_all();
    crate::elog!("[ngu] Awake en {:?}", t2.elapsed());
    // virtual 960x600 -> screen, keeps proportions, centred
    let scale = (dev_w as f32 / 960.0).min(dev_h as f32 / 600.0);
    let ox = (dev_w as f32 - 960.0 * scale) * 0.5;
    let oy = (dev_h as f32 - 600.0 * scale) * 0.5;
    let rend = Renderer::new(dev_w as usize, dev_h as usize);
    unsafe {
        STATE = Some(State { vm, rend, verts: Vec::with_capacity(1 << 16), batches: vec![], dev: (dev_w as f32, dev_h as f32), scale, ox, oy, prev_down: false, frames: 0 });
    }
    0
}

/// x,y in screen pixels (top-left origin), down = finger down.
#[no_mangle]
pub extern "C" fn ngu_frame(dt: f32, x: f32, y: f32, down: i32) {
    let s = match st() {
        Some(s) => s,
        None => return,
    };
    let (vx, vy_dev) = ((x - s.ox) / s.scale, (y - s.oy) / s.scale);
    let vy = 600.0 - vy_dev; // bottom-left origin (Unity)
    let d = down != 0;
    s.vm.eng.mouse = (vx, vy);
    s.vm.eng.mouse_down_edge = d && !s.prev_down;
    s.vm.eng.mouse_up_edge = !d && s.prev_down;
    s.vm.eng.mouse_down = d;
    s.prev_down = d;
    let t = std::time::Instant::now();
    let steps0 = s.vm.steps;
    s.vm.frame(dt.min(1.0 / 3.0));
    s.frames += 1;
    if s.frames <= 12 || s.frames % 100 == 0 {
        crate::elog!("[ngu] frame {} dt={:.3} logique {:?} instr={}", s.frames, dt, t.elapsed(), s.vm.steps - steps0);
    }
}

/// Builds the frame's triangle list; returns the number of batches.
#[no_mangle]
pub extern "C" fn ngu_emit() -> i32 {
    let s = match st() {
        Some(s) => s,
        None => return 0,
    };
    s.rend.emit(&mut s.vm, s.scale, s.ox, s.oy);
    s.verts.clear();
    s.batches.clear();
    let quads = std::mem::take(&mut s.rend.quads);
    for q in &quads {
        let first = s.verts.len() as u32;
        let c = pack(q.col);
        let v = |i: usize| NguVertex { x: q.p[i][0], y: q.p[i][1], u: q.uv[i][0], v: q.uv[i][1], rgba: c };
        for i in [0usize, 1, 2, 0, 2, 3] {
            s.verts.push(v(i));
        }
        match s.batches.last_mut() {
            Some(b) if b.tex == q.tex && b.nearest == q.nearest as u32 && b.clip == q.clip => b.count += 6,
            _ => s.batches.push(NguBatch { tex: q.tex, nearest: q.nearest as u32, clip: q.clip, first, count: 6 }),
        }
    }
    s.rend.quads = quads;
    s.batches.len() as i32
}

#[no_mangle]
pub extern "C" fn ngu_batches() -> *const NguBatch {
    st().map(|s| s.batches.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn ngu_vertices() -> *const NguVertex {
    st().map(|s| s.verts.as_ptr()).unwrap_or(std::ptr::null())
}

/// RGBA8 pixels of a texture (id >= 2). Returns NULL if unknown.
#[no_mangle]
pub extern "C" fn ngu_tex(id: u32, w: *mut u32, h: *mut u32) -> *const u8 {
    let s = match st() {
        Some(s) => s,
        None => return std::ptr::null(),
    };
    match s.rend.tex_load(id) {
        Some(t) => unsafe {
            *w = t.w as u32;
            *h = t.h as u32;
            // the pointer stays valid: s.rend.textures keeps the Rc
            t.px.as_ptr()
        },
        None => std::ptr::null(),
    }
}

/// Glyph atlas (RGBA8, ATLAS_SIZE^2). `dirty` = 1 if modified since the last call.
#[no_mangle]
pub extern "C" fn ngu_atlas(dirty: *mut i32, size: *mut u32) -> *const u8 {
    let s = match st() {
        Some(s) => s,
        None => return std::ptr::null(),
    };
    unsafe {
        *dirty = s.rend.atlas_take_dirty() as i32;
        *size = ATLAS_SIZE as u32;
    }
    s.rend.atlas_pixels().as_ptr()
}

#[no_mangle]
pub extern "C" fn ngu_texture_count() -> u32 {
    st().map(|s| s.rend.textures.len() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ngu_dev_size(w: *mut f32, h: *mut f32) {
    if let Some(s) = st() {
        unsafe {
            *w = s.dev.0;
            *h = s.dev.1;
        }
    }
}

/// Pending text entry request? Copies the initial text into `buf`.
#[no_mangle]
pub extern "C" fn ngu_text_request(buf: *mut c_char, cap: i32, max: *mut i32, numeric: *mut i32) -> i32 {
    let s = match st() {
        Some(s) => s,
        None => return 0,
    };
    if let Some(r) = s.vm.eng.text_req.as_mut() {
        if r.pending {
            r.pending = false;
            let b = r.text.as_bytes();
            let n = b.len().min(cap as usize - 1);
            unsafe {
                std::ptr::copy_nonoverlapping(b.as_ptr(), buf as *mut u8, n);
                *buf.add(n) = 0;
                *max = r.max;
                *numeric = r.numeric as i32;
            }
            return 1;
        }
    }
    0
}

/// Result of the entry (`ok` = 0 if cancelled).
#[no_mangle]
pub extern "C" fn ngu_text_submit(text: *const c_char, ok: i32) {
    let s = match st() {
        Some(s) => s,
        None => return,
    };
    if let Some(r) = s.vm.eng.text_req.take() {
        if ok != 0 {
            let t = unsafe { CStr::from_ptr(text) }.to_string_lossy().to_string();
            s.vm.input_commit(&r.comp, &t);
        }
    }
}

/// The GPU received the texture: free the RAM copy.
#[no_mangle]
pub extern "C" fn ngu_tex_release(id: u32) {
    if let Some(s) = st() {
        s.rend.tex_release(id);
    }
}
