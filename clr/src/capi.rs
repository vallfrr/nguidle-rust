//! The engine's C ABI: what a native host (the Vita, or any C or C++ program) calls. `engine_api.h` in a port declares
//! the same functions.
//!
//! **Threading.** Call `ngu_init` once, then every `ngu_*` function, from that one thread, one call at a time and never
//! from inside the engine's own callbacks. All state is process-wide, so there is one engine per process. A host that
//! presents on another thread copies the draw list and texture pixels first (they are borrowed).
//!
//! **Units.** Time in seconds (`ngu_frame`) or microseconds (`ngu_clock_us`); positions in device pixels, top-left
//! origin; game coordinates are 960 x 600 and the engine maps them. **Memory.** Pointers returned by the engine are
//! borrowed and valid until the next call; pointers passed in belong to the caller. Strings are NUL-terminated UTF-8.
//!
//! The same service for Rust hosts, without globals, is [`crate::session::Session`].
#![deny(missing_docs)]
use crate::render::{Renderer, ATLAS_SIZE};
use crate::Vm;
use std::ffi::CStr;
use std::os::raw::c_char;

const GAME_STEP: f64 = 1.0 / 30.0;

use platform_api::{DrawTarget, TriangleList};
pub use platform_api::{Batch as NguBatch, Vertex as NguVertex};

struct State {
    vm: Box<Vm>,
    rend: Renderer,
    draw: TriangleList,
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
    /// scripted player (`ngu_autoplay_load`), simulation steps run so far and what the last frame did
    autoplay: Option<crate::autoplay::Autoplay>,
    ticks: u64,
    last_steps: u32,
    last_instr: u64,
    /// time budget (ms) of simulation per display frame; 0 = a step always runs in one piece
    slice_ms: f32,
    frame_target_ms: f32,
    no_skip: bool,
    /// moving average (ms) of the layout + draw-list time of recent frames, to leave room for them
    render_ms: f32,
    /// moving average (ms) of what one simulation step costs, and the cost of the step in progress
    tick_ms: f32,
    cur_tick_ms: f32,
    /// the scene or the input changed since the last draw list: with slicing, unchanged frames re-present the previous list
    need_emit: bool,
    /// steps completed early because input arrived (counted in the next frame's trace)
    extra_steps: u32,
    /// number of draw lists built so far (the host publishes a list to its render thread only when this changes)
    emit_seq: u32,
}

