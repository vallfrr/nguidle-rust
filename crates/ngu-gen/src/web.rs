//! Browser transport for the same native VM used by the desktop window.
use ngu_desktop::{actions::{self, Action}, LOGIC_DT, VIRT_H, VIRT_W};
use ngu_host::clr::{self, render::Renderer, unity_api, Vm};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(untagged)]
enum Request {
    Frame { width: usize, height: usize, events: Vec<Event> },
    Import { import: Upload },
}

#[derive(Deserialize)]
struct Upload {
    name: String,
    slot: usize,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Move { x: f32, y: f32 },
    Button { x: f32, y: f32, button: i32, down: bool },
    Wheel { x: f32, y: f32, delta: f32 },
    Key { key: String, right: bool, down: bool, repeat: bool },
    Navigate { action: Navigation },
    Scroll { x: f32, y: f32 },
    Text { value: Option<String> },
    Reset,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Navigation { Up, Down, Left, Right, Activate, Back, Prev, Next }

impl From<Navigation> for Action {
    fn from(value: Navigation) -> Self {
        match value {
            Navigation::Up => Self::Up, Navigation::Down => Self::Down,
            Navigation::Left => Self::Left, Navigation::Right => Self::Right,
            Navigation::Activate => Self::Activate, Navigation::Back => Self::Back,
            Navigation::Prev => Self::Prev, Navigation::Next => Self::Next,
        }
    }
}

struct View {
    width: usize,
    height: usize,
    scale: f32,
    top: f32,
}

impl View {
    fn new(width: usize, height: usize) -> Self {
        let scale = (width as f32 / VIRT_W).min(height as f32 / VIRT_H);
        let top = (height as f32 - VIRT_H * scale) / 2.0;
        Self { width, height, scale, top }
    }

    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        let py = (y * self.height as f32 - self.top) / self.scale;
        if (0.0..1.0).contains(&x) && (0.0..VIRT_H).contains(&py) {
            (x * self.width as f32 / self.scale, VIRT_H - py)
        } else {
            (-1.0, -1.0)
        }
    }
}

struct Button { point: (f32, f32), kind: i32, down: bool }

#[derive(Default)]
struct Input {
    buttons: VecDeque<Button>,
    mouse: (f32, f32),
    using_focus: bool,
    last_dropdown_scroll: Option<Instant>,
}

fn key_code(key: &str, right: bool) -> i32 {
    match key {
        "ArrowUp" => 273, "ArrowDown" => 274, "ArrowRight" => 275, "ArrowLeft" => 276,
        "Shift" => if right { 303 } else { 304 },
        "Control" => if right { 305 } else { 306 },
        "Alt" => if right { 307 } else { 308 },
        " " => 32, "PageUp" => 280, "PageDown" => 281,
        other => clr::input::key_by_name(other),
    }
}

impl Input {
    fn button(&mut self, point: (f32, f32), kind: i32, down: bool) {
        if self.buttons.len() < 128 {
            self.buttons.push_back(Button { point, kind, down });
        } else {
            self.buttons.clear();
            self.buttons.push_back(Button { point: (-1.0, -1.0), kind, down: false });
        }
    }

    fn events(&mut self, vm: &mut Vm, view: &View, events: Vec<Event>) {
        for event in events {
            match event {
                Event::Move { x, y } => {
                    if self.using_focus { clr::focus::clear(vm); }
                    self.using_focus = false;
                    self.mouse = view.point(x, y);
                }
                Event::Button { x, y, button, down } => {
                    self.using_focus = false;
                    self.mouse = view.point(x, y);
                    let kind = match button { 0 => 0, 1 => 2, 2 => 1, _ => continue };
                    self.button(self.mouse, kind, down);
                }
                Event::Wheel { x, y, delta } => {
                    vm.eng.mouse = view.point(x, y);
                    vm.scroll_wheel(delta.clamp(-30.0, 30.0));
                }
                Event::Key { key, right, down, repeat } if vm.eng.text_req.is_none() => {
                    let code = key_code(&key, right);
                    if code == 0 { continue; }
                    if down {
                        vm.eng.keys_down.insert(code);
                        if !repeat { vm.eng.keys_edge.insert(code); }
                    } else {
                        vm.eng.keys_down.remove(&code);
                    }
                }
                Event::Navigate { action } => self.navigate(vm, action.into()),
                Event::Scroll { x, y } => {
                    if vm.dropdown_is_open() {
                        if y.abs() >= 0.5 && self.last_dropdown_scroll.is_none_or(|t| t.elapsed() >= Duration::from_millis(90)) {
                            self.last_dropdown_scroll = Some(Instant::now());
                            self.navigate(vm, if y < 0.0 { Action::Up } else { Action::Down });
                        }
                    } else {
                        self.navigate(vm, Action::Scroll { x: x.clamp(-90.0, 90.0), y: y.clamp(-90.0, 90.0) });
                    }
                }
                Event::Text { value } => {
                    if let Some(request) = vm.eng.text_req.take() {
                        if let Some(value) = value {
                            vm.input_commit(&request.comp, &value.chars().take(1024).collect::<String>());
                        }
                    }
                }
                Event::Reset => {
                    self.buttons.clear();
                    self.mouse = (-1.0, -1.0);
                    vm.eng.keys_down.clear();
                    vm.eng.keys_edge.clear();
                    if vm.eng.mouse_down { self.button(self.mouse, vm.eng.mouse_button, false); }
                }
                _ => {}
            }
        }
    }

