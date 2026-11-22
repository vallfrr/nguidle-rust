//! Desktop shell: window, mouse, keyboard (and gamepad through the `pad` feature).
//! Game logic runs at real time (fixed 1/30 s step, bounded catch-up) independently of the display, whose rate
//! is capped (30 or 60 frames/s).
//! Controls: arrows / ZQSD(WASD) = move the focus, Enter / Space = confirm, Escape = remove the focus,
//! Tab / A / E = previous / next menu, mouse = direct click. Text entry: keyboard, Enter to finish.
//! These navigation keys are active only after toggling with F1; by default every key goes to the game (original behaviour:
//! shortcuts, Shift/Ctrl modifiers) and the mouse sends left, right and middle clicks and the wheel.
pub mod actions;
pub mod gl;
#[cfg(windows)]
pub mod pad;

use actions::{apply, Action};
use clr::render::Renderer;
use clr::Vm;
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use std::time::{Duration, Instant};

pub const VIRT_W: f32 = 960.0;
pub const VIRT_H: f32 = 600.0;
/// logic step (identical to the oracle and to the test scenarios)
pub const LOGIC_DT: f32 = 1.0 / 30.0;

pub struct Options {
    pub width: usize,
    pub height: usize,
    /// display cap (30 or 60)
    pub fps: u32,
    /// stop after N displayed frames (tests)
    pub max_frames: Option<u64>,
    /// write a PNG capture of the last frame
    pub screenshot: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options { width: 1280, height: 800, fps: 60, max_frames: None, screenshot: None }
    }
}

/// Window -> virtual game space (y up, like `Engine::mouse`); (scale, x offset, y offset) of the layout.
pub(crate) fn layout(w: usize, h: usize) -> (f32, f32, f32) {
    let s = (w as f32 / VIRT_W).min(h as f32 / VIRT_H);
    ((s), (w as f32 - VIRT_W * s) / 2.0, (h as f32 - VIRT_H * s) / 2.0)
}

/// Unity `KeyCode` of a window key (letters, digits, arrows, modifiers and the few others the game reads).
fn unity_key(k: Key) -> Option<i32> {
    let n = k as i32;
    let (a, z, k0, k9) = (Key::A as i32, Key::Z as i32, Key::Key0 as i32, Key::Key9 as i32);
    Some(match k {
        _ if (a..=z).contains(&n) => 97 + (n - a),
        _ if (k0..=k9).contains(&n) => 48 + (n - k0),
        Key::Up => 273,
        Key::Down => 274,
        Key::Right => 275,
        Key::Left => 276,
        Key::LeftShift => 304,
        Key::RightShift => 303,
        Key::LeftCtrl => 306,
        Key::RightCtrl => 305,
        Key::LeftAlt => 308,
        Key::RightAlt => 307,
        Key::Space => 32,
        Key::Enter => 13,
        Key::Escape => 27,
        Key::Tab => 9,
        Key::Backspace => 8,
        _ => return None,
    })
}

fn key_actions(window: &Window) -> Vec<Action> {
    let mut v = vec![];
    for k in window.get_keys_pressed(KeyRepeat::Yes) {
        match k {
            Key::Up | Key::W | Key::Z => v.push(Action::Up),
            Key::Down | Key::S => v.push(Action::Down),
            Key::Left | Key::A | Key::Q => v.push(Action::Left),
            Key::Right | Key::D => v.push(Action::Right),
            Key::Enter | Key::Space => v.push(Action::Activate),
            Key::Escape => v.push(Action::Back),
            Key::Tab | Key::PageDown | Key::E => v.push(Action::Next),
            Key::PageUp => v.push(Action::Prev),
            _ => {}
        }
    }
    v
}

/// Text entry: printable keys, backspace, Enter = end. The field is updated on every keystroke.
struct TextCapture {
    chars: std::sync::Arc<std::sync::Mutex<Vec<u32>>>,
}
struct CharCb(std::sync::Arc<std::sync::Mutex<Vec<u32>>>);
impl minifb::InputCallback for CharCb {
    fn add_char(&mut self, uni_char: u32) {
        self.0.lock().unwrap().push(uni_char);
    }
}