static mut STATE: Option<State> = None;

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    #[ignore = "requires prepared assets and a late-game save"]
    fn autosave_projects_small_menus_to_the_display() {
        let assets = std::env::var("NGU_TEST_PREPARED_ASSETS").unwrap();
        let profile = std::env::var("NGU_SAVE_DIR").unwrap();
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


/// Load the game and open its home screen. Call once, first, on the thread that will make every other call.
///
/// * `data_dir`: NUL-terminated path of the folder prepared with `tools/nguprep.py` (or holding `ngu.pak`). It must
///   hold `Assembly-CSharp.dll`. Saves go to `<data_dir>/save` unless `NGU_SAVE_DIR` names another folder.
/// * `dev_w`, `dev_h`: output size in device pixels; the game keeps its 960 x 600 proportions, centred, and reflows
///   to extra width.
///
/// Returns `0` on success, `1` if `Assembly-CSharp.dll` is missing, `2` if the scene cannot be loaded (data prepared
/// by another engine revision), `3` if the save menu cannot start. The reason is also written to `<data_dir>/log.txt`.
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
    let mut vm = Box::new(Vm::from_bytes(dll));
    crate::elog!("[ngu] VM ready in {:?}", t0.elapsed());
    vm.eng.screen = (960, 600);
    vm.eng.desktop_panels = true;
    vm.eng.cache_focus_controls = true;
    let benchmark_dir = std::env::var("NGU_SAVE_DIR");
    let benchmark = benchmark_dir.is_ok();
    vm.eng.save_dir = benchmark_dir.unwrap_or_else(|_| format!("{dir}/save"));
    let _ = std::fs::create_dir_all(&vm.eng.save_dir);
    crate::unity_api::load_prefs(&mut vm);
    let t1 = std::time::Instant::now();
    if let Err(e) = vm.load_scene(&dir) {
        crate::elog!("[ngu] load_scene: {}", e);
        return 2;
    }
    crate::elog!("[ngu] scene loaded in {:?} ({} GameObjects)", t1.elapsed(), vm.eng.gos.len());
    let fonts = crate::host_platform::host_tuning().startup_fonts_worker.then(crate::render::prepare_startup_fonts).flatten();
    let t2 = std::time::Instant::now();
    vm.awake_all();
    crate::elog!("[ngu] Awake in {:?}", t2.elapsed());
    vm.enable_visual_motion();
    if let Err(error) = vm.start_desktop_save_picker() {
        crate::elog!("[ngu] save picker: {error}");
        return 3;
    }
    crate::ui::cache_menu_canvases(&mut vm);
    // Keep the native proportions and reflow the desktop UI to use the whole screen.
    let scale = (dev_w as f32 / 960.0).min(dev_h as f32 / 600.0);
    let ox = 0.0;
    let oy = (dev_h as f32 - 600.0 * scale) * 0.5;
    vm.eng.desktop_layout.set_width(dev_w as f32 / scale);
    let mut rend = Renderer::new(0, 0);
    if let Some(fonts) = fonts.and_then(|worker| worker.join().ok()) {
        rend.install_fonts(fonts);
    }
    rend.resize(dev_w as usize, dev_h as usize, false);
    rend.prepare_layout(&mut vm);
    unsafe {
        STATE = Some(State {
            vm, rend, draw: TriangleList { vertices: Vec::with_capacity(1 << 16), batches: vec![] }, scale, ox, oy,
            touch_down: false, pad_focus: false, pad_click: None, accumulator: 0.0, was_at_title: true,
            benchmark, frames: 0, autoplay: None, ticks: 0, last_steps: 0, last_instr: 0,
            slice_ms: timing_ms("NGU_SLICE_MS", 0.0),
            frame_target_ms: timing_ms("NGU_FRAME_MS", 12.0),
            no_skip: std::env::var_os("NGU_NO_SKIP").is_some(),
            render_ms: 6.0, tick_ms: 8.0, cur_tick_ms: 0.0, need_emit: true, extra_steps: 0, emit_seq: 0,
        });
    }
    0
}

