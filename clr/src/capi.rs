//! C API for the Vita backend (vitaGL): init, input, frame, triangle list, textures.
use crate::render::{Renderer, ATLAS_SIZE};
use crate::Vm;
use std::ffi::CStr;
use std::os::raw::c_char;

const GAME_STEP: f64 = 1.0 / 30.0;

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
    vm: Box<Vm>,
    rend: Renderer,
    verts: Vec<NguVertex>,
    batches: Vec<NguBatch>,
    dev: (f32, f32),
    scale: f32,
    ox: f32,
    oy: f32,
    touch_down: bool,
    pad_focus: bool,
    pad_click: Option<((f32, f32), bool)>,
    accumulator: f64,
    was_at_title: bool,
    benchmark: bool,
    frames: u64,
}

static mut STATE: Option<State> = None;

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    #[ignore = "requires prepared assets and a late-game save"]
    fn vita_autosave_projects_small_menus_to_the_display() {
        let assets = std::env::var("NGU_TEST_VITA_ASSETS").unwrap();
        let profile = std::env::var("NGU_VITA_SAVE_DIR").unwrap();
        let save = std::env::var("NGU_TEST_SAVE").unwrap();
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::copy(save, std::path::Path::new(&profile).join("NGUSaveSteam.txt")).unwrap();
        assert_eq!(ngu_init(std::ffi::CString::new(assets).unwrap().as_ptr(), 960, 544), 0);
        let click = |x, y| {
            ngu_frame(1.0 / 60.0, x, y, 1);
            ngu_frame(1.0 / 60.0, x, y, 0);
        };
        click(480.0, 368.0);
        for _ in 0..90 {
            ngu_frame(1.0 / 60.0, -1.0, -1.0, 0);
        }
        click(300.0, 508.0);
        for (name, y, menu) in [
            ("Basic Training", 18.0, 0),
            ("Augmentation", 143.0, 5),
            ("Advanced Training", 167.0, 17),
            ("Time Machine", 190.0, 18),
            ("Blood Magic", 215.0, 6),
            ("Wandoos", 239.0, 14),
            ("NGU", 264.0, 8),
        ] {
            click(210.0, y);
            ngu_frame(1.0 / 60.0, -1.0, -1.0, 0);
            ngu_emit();
            let s = st().unwrap();
            let field = crate::vm::field_ref(&s.vm, "MenuSwapper", "menuID").unwrap();
            let active = s.vm.eng.user_comps.iter().find_map(|component| crate::vm::read_field(component, field)).unwrap();
            assert_eq!(active.as_i32(), menu, "{name}: unexpected active menu");
            s.rend.fb.resize(960 * 544 * 4, 0);
            s.rend.raster();
            let filled = (350..900).filter(|&x| {
                let i = (535 * 960 + x) * 4;
                s.rend.fb[i..i + 3] != [0, 0, 0]
            }).count();
            assert!(filled > 500, "{name}: the right panel must reach the bottom of the display ({filled} non-black pixels)");
        }
    }
}

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
    init_vita(data_dir, dev_w, dev_h, |_| {})
}