    fn navigate(&mut self, vm: &mut Vm, action: Action) {
        if vm.eng.text_req.is_some() { return; }
        self.using_focus = true;
        if let Some(point) = actions::apply(vm, action) {
            self.button(point, 0, true);
            self.button(point, 0, false);
        }
    }

    fn current_point(&self, vm: &Vm) -> (f32, f32) {
        if self.using_focus { clr::focus::pointer(vm).unwrap_or((-1.0, -1.0)) } else { self.mouse }
    }

    fn step(&mut self, vm: &mut Vm) {
        if let Some(button) = self.buttons.pop_front() {
            vm.eng.mouse = button.point;
            vm.eng.mouse_button = button.kind;
            vm.eng.mouse_down = button.down;
            vm.eng.mouse_down_edge = button.down;
            vm.eng.mouse_up_edge = !button.down;
        } else {
            vm.eng.mouse = self.current_point(vm);
        }
        vm.frame(LOGIC_DT);
    }
}

fn packet(out: &mut impl Write, info: Value, image: &[u8]) -> io::Result<()> {
    let info = info.to_string();
    out.write_all(&(info.len() as u32).to_le_bytes())?;
    out.write_all(&(image.len() as u32).to_le_bytes())?;
    out.write_all(info.as_bytes())?;
    out.write_all(image)?;
    out.flush()
}

fn png(renderer: &Renderer, target: &mut Vec<u8>) -> Result<(), png::EncodingError> {
    target.clear();
    let mut encoder = png::Encoder::new(target, renderer.w as u32, renderer.h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.write_header()?.write_image_data(&renderer.fb)
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let [dll, assets, save] = args else { return Err("Expected DLL, prepared data, and save directory.".into()); };
    let mut vm = Box::new(Vm::new(dll));
    vm.eng.screen = (VIRT_W as i32, VIRT_H as i32);
    vm.eng.save_dir = save.clone();
    vm.eng.desktop_panels = true;
    vm.eng.hover_always = true;
    vm.eng.read_live_fields = Some(ngu_host::live_fields);
    vm.native_mode = true;
    vm.native_inv = Some(crate::gen::hook);
    ngu_host::install_panic_hook();
    ngu_host::set_vm(&mut *vm);
    unity_api::load_prefs(&mut vm);
    vm.load_scene(assets)?;
    vm.awake_all();
    vm.enable_visual_motion();
    vm.start_desktop_save_picker()?;

    let (sender, requests) = mpsc::sync_channel(2);
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            if let Ok(line) = line {
                if sender.send(line).is_err() { break; }
            } else {
                break;
            }
        }
    });
    let mut response = io::BufWriter::new(io::stdout().lock());
    let mut renderer = Renderer::new(960, 540);
    let mut pixels = Vec::new();
    let mut input = Input { mouse: (-1.0, -1.0), ..Input::default() };
    let mut last = Instant::now();
    let mut accumulated = 0.0;
    loop {
        let now = Instant::now();
        accumulated += (now - last).as_secs_f32().min(0.2);
        last = now;
        while accumulated >= LOGIC_DT {
            input.step(&mut vm);
            accumulated -= LOGIC_DT;
        }
        let message = match requests.recv_timeout(Duration::from_secs_f32((LOGIC_DT - accumulated).max(0.0))) {
            Ok(message) => message,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        match serde_json::from_str::<Request>(&message) {
            Ok(Request::Import { import }) => {
                let result = vm.import_browser_save(&import.name, import.slot);
                packet(&mut response, json!({ "error": result.err() }), &[])?;
            }
            Ok(Request::Frame { width, height, events }) if (200..=1280).contains(&width) && (110..=900).contains(&height) && events.len() <= 96 => {
                let view = View::new(width, height);
                renderer.resize(width, height, true);
                vm.eng.desktop_layout.set_width(width as f32 / view.scale);
                input.events(&mut vm, &view, events);
                if input.buttons.is_empty() && vm.eng.mouse != input.current_point(&vm) {
                    vm.eng.mouse = input.current_point(&vm);
                    if !vm.eng.mouse_down || vm.pointer_holds_scroll_view() || vm.dropdown_is_open() { vm.pointer_update(); }
                    vm.update_color_picker_pointer();
                }
                renderer.set_subframe_time(accumulated);
                renderer.render(&mut vm, view.scale, 0.0, view.top);
                png(&renderer, &mut pixels)?;
                let entry = vm.eng.text_req.as_ref().map(|field| json!({
                    "value": field.text.chars().take(1024).collect::<String>(),
                    "max": field.max.clamp(1, 1024),
                    "numeric": field.numeric,
                }));
                let sounds: Vec<_> = std::mem::take(&mut vm.eng.sound_queue).into_iter()
                    .filter(|name| matches!(name.as_str(), "Fart" | "LongFart")).take(12).collect();
                packet(&mut response, json!({ "text": entry, "sounds": sounds }), &pixels)?;
            }
            _ => packet(&mut response, json!({ "error": "Invalid game frame request." }), &[])?,
        }
    }
}