/// Advance the game and apply the pointer, once per display frame.
///
/// * `dt`: seconds since the previous call; logic runs in whole 1/30 s steps and the remainder carries over.
/// * `x`, `y`: pointer or finger in device pixels, top-left origin; negative when there is none.
/// * `down`: non-zero while the primary button or a finger is down. A change between calls is a press or a release.
///
/// Does nothing before a successful [`ngu_init`].
#[no_mangle]
pub extern "C" fn ngu_frame(dt: f32, x: f32, y: f32, down: i32) {
    let s = match st() {
        Some(s) => s,
        None => return,
    };
    let (x, y, down) = autoplay_input(s, x, y, down);
    let touch_now = down != 0;
    let instr0 = s.vm.steps;
    let (vx, vy_dev) = ((x - s.ox) / s.scale, (y - s.oy) / s.scale);
    let vy = 600.0 - vy_dev; // bottom-left origin (Unity)
    let touch = down != 0;
    // Discrete input (a press, a release, a pad click) is applied between steps only, so the game state never depends on
    // where a time-sliced step was cut: an unfinished step is completed first.
    if s.pad_click.is_some() || touch != s.touch_down {
        finish_tick(s);
    }
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
    s.last_steps = 0;
    if at_title {
        s.accumulator = 0.0;
        s.vm.frame(GAME_STEP as f32);
        s.last_steps = 1;
    } else {
        // Bound recovery work so a slow frame does not monopolize input and presentation.
        if !s.was_at_title && dt.is_finite() {
            s.accumulator += f64::from(dt.max(0.0));
        }
        let mut steps = 0;
        if s.slice_ms > 0.0 {
            // Time-sliced steps: a step may span several display frames, so a heavy step never delays a presentation.
            // The scene is only drawn again when a step completes or the input changed, so frames alternate between "simulate"
            // (cheap to present) and "complete the step and draw". The share of the step done before the last frame is what
            // remains after reserving that frame's layout, draw list and submission.
            let finish_ms = (s.frame_target_ms - s.render_ms - 2.0).max(2.5);
            let rem = if s.vm.eng.slice.is_some() { (s.tick_ms - s.cur_tick_ms).max(0.5) } else { s.tick_ms };
            let budget = if rem <= finish_ms { finish_ms } else { (rem - finish_ms).max(2.0) }.min(s.slice_ms);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f32(budget / 1000.0);
            loop {
                let t = std::time::Instant::now();
                if s.vm.eng.slice.is_none() {
                    if !(s.accumulator >= GAME_STEP && steps < 2) {
                        break;
                    }
                    s.accumulator -= GAME_STEP;
                    s.cur_tick_ms = 0.0;
                    if !s.vm.frame_start(GAME_STEP as f32) {
                        steps += 1;
                        continue;
                    }
                }
                let done = s.vm.frame_run(Some(deadline));
                s.cur_tick_ms += t.elapsed().as_secs_f32() * 1000.0;
                if done {
                    steps += 1;
                    s.tick_ms = s.tick_ms * 0.8 + s.cur_tick_ms * 0.2;
                }
                if std::time::Instant::now() >= deadline {
                    break;
                }
            }
        } else {
            while s.accumulator >= GAME_STEP && steps < 2 {
                s.accumulator -= GAME_STEP;
                s.vm.frame(GAME_STEP as f32);
                steps += 1;
            }
        }
        s.last_steps = steps;
        if steps == 0 && s.vm.eng.slice.is_none() && (s.vm.eng.mouse_down_edge || s.vm.eng.mouse_up_edge || s.vm.eng.ptr.last_pos != s.vm.eng.mouse) {
            s.vm.pointer_update();
            s.vm.eng.mouse_down_edge = false;
            s.vm.eng.mouse_up_edge = false;
        }
    }
    s.was_at_title = at_title;
    s.ticks += u64::from(s.last_steps);
    s.last_steps += std::mem::take(&mut s.extra_steps);
    if s.slice_ms <= 0.0 || s.last_steps > 0 || touch_now || s.touch_down || s.pad_click.is_some() || s.vm.eng.text_req.is_some() {
        s.need_emit = true;
    }
    s.last_instr = s.vm.steps - instr0;
    if let Some((start, steps)) = profile {
        s.frames += 1;
        if s.frames <= 12 || s.frames % 100 == 0 {
            crate::elog!("[ngu] frame {} dt={:.3} logic={:?} instr={}", s.frames, dt, start.elapsed(), s.vm.steps - steps);
        }
    }
}

/// Apply a navigation command from a pad or keyboard. `action` is one ASCII code: `u` up, `d` down, `l` left, `r` right,
/// `a` activate (press the focused control), `b` back, `p` previous menu, `n` next menu (`platform_api::Action`).
///
/// Returns `1` when the focus moved, `0` otherwise (including when a finger is down or a text field is open).
#[no_mangle]
pub extern "C" fn ngu_pad(action: u32) -> i32 {
    let Some(s) = st() else { return 0 };
    pad_action(s, action)
}

fn pad_action(s: &mut State, action: u32) -> i32 {
    finish_tick(s);
    s.need_emit = true;
    if s.touch_down || s.pad_click.is_some() || s.vm.eng.text_req.is_some() { return 0; }
    let Some(action) = char::from_u32(action) else { return 0 };
    s.pad_focus = true;
    let (click, moved) = crate::focus::pad_action(&mut s.vm, action, true);
    if let Some(at) = click { s.pad_click = Some((at, true)); }
    i32::from(moved)
}

/// Scroll the focused list by `dx`, `dy` game pixels (positive `dy` scrolls down). Ignored while a finger is down or a
/// text field is open.
#[no_mangle]
pub extern "C" fn ngu_scroll(dx: f32, dy: f32) {
    let Some(s) = st() else { return };
    scroll_action(s, dx, dy);
}