/// Initialize the Vita host, optionally attaching translated game methods.
pub fn init_vita(data_dir: *const c_char, dev_w: i32, dev_h: i32, configure: fn(&mut Vm)) -> i32 {
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
    let mut vm = Box::new(Vm::from_bytes(dll));
    crate::elog!("[ngu] VM prete en {:?}", t0.elapsed());
    vm.eng.screen = (960, 600);
    vm.eng.desktop_panels = true;
    vm.eng.cache_focus_controls = true;
    let benchmark_dir = std::env::var("NGU_VITA_SAVE_DIR");
    let benchmark = benchmark_dir.is_ok();
    vm.eng.save_dir = benchmark_dir.unwrap_or_else(|_| format!("{dir}/save"));
    let _ = std::fs::create_dir_all(&vm.eng.save_dir);
    crate::unity_api::load_prefs(&mut vm);
    configure(&mut vm);
    let t1 = std::time::Instant::now();
    if let Err(e) = vm.load_scene(&dir) {
        crate::elog!("[ngu] load_scene: {}", e);
        return 2;
    }
    crate::elog!("[ngu] scene loaded in {:?} ({} GameObjects)", t1.elapsed(), vm.eng.gos.len());
    #[cfg(target_os = "vita")]
    let fonts = crate::render::prepare_startup_fonts();
    let t2 = std::time::Instant::now();
    vm.awake_all();
    crate::elog!("[ngu] Awake en {:?}", t2.elapsed());
    vm.enable_visual_motion();
    if let Err(error) = vm.start_desktop_save_picker() {
        crate::elog!("[ngu] save picker: {error}");
        return 3;
    }
    crate::ui::cache_menu_canvases(&mut vm);
    // Keep the native proportions and reflow the desktop UI to use the full Vita screen.
    let scale = (dev_w as f32 / 960.0).min(dev_h as f32 / 600.0);
    let ox = 0.0;
    let oy = (dev_h as f32 - 600.0 * scale) * 0.5;
    vm.eng.desktop_layout.set_width(dev_w as f32 / scale);
    let mut rend = Renderer::new(0, 0);
    #[cfg(target_os = "vita")]
    if let Some(fonts) = fonts.and_then(|worker| worker.join().ok()) {
        rend.install_fonts(fonts);
    }
    rend.resize(dev_w as usize, dev_h as usize, false);
    rend.prepare_layout(&mut vm);
    unsafe {
        STATE = Some(State { vm, rend, verts: Vec::with_capacity(1 << 16), batches: vec![], dev: (dev_w as f32, dev_h as f32), scale, ox, oy, touch_down: false, pad_focus: false, pad_click: None, accumulator: 0.0, was_at_title: true, benchmark, frames: 0 });
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
    let touch = down != 0;
    if let Some((at, press)) = s.pad_click.take() {
        s.vm.eng.mouse = at;
        s.vm.eng.mouse_down = press;
        s.vm.eng.mouse_down_edge = press;
        s.vm.eng.mouse_up_edge = !press;
        if press { s.pad_click = Some((at, false)); }
    } else if touch || s.touch_down {
        if s.pad_focus { crate::focus::clear(&mut s.vm); }
        s.pad_focus = false;
        s.vm.eng.mouse = (vx, vy);
        s.vm.eng.mouse_down = touch;
        s.vm.eng.mouse_down_edge = touch && !s.touch_down;
        s.vm.eng.mouse_up_edge = !touch && s.touch_down;
        s.touch_down = touch;
    } else {
        s.vm.eng.mouse = if s.pad_focus {
            crate::focus::pointer(&s.vm).unwrap_or((-1.0, -1.0))
        } else {
            (-1.0, -1.0)
        };
    }
    s.vm.eng.hover_always = s.pad_focus || touch;
    s.vm.eng.pad_hover = s.pad_focus && !touch;
    let profile = s.benchmark.then(|| (std::time::Instant::now(), s.vm.steps));
    let at_title = s.vm.at_desktop_start();
    if at_title {
        s.accumulator = 0.0;
        s.vm.frame(GAME_STEP as f32);
    } else {
        // Bound recovery work so a slow frame does not monopolize input and presentation.
        if !s.was_at_title && dt.is_finite() {
            s.accumulator += f64::from(dt.max(0.0));
        }
        let mut steps = 0;
        while s.accumulator >= GAME_STEP && steps < 2 {
            s.accumulator -= GAME_STEP;
            s.vm.frame(GAME_STEP as f32);
            steps += 1;
        }
        if steps == 0 && (s.vm.eng.mouse_down_edge || s.vm.eng.mouse_up_edge || s.vm.eng.ptr.last_pos != s.vm.eng.mouse) {
            s.vm.pointer_update();
            s.vm.eng.mouse_down_edge = false;
            s.vm.eng.mouse_up_edge = false;
        }
    }
    s.was_at_title = at_title;
    if let Some((start, steps)) = profile {
        s.frames += 1;
        if s.frames <= 12 || s.frames % 100 == 0 {
            crate::elog!("[ngu] frame {} dt={:.3} logic={:?} instr={}", s.frames, dt, start.elapsed(), s.vm.steps - steps);
        }
    }
}

/// Shared focus, dropdown and menu navigation. The host sends ASCII actions.
#[no_mangle]
pub extern "C" fn ngu_pad(action: u32) -> i32 {
    let Some(s) = st() else { return 0 };
    if s.touch_down || s.pad_click.is_some() || s.vm.eng.text_req.is_some() { return 0; }
    let Some(action) = char::from_u32(action) else { return 0 };
    s.pad_focus = true;
    if s.vm.dropdown_navigate(action) { return 0; }
    let click = match action {
        'a' => {
            let at = crate::focus::activation_point(&mut s.vm);
            if at.is_none() {
                crate::focus::navigate(&mut s.vm, 'd');
                crate::focus::activation_point(&mut s.vm)
            } else { at }
        }
        'b' => {
            if !s.vm.back_from_desktop_picker() { crate::focus::clear(&mut s.vm); }
            None
        }
        'p' => crate::focus::menu_step(&mut s.vm, -1),
        'n' => crate::focus::menu_step(&mut s.vm, 1),
        'u' | 'd' | 'l' | 'r' => {
            let before = s.vm.eng.focus.clone();
            crate::focus::navigate(&mut s.vm, action);
            return i32::from(!crate::unity::same(&before, &s.vm.eng.focus));
        }
        _ => None,
    };
    if let Some(at) = click { s.pad_click = Some((at, true)); }
    0
}

#[no_mangle]
pub extern "C" fn ngu_scroll(dx: f32, dy: f32) {
    let Some(s) = st() else { return };
    if !s.touch_down && s.vm.eng.text_req.is_none() {
        s.pad_focus = true;
        crate::focus::scroll(&mut s.vm, dx, dy);
    }
}

/// Builds the frame's triangle list; returns the number of batches.
#[no_mangle]
pub extern "C" fn ngu_emit() -> i32 {
    let s = match st() {
        Some(s) => s,
        None => return 0,
    };
    s.rend.set_subframe_time(s.accumulator.min(GAME_STEP) as f32);
    s.rend.emit_frame(&mut s.vm, s.scale, s.ox, s.oy);
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

#[cfg(target_os = "vita")]
#[no_mangle]
pub extern "C" fn ngu_layout_us() -> u32 {
    st().map_or(0, |s| s.rend.layout_us)
}

#[no_mangle]
pub extern "C" fn ngu_clock_us(debt_us: *mut u64) -> u64 {
    let Some(s) = st() else { return 0 };
    unsafe { *debt_us = (s.accumulator * 1_000_000.0) as u64; }
    (s.vm.eng.time * 1_000_000.0) as u64
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