pub fn run(vm: &mut Vm, o: Options) -> Result<(), String> {
    let mut window = Window::new("NGU Idle", o.width, o.height, WindowOptions { resize: true, ..WindowOptions::default() }).map_err(|e| e.to_string())?;
    let tc = TextCapture { chars: Default::default() };
    window.set_input_callback(Box::new(CharCb(tc.chars.clone())));
    window.limit_update_rate(Some(Duration::from_micros(1_000_000 / o.fps.max(1) as u64)));
    vm.eng.screen = (VIRT_W as i32, VIRT_H as i32);
    let (mut w, mut h) = window.get_size();
    let mut rend = Renderer::new(w, h);
    let mut buf = vec![0u32; w * h];
    let mut last = Instant::now();
    let mut acc = 0.0f32;
    let mut prev_btn = [false; 3];
    let mut nav_mode = false; // F1: keyboard drives the focus instead of the game
    let mut pending: Vec<(f32, f32)> = vec![]; // synthetic clicks (confirm at focus): one click = 2 logic steps
    let mut synth_phase = 0u8;
    let mut shown = 0u64;
    let perf = std::env::var("NGU_PERF").is_ok();
    let (mut p_logic, mut p_conv, mut p_present, mut p_emit, mut p_raster, mut p_steps, mut p_t0) = (0f32, 0f32, 0f32, 0f32, 0f32, 0u32, Instant::now());
    let mut text_buf = String::new();
    #[cfg(windows)]
    let mut pad = pad::Pad::new();
    while window.is_open() {
        let (nw, nh) = window.get_size();
        if (nw, nh) != (w, h) && nw > 0 && nh > 0 {
            w = nw;
            h = nh;
            rend = Renderer::new(w, h);
            buf = vec![0u32; w * h];
        }
        let (scale, ox, oy) = layout(w, h);
        // ---- input: mouse
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Clamp) {
            let vx = (mx - ox) / scale;
            let vy = (my - oy) / scale;
            if pending.is_empty() {
                vm.eng.mouse = (vx, VIRT_H - vy);
            }
        }
        let btn = [MouseButton::Left, MouseButton::Right, MouseButton::Middle].map(|b| window.get_mouse_down(b));
        if pending.is_empty() && synth_phase == 0 {
            for i in 0..3 {
                if btn[i] && !prev_btn[i] {
                    vm.eng.mouse_button = i as i32;
                    vm.eng.mouse_down = true;
                    vm.eng.mouse_down_edge = true;
                } else if !btn[i] && prev_btn[i] {
                    vm.eng.mouse_button = i as i32;
                    vm.eng.mouse_down = false;
                    vm.eng.mouse_up_edge = true;
                }
            }
            prev_btn = btn;
        }
        if let Some((_, wy)) = window.get_scroll_wheel() {
            vm.scroll_wheel(wy.clamp(-3.0, 3.0));
        }
        if window.is_key_pressed(Key::F1, KeyRepeat::No) {
            nav_mode = !nav_mode;
            vm.eng.keys_down.clear();
        }
        // ---- input: text entry or navigation actions
        let comp = vm.eng.text_req.as_ref().map(|r| r.comp.clone());
        if let Some(comp) = comp {
            vm.eng.keys_down.clear(); // typing must not trigger game shortcuts
            let typed: Vec<u32> = std::mem::take(&mut *tc.chars.lock().unwrap());
            let mut changed = false;
            for c in typed {
                if c == 8 {
                    text_buf.pop();
                    changed = true;
                } else if c >= 32 && c != 127 {
                    if let Some(ch) = char::from_u32(c) {
                        text_buf.push(ch);
                        changed = true;
                    }
                }
            }
            if let Some(r) = vm.eng.text_req.as_mut() {
                if r.pending {
                    r.pending = false;
                    text_buf = r.text.clone();
                }
            }
            if changed {
                vm.input_commit(&comp, &text_buf);
            }
            if window.is_key_pressed(Key::Enter, KeyRepeat::No) || window.is_key_pressed(Key::Escape, KeyRepeat::No) {
                vm.eng.text_req = None;
                text_buf.clear();
            }
        } else {
            tc.chars.lock().unwrap().clear();
            #[allow(unused_mut)]
            let at_desktop_start = vm.at_desktop_start();
            let mut acts = if nav_mode || at_desktop_start { key_actions(&window) } else { vec![] };
            if at_desktop_start && !window.is_key_pressed(Key::Enter, KeyRepeat::No)
                && !window.is_key_pressed(Key::Space, KeyRepeat::No) {
                acts.retain(|action| *action != Action::Activate);
            }
            if !nav_mode && !at_desktop_start {
                // original behaviour: the game reads the keys itself (GetKey / GetKeyDown)
                vm.eng.keys_down = window.get_keys().into_iter().filter_map(unity_key).collect();
                vm.eng.keys_edge.extend(window.get_keys_pressed(KeyRepeat::No).into_iter().filter_map(unity_key));
            }
            #[cfg(windows)]
            if let Some(p) = pad.as_mut() {
                acts.extend(p.poll(vm.dropdown_is_open(), false));
            }
            for a in acts {
                if let Some(p) = apply(vm, a) {
                    pending.push(p);
                }
            }
        }
        // ---- logic: fixed steps caught up on real time
        let t_logic = Instant::now();
        let now = Instant::now();
        acc += (now - last).as_secs_f32().min(0.25);
        last = now;
        let mut steps = 0;
        while acc >= LOGIC_DT && steps < 6 {
            // synthetic click: press then release over two steps
            if let Some(&(x, y)) = pending.first() {
                if synth_phase == 0 {
                    vm.eng.mouse = (x, y);
                    vm.eng.mouse_down = true;
                    vm.eng.mouse_down_edge = true;
                    synth_phase = 1;
                } else {
                    vm.eng.mouse_down = false;
                    vm.eng.mouse_up_edge = true;
                    synth_phase = 0;
                    pending.remove(0);
                }
            }
            vm.frame(LOGIC_DT);
            acc -= LOGIC_DT;
            steps += 1;
        }
        if steps == 6 {
            acc = 0.0;
        }
        p_logic += t_logic.elapsed().as_secs_f32() * 1000.0;
        p_steps += steps as u32;
        // ---- display
        rend.set_subframe_time(acc);
        rend.render(vm, scale, ox, oy);
        p_emit += rend.t_emit;
        p_raster += rend.t_raster;
        let t = Instant::now();
        for (d, p) in buf.iter_mut().zip(rend.fb.chunks_exact(4)) {
            *d = ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32;
        }
        p_conv += t.elapsed().as_secs_f32() * 1000.0;
        let t = Instant::now();
        window.update_with_buffer(&buf, w, h).map_err(|e| e.to_string())?;
        p_present += t.elapsed().as_secs_f32() * 1000.0;
        if perf && (shown + 1) % 60 == 0 {
            let s = p_t0.elapsed().as_secs_f32();
            eprintln!("[perf] {:.1} img/s | per frame (ms): logic {:.1} ({:.1} steps) emit {:.1} raster {:.1} conv {:.1} present {:.1} | {}x{}", 60.0 / s, p_logic / 60.0, p_steps as f32 / 60.0, p_emit / 60.0, p_raster / 60.0, p_conv / 60.0, p_present / 60.0, w, h);
            (p_logic, p_conv, p_present, p_emit, p_raster, p_steps, p_t0) = (0.0, 0.0, 0.0, 0.0, 0.0, 0, Instant::now());
        }
        shown += 1;
        if let Some(m) = o.max_frames {
            if shown >= m {
                break;
            }
        }
    }
    if let Some(p) = &o.screenshot {
        rend.save_png(p);
    }
    Ok(())
}