fn scroll_action(s: &mut State, dx: f32, dy: f32) {
    finish_tick(s);
    s.need_emit = true;
    if !s.touch_down && s.vm.eng.text_req.is_none() {
        s.pad_focus = true;
        crate::focus::scroll(&mut s.vm, dx, dy);
    }
}

/// Build the frame's triangle list and return its number of batches. Read it with [`ngu_batches`] and
/// [`ngu_vertices`] before the next call to any `ngu_*` function that can rebuild it; copy it first if another
/// thread will draw it. When nothing changed since the last call the previous list is kept (see [`ngu_emit_seq`]).
#[no_mangle]
pub extern "C" fn ngu_emit() -> i32 {
    let s = match st() {
        Some(s) => s,
        None => return 0,
    };
    if !s.need_emit && !s.draw.batches.is_empty() && !s.no_skip {
        return s.draw.batches.len() as i32; // nothing changed: the host draws the previous list again
    }
    s.need_emit = false;
    s.rend.defer_layout = s.vm.eng.slice.is_some();
    s.rend.set_subframe_time(s.accumulator.min(GAME_STEP) as f32);
    s.rend.emit_frame(&mut s.vm, s.scale, s.ox, s.oy);
    s.render_ms = s.render_ms * 0.9 + s.rend.t_emit * 0.1;
    s.draw.draw_quads(&s.rend.quads);
    s.emit_seq = s.emit_seq.wrapping_add(1);
    s.draw.batches.len() as i32
}

/// Number of draw lists built so far. A host that draws on another thread publishes a list only when this changes.
#[no_mangle]
pub extern "C" fn ngu_emit_seq() -> u32 {
    st().map_or(0, |s| s.emit_seq)
}

/// The batches of the last [`ngu_emit`] (`NguBatch`, 32 bytes each), valid until the next engine call. NULL before init.
#[no_mangle]
pub extern "C" fn ngu_batches() -> *const NguBatch {
    st().map(|s| s.draw.batches.as_ptr()).unwrap_or(std::ptr::null())
}

/// The vertices of the last [`ngu_emit`] (`NguVertex`, 20 bytes each, six per quad), valid until the next engine call.
/// NULL before init.
#[no_mangle]
pub extern "C" fn ngu_vertices() -> *const NguVertex {
    st().map(|s| s.draw.vertices.as_ptr()).unwrap_or(std::ptr::null())
}

/// Choose the host tuning, a bit set of `HostTuning::*` values: 1 background autosave, 2 single-writer storage (no hard
/// links), 4 startup fonts on a worker thread, 8 time the layout. Call it before [`ngu_init`]; it applies to the whole
/// process. A host that never calls it gets the defaults (all off).
#[no_mangle]
pub extern "C" fn ngu_set_host_tuning(bits: u32) {
    crate::host_platform::set_host_tuning(crate::host_platform::HostTuning::from_bits(bits));
}

/// Microseconds the last [`ngu_emit`] spent laying out the scene, for the host's trace. 0 unless the host asked for
/// `HostTuning::TIME_LAYOUT` with [`ngu_set_host_tuning`].
#[no_mangle]
pub extern "C" fn ngu_layout_us() -> u32 {
    st().map_or(0, |s| s.rend.layout_us)
}

/// Game time in microseconds. `debt_us` receives the real time already received but not yet simulated (less than one
/// step). Both are for the host's timing trace.
#[no_mangle]
pub extern "C" fn ngu_clock_us(debt_us: *mut u64) -> u64 {
    let Some(s) = st() else { return 0 };
    unsafe { *debt_us = (s.accumulator * 1_000_000.0) as u64; }
    (s.vm.eng.time * 1_000_000.0) as u64
}

/// RGBA8 pixels of texture `id` (an id above 1, as in `NguBatch::tex`), row by row from the top. `w` and `h` receive its
/// size. Returns NULL if the id is unknown. The pixels are borrowed until the next engine call; upload them, then call
/// [`ngu_tex_release`].
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

/// The glyph atlas (texture id 1): RGBA8, `ATLAS_SIZE` x `ATLAS_SIZE` pixels, size written to `size`. `dirty` receives
/// `1` if its pixels changed since the previous call (upload it again), `0` otherwise. The pixels belong to the renderer
/// and may change at the next frame.
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

/// Poll for a text field the game wants edited. Returns `1` once per request and then copies the field's current UTF-8
/// content into `buf` (at most `cap - 1` bytes plus a NUL), `max` receives the maximum length (0 = none) and `numeric`
/// is `1` for a number-only field. Show a keyboard, then answer with [`ngu_text_submit`]. Returns `0` when there is no
/// new request.
#[no_mangle]
pub extern "C" fn ngu_text_request(buf: *mut c_char, cap: i32, max: *mut i32, numeric: *mut i32) -> i32 {
    let s = match st() {
        Some(s) => s,
        None => return 0,
    };
    if s.autoplay.is_some() {
        // a scripted session never opens the system keyboard (it would block the run): the entry is cancelled
        s.vm.eng.text_req = None;
        return 0;
    }
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

/// Answer a text request: `ok` non-zero commits the NUL-terminated UTF-8 `text`, zero cancels (`text` may then be
/// NULL).
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

/// Tell the engine that texture `id` has been uploaded to the GPU, so it can free its RAM copy.
#[no_mangle]
pub extern "C" fn ngu_tex_release(id: u32) {
    if let Some(s) = st() {
        s.rend.tex_release(id);
    }
}

/// Runs the scripted player for this frame (if any) and returns the touch the engine must see.
fn autoplay_input(s: &mut State, x: f32, y: f32, down: i32) -> (f32, f32, i32) {
    use crate::autoplay::Act;
    let Some(mut ap) = s.autoplay.take() else { return (x, y, down) };
    let mut touch = None;
    if let Some((hx, hy, n)) = ap.hold.take() {
        touch = Some((hx, hy, 1));
        if n > 1 {
            ap.hold = Some((hx, hy, n - 1));
        }
    } else if !ap.done {
        match ap.poll(s.ticks) {
            Some(Act::Pad(c)) => {
                pad_action(s, c as u32);
            }
            Some(Act::Touch(tx, ty)) => {
                ap.hold = Some((tx, ty, 2));
                touch = Some((tx, ty, 1));
            }
            Some(Act::Scroll(dx, dy)) => scroll_action(s, dx, dy),
            Some(Act::Menu(n)) => {
                let t = std::time::Instant::now();
                let i0 = s.vm.steps;
                let before = s.vm.prof.clone();
                crate::autoplay::swap_menu(&mut s.vm, n);
                if let (Some(b), Some(a)) = (before, s.vm.prof.as_ref()) {
                    // CLR_PROF: what this swap executed (top exclusive instructions), to find what makes menu switches slow
                    let mut d: Vec<_> = a.iter().map(|(m, v)| (v.2 - b.get(m).map_or(0, |x| x.2), v.0 - b.get(m).map_or(0, |x| x.0), *m)).filter(|x| x.0 > 0).collect();
                    d.sort_by(|x, y| y.0.cmp(&x.0));
                    for (e, c, m) in d.into_iter().take(4) {
                        let mi = &s.vm.prog.methods[m as usize];
                        crate::elog!("[autoplay]   swap hot: {e} instr, {c} calls  {}::{}", s.vm.types[mi.owner as usize].name, mi.name);
                    }
                }
                crate::elog!("[autoplay] menu {n} swap {:?} instr {}", t.elapsed(), s.vm.steps - i0);
            }
            Some(Act::Click(text)) => {
                if let Some((cx, cy, _)) = crate::dev::find_click(&mut s.vm, &text) {
                    s.pad_focus = true;
                    s.pad_click = Some(((cx, cy), true));
                } else {
                    crate::elog!("[autoplay] click: no control '{text}' at tick {}", s.ticks);
                }
            }
            Some(Act::Mark(name)) => {
                crate::elog!("[autoplay] mark {} {name} tick={}", ap.marks.len(), s.ticks);
                ap.marks.push(name);
            }
            Some(Act::Check(name)) => {
                let h = crate::autoplay::save_text(&mut s.vm).map(|t| crate::autoplay::digest(&t)).unwrap_or(0);
                crate::elog!("[autoplay] check {name} {h:016x} tick={}", s.ticks);
                ap.digests.push((name, h));
            }
            Some(Act::Shot) => ap.shots += 1,
            Some(Act::Ui) => crate::elog!("[autoplay] ui tick={}
{}", s.ticks, crate::dev::ui(&mut s.vm)),
            Some(Act::End) => crate::elog!("[autoplay] end tick={} actions={}", s.ticks, ap.action_id),
            None => {}
        }
    }
    s.autoplay = Some(ap);
    match touch {
        Some((tx, ty, d)) => (tx, ty, d),
        None => (x, y, down),
    }
}

/// Load a scripted-player scenario (`clr/src/autoplay.rs`) for unattended measurements. `path` is a NUL-terminated file
/// path. Returns `0` on success.
#[no_mangle]
pub extern "C" fn ngu_autoplay_load(path: *const c_char) -> i32 {
    let (Some(s), path) = (st(), unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string()) else { return 1 };
    match crate::autoplay::Autoplay::load(&path) {
        Ok(a) => {
            crate::elog!("[autoplay] scenario {path} loaded");
            s.autoplay = Some(a);
            0
        }
        Err(e) => {
            crate::elog!("[autoplay] {e}");
            2
        }
    }
}

/// What the last [`ngu_frame`] did, for the host's per-frame trace. `out` must point to eight `u32`: ticks, simulation
/// steps, instructions (saturated), menu id, last scripted action number, number of marks, finished flag, screenshots
/// taken by the scenario.
#[no_mangle]
pub extern "C" fn ngu_frame_info(out: *mut u32) {
    let Some(s) = st() else { return };
    let menu = crate::vm::field_ref(&s.vm, "MenuSwapper", "menuID")
        .and_then(|f| s.vm.eng.user_comps.iter().find_map(|c| crate::vm::read_field(c, f)))
        .map_or(-1, |v| v.as_i32());
    let (action, marks, done) = s.autoplay.as_ref().map_or((0, 0, 0), |a| (a.action_id, a.marks.len() as u32, u32::from(a.done)));
    let shots = s.autoplay.as_ref().map_or(0, |a| a.shots);
    let v = [s.ticks as u32, s.last_steps, s.last_instr.min(u64::from(u32::MAX)) as u32, menu as u32, action, marks, done, shots];
    unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), out, v.len()) };
}

/// `CLR_PROF` report: the `top` methods by exclusive interpreted instructions, one per line:
/// `exclusive inclusive calls Type::Method`. Used by `tools/ngu-profile.sh` to choose what to translate to Rust.
pub fn profile_report(top: usize) -> String {
    let Some(s) = st() else { return String::new() };
    let Some(p) = s.vm.prof.as_ref() else { return "set CLR_PROF=1
".into() };
    let mut v: Vec<_> = p.iter().collect();
    v.sort_by(|a, b| b.1 .2.cmp(&a.1 .2));
    let total: u64 = p.values().map(|e| e.2).sum();
    let mut out = format!("total {total} instructions in {} methods
", p.len());
    for (m, (calls, incl, excl)) in v.into_iter().take(top) {
        let mi = &s.vm.prog.methods[*m as usize];
        out += &format!("{excl:>10} {incl:>10} {calls:>8}  {}::{}
", s.vm.types[mi.owner as usize].name, mi.name);
    }
    out
}

/// Optional host experiments read their timing options once, during initialization.
fn timing_ms(name: &str, default: f32) -> f32 {
    std::env::var(name).ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| (0.0..=1000.0).contains(value))
        .unwrap_or(default)
}

/// Completes the time-sliced step in progress (no deadline).
fn finish_tick(s: &mut State) {
    if s.vm.eng.slice.is_none() {
        return;
    }
    let t = std::time::Instant::now();
    s.vm.frame_run(None);
    s.cur_tick_ms += t.elapsed().as_secs_f32() * 1000.0;
    s.tick_ms = s.tick_ms * 0.8 + s.cur_tick_ms * 0.2;
    s.ticks += 1;
    s.extra_steps += 1;
    s.need_emit = true;
}
