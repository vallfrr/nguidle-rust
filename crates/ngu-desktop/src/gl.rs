//! GPU window (miniquad: OpenGL / WebGL, no native dependency): draws the engine's quad list on the GPU, sharp at any window
//! size, presented at the monitor's refresh rate (vsync), like the original game (`vSyncCount = 1`).
//! The game logic runs in fixed 1/30 s steps caught up on real time, exactly like the software window.
//! Input: every key goes to the game (original behaviour) unless F1 toggled focus navigation; F11 toggles fullscreen.
use crate::actions::{apply, Action};
use crate::{layout, Options, LOGIC_DT, VIRT_H, VIRT_W};
use clr::render::{Quad, Renderer, ATLAS_SIZE, TEX_ATLAS, TEX_WHITE};
use clr::unity::{mark_dirty, with_comp, with_go, with_tr, CK, SelKind};
use clr::value::Value;
use clr::Vm;
use miniquad::*;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

const VERTEX: &str = r#"#version 100
attribute vec2 pos;
attribute vec2 uv;
attribute vec4 color;
uniform vec2 screen;
varying highp vec2 vuv;
varying lowp vec4 vcolor;
void main() {
    gl_Position = vec4(pos.x / screen.x * 2.0 - 1.0, 1.0 - pos.y / screen.y * 2.0, 0.0, 1.0);
    vuv = uv;
    vcolor = color / 255.0; // Byte4 vertex attributes arrive as 0..255
}"#;

const FRAGMENT: &str = r#"#version 100
precision mediump float;
varying highp vec2 vuv;
varying lowp vec4 vcolor;
uniform sampler2D tex;
void main() {
    gl_FragColor = texture2D(tex, vuv) * vcolor;
}"#;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vert {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [u8; 4],
}

#[repr(C)]
struct Uniforms {
    screen: (f32, f32),
}

/// Quads per upload: 16-bit indices address 65,536 vertices.
const CHUNK: usize = 16_000;

/// Interpolate the rendered HUD fill between game ticks. The Slider value stays authoritative.
struct EnergyFill {
    transform: Value,
    samples: VecDeque<(f64, f32)>,
}

impl EnergyFill {
    fn new(vm: &Vm) -> Self {
        let transform = vm.eng.gos.iter().find(|go| {
            with_go(go, |g| g.name == "Energy Bar").unwrap_or(false)
        }).and_then(|go| {
            with_go(go, |g| {
                g.comps.iter().find_map(|component| {
                    with_comp(component, |c| match &c.k {
                        CK::Sel(s) if s.kind == SelKind::Slider && s.direction == 0 => {
                            Some(s.fill_rect.clone())
                        }
                        _ => None,
                    }).flatten()
                })
            }).flatten()
        }).unwrap_or_default();
        let current = with_tr(&transform, |t| t.amax[0]).unwrap_or(0.0);
        Self {
            transform,
            samples: VecDeque::from([(vm.eng.time, current)]),
        }
    }

    fn sample_tick(&mut self, vm: &Vm) {
        let Some(bar) = vm.eng.invokes.iter().find(|timer| timer.method == "updateBar") else { return };
        let time = bar.next - bar.repeat;
        if time > vm.eng.time {
            return;
        }
        let value = with_tr(&self.transform, |t| t.amax[0]).unwrap_or_default();
        if self.samples.back().is_some_and(|(last, _)| *last == time) {
            return;
        }
        if self.samples.back().is_some_and(|(last, _)| *last > time) {
            self.samples.clear();
        }
        if self.samples.len() == 4 {
            self.samples.pop_front();
        }
        self.samples.push_back((time, value));
    }

    fn at(&self, time: f64) -> f32 {
        let Some(&(mut before, mut value)) = self.samples.front() else { return 0.0 };
        for &(after, next) in self.samples.iter().skip(1) {
            if time < after {
                return energy_fill_at(value, next, ((time - before) / (after - before)) as f32);
            }
            (before, value) = (after, next);
        }
        value
    }

    fn set_rendered(&self, value: f32) -> Option<f32> {
        let old = with_tr(&self.transform, |t| std::mem::replace(&mut t.amax[0], value))?;
        if old != value {
            mark_dirty(&self.transform);
        }
        Some(old)
    }
}

fn energy_fill_at(previous: f32, current: f32, fraction: f32) -> f32 {
    let fraction = fraction.clamp(0.0, 1.0);
    if current >= previous {
        previous + (current - previous) * fraction
    } else if previous >= 0.8 && current <= 0.2 && fraction < 1.0 {
        // Complete the visible fill before the next tick starts at the reset value.
        previous + (1.0 - previous) * fraction
    } else {
        current
    }
}

struct Stage {
    ctx: Box<dyn RenderingBackend>,
    vm: &'static mut Vm,
    rend: Renderer,
    pipeline: Pipeline,
    vbuf: BufferId,
    ibuf: BufferId,
    white: TextureId,
    atlas: TextureId,
    texs: HashMap<u32, (TextureId, bool)>,
    size: (f32, f32),
    // real-time loop
    last: Instant,
    acc: f32,
    energy_fill: EnergyFill,
    // input
    nav_mode: bool,
    #[cfg(windows)]
    audio: Option<(rodio::OutputStream, rodio::OutputStreamHandle)>,
    #[cfg(windows)]
    pad: Option<crate::pad::Pad>,
    fullscreen: bool,
    typed: Vec<char>,
    text_buf: String,
    pending: Vec<(f32, f32)>,
    synth_phase: u8,
    perf: bool,
    p: [f32; 4],
    previous_present: Option<Instant>,
    longest_frame_ms: f32,
    missed_frames: u32,
    frames: u64,
    t0: Instant,
    /// An explicit classic, Vita or custom region profile.
    profile: Option<clr::profile::Profile>,
    hold: bool,
    pointer: Option<(f32, f32)>,
    focus_input: bool,
    hover_at: (f32, f32),
}

impl Stage {
    fn new(vm: &'static mut Vm, o: &Options) -> Stage {
        let mut ctx = window::new_rendering_backend();
        let shader = ctx
            .new_shader(
                ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT },
                ShaderMeta { images: vec!["tex".to_string()], uniforms: UniformBlockLayout { uniforms: vec![UniformDesc::new("screen", UniformType::Float2)] } },
            )
            .expect("shader");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[VertexAttribute::new("pos", VertexFormat::Float2), VertexAttribute::new("uv", VertexFormat::Float2), VertexAttribute::new("color", VertexFormat::Byte4)],
            shader,
            PipelineParams {
                color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),
                ..Default::default()
            },
        );
        let vbuf = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Stream, BufferSource::empty::<Vert>(CHUNK * 4));
        let mut idx: Vec<u16> = Vec::with_capacity(CHUNK * 6);
        for q in 0..CHUNK as u32 {
            for i in [0u32, 1, 2, 0, 2, 3] {
                idx.push((q * 4 + i) as u16);
            }
        }
        let ibuf = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&idx));
        let white = ctx.new_texture_from_rgba8(1, 1, &[255, 255, 255, 255]);
        let atlas = ctx.new_texture_from_rgba8(ATLAS_SIZE as u16, ATLAS_SIZE as u16, &vec![0u8; ATLAS_SIZE * ATLAS_SIZE * 4]);
        ctx.texture_set_filter(atlas, FilterMode::Nearest, MipmapFilterMode::None);
        vm.eng.screen = (VIRT_W as i32, VIRT_H as i32);
        vm.eng.hover_always = true;
        vm.eng.mouse = (-1.0, -1.0);
        let (w, h) = window::screen_size();
        let profile = std::env::var("NGU_LAYOUT").ok().filter(|name| name != "pc").and_then(|name| {
            clr::profile::load(if name == "classic" { "pc" } else { &name })
                .map_err(|e| eprintln!("[layout] {e}")).ok()
        });
        vm.eng.desktop_layout.pc_resolutions = profile.is_none();
        let energy_fill = EnergyFill::new(vm);
        Stage {
            ctx,
            vm,
            rend: Renderer::new(1, 1),
            pipeline,
            vbuf,
            ibuf,
            white,
            atlas,
            texs: HashMap::new(),
            size: (w, h),
            last: Instant::now(),
            acc: 0.0,
            energy_fill,
            nav_mode: false,
            #[cfg(windows)]
            audio: rodio::OutputStream::try_default().ok(),
            #[cfg(windows)]
            pad: crate::pad::Pad::new(),
            fullscreen: false,
            typed: vec![],
            text_buf: String::new(),
            pending: vec![],
            synth_phase: 0,
            perf: std::env::var("NGU_PERF").is_ok() && o.fps > 0,
            p: [0.0; 4],
            previous_present: None,
            longest_frame_ms: 0.0,
            missed_frames: 0,
            frames: 0,
            t0: Instant::now(),
            profile,
            hold: false,
            pointer: None,
            focus_input: false,
            hover_at: (-1.0, -1.0),
        }
    }

    fn viewport(&self, w: f32, h: f32) -> (f32, f32, f32, f32) {
        let (scale, ox, oy) = layout(w.max(1.0) as usize, h.max(1.0) as usize);
        if self.profile.is_none() {
            (scale, 0.0, oy, w / scale)
        } else {
            (scale, ox, oy, VIRT_W)
        }
    }

    fn map_pointer(&mut self) -> bool {
        if self.focus_input {
            return false;
        }
        let Some((x, y)) = self.pointer else { return false };
        let mapped = if self.profile.is_some() {
            clr::profile::dev_to_game(&self.rend.regions, x, y)
        } else {
            let (w, h) = self.size;
            let (scale, ox, oy, width) = self.viewport(w, h);
            let (gx, gy) = ((x - ox) / scale, (y - oy) / scale);
            (gx >= 0.0 && gx < width && gy >= 0.0 && gy < VIRT_H).then_some((gx, gy))
        };
        if self.pending.is_empty() && self.synth_phase == 0 {
            self.vm.eng.mouse = mapped.map_or((-1.0, -1.0), |(gx, gy)| (gx, VIRT_H - gy));
        }
        mapped.is_some()
    }

    fn navigate(&mut self, action: Action) {
        if self.vm.eng.text_req.is_some() {
            if action == Action::Back {
                self.vm.eng.text_req = None;
                self.text_buf.clear();
            }
            return;
        }
        self.focus_input = true;
        if let Some(click) = apply(self.vm, action) {
            self.pending.push(click);
        }
    }

    fn update_hover(&mut self) {
        if !self.pending.is_empty() || self.synth_phase != 0 || self.vm.eng.mouse_down ||
            self.vm.eng.mouse_down_edge || self.vm.eng.mouse_up_edge {
            return;
        }
        if self.focus_input {
            self.vm.eng.mouse = clr::focus::pointer(self.vm).unwrap_or((-1.0, -1.0));
        }
        if self.vm.eng.mouse != self.hover_at {
            self.hover_at = self.vm.eng.mouse;
            self.vm.pointer_update();
        }
    }

    /// Sampler state and GPU copy of a game texture, uploaded on first use.
    fn texture(&mut self, id: u32, nearest: bool) -> TextureId {
        match id {
            TEX_WHITE => return self.white,
            TEX_ATLAS => return self.atlas,
            _ => {}
        }
        if !self.texs.contains_key(&id) {
            let Some(t) = self.rend.tex_load(id) else {
                if self.perf {
                    eprintln!("[gl] texture {id} unavailable");
                }
                return self.white;
            };
            if self.perf && self.texs.len() < 6 {
                eprintln!("[gl] texture {id}: {}x{} px={} first={:?}", t.w, t.h, t.px.len(), &t.px[..t.px.len().min(8)]);
            }
            let tid = self.ctx.new_texture_from_rgba8(t.w as u16, t.h as u16, &t.px);
            self.ctx.texture_set_wrap(tid, TextureWrap::Clamp, TextureWrap::Clamp);
            self.ctx.texture_set_filter(tid, FilterMode::Linear, MipmapFilterMode::None);
            self.texs.insert(id, (tid, false));
        }
        let (tid, cur) = self.texs[&id];
        if cur != nearest {
            let f = if nearest { FilterMode::Nearest } else { FilterMode::Linear };
            self.ctx.texture_set_filter(tid, f, MipmapFilterMode::None);
            self.texs.insert(id, (tid, nearest));
        }
        tid
    }

    fn game_keys(&mut self, k: KeyCode, down: bool, repeat: bool) {
        if self.nav_mode || self.vm.eng.text_req.is_some() {
            return;
        }
        let Some(code) = unity_key(k) else { return };
        if down {
            self.vm.eng.keys_down.insert(code);
            if !repeat {
                self.vm.eng.keys_edge.insert(code);
            }
        } else {
            self.vm.eng.keys_down.remove(&code);
        }
    }

    fn button(&mut self, b: MouseButton, down: bool) {
        let i = match b {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            _ => return,
        };
        if !self.pending.is_empty() || self.synth_phase != 0 {
            return;
        }
        let eng = &mut self.vm.eng;
        eng.mouse_button = i;
        eng.mouse_down = down;
        if down {
            eng.mouse_down_edge = true;
        } else {
            eng.mouse_up_edge = true;
        }
    }
}

/// Unity `KeyCode` of a window key (letters, digits, arrows, modifiers and the few others the game reads).
fn unity_key(k: KeyCode) -> Option<i32> {
    let n = k as i32;
    let (a, z, k0, k9) = (KeyCode::A as i32, KeyCode::Z as i32, KeyCode::Key0 as i32, KeyCode::Key9 as i32);
    Some(match k {
        _ if (a..=z).contains(&n) => 97 + (n - a),
        _ if (k0..=k9).contains(&n) => 48 + (n - k0),
        KeyCode::Up => 273,
        KeyCode::Down => 274,
        KeyCode::Right => 275,
        KeyCode::Left => 276,
        KeyCode::LeftShift => 304,
        KeyCode::RightShift => 303,
        KeyCode::LeftControl => 306,
        KeyCode::RightControl => 305,
        KeyCode::LeftAlt => 308,
        KeyCode::RightAlt => 307,
        KeyCode::Space => 32,
        KeyCode::Enter => 13,
        KeyCode::Escape => 27,
        KeyCode::Tab => 9,
        KeyCode::Backspace => 8,
        _ => return None,
    })
}

impl EventHandler for Stage {
    fn update(&mut self) {
        let t_logic = Instant::now();
        // text entry (InputField): typed characters edit the field
        if let Some(comp) = self.vm.eng.text_req.as_ref().map(|r| r.comp.clone()) {
            let mut changed = false;
            for c in std::mem::take(&mut self.typed) {
                if c == '\u{8}' {
                    self.text_buf.pop();
                    changed = true;
                } else if !c.is_control() {
                    self.text_buf.push(c);
                    changed = true;
                }
            }
            if let Some(r) = self.vm.eng.text_req.as_mut() {
                if r.pending {
                    r.pending = false;
                    self.text_buf = r.text.clone();
                }
            }
            if changed {
                self.vm.input_commit(&comp, &self.text_buf.clone());
            }
        } else {
            self.typed.clear();
        }
        // Screen.SetResolution from a game button: resize the window or toggle fullscreen (start-up calls are ignored)
        if let Some((w, h, fs, at)) = self.vm.eng.resolution_req.take() {
            if at > 120 {
                if fs != self.fullscreen {
                    self.fullscreen = fs;
                    window::set_fullscreen(fs);
                }
                if !fs {
                    let (w, h) = if self.profile.is_none() { clr::desktop_layout::pc_resolution(w, h) } else { (w, h) };
                    window::set_window_size(w.max(320) as u32, h.max(200) as u32);
                }
            }
        }
        // sounds requested by the game (AudioSource.Play): `audio/<clip>.wav` extracted by nguprep
        for name in std::mem::take(&mut self.vm.eng.sound_queue) {
            #[cfg(windows)]
            if let (Some((_, h)), Some(bytes)) = (self.audio.as_ref(), clr::assets::read(&format!("audio/{name}.wav"))) {
                if let Ok(src) = rodio::Decoder::new(std::io::Cursor::new(bytes)) {
                    let _ = h.play_raw(rodio::Source::convert_samples(src));
                }
            }
            #[cfg(not(windows))]
            let _ = name;
        }
        // gamepad: always drives the focus (the keyboard keeps going to the game)
        #[cfg(windows)]
        for action in self.pad.as_mut().map(|pad| pad.poll()).unwrap_or_default() {
            self.navigate(action);
        }
        self.update_hover();
        // fixed logic steps caught up on real time
        let now = Instant::now();
        self.acc += (now - self.last).as_secs_f32().min(0.25);
        self.last = now;
        let mut steps = 0;
        while self.acc >= LOGIC_DT && steps < 6 {
            if let Some(&(x, y)) = self.pending.first() {
                if self.synth_phase == 0 {
                    self.vm.eng.mouse = (x, y);
                    self.vm.eng.mouse_button = 0;
                    self.vm.eng.mouse_down = true;
                    self.vm.eng.mouse_down_edge = true;
                    self.synth_phase = 1;
                } else {
                    self.vm.eng.mouse_down = false;
                    self.vm.eng.mouse_up_edge = true;
                    self.synth_phase = 0;
                    self.pending.remove(0);
                }
            }
            self.vm.frame(LOGIC_DT);
            self.energy_fill.sample_tick(self.vm);
            self.acc -= LOGIC_DT;
            steps += 1;
        }
        if steps == 6 {
            self.acc = 0.0;
        }
        self.p[0] += t_logic.elapsed().as_secs_f32() * 1000.0;
    }

    fn draw(&mut self) {
        let (w, h) = window::screen_size();
        if w < 1.0 || h < 1.0 {
            return;
        }
        self.size = (w, h);
        self.rend.resize(w as usize, h as usize, false);
        let (scale, ox, oy, width) = self.viewport(w, h);
        if let Some(p) = &self.profile {
            self.rend.regions = p.place(w, h, self.hold, self.pointer.map_or(0.0, |(_, y)| y));
        }
        self.vm.eng.desktop_layout.set_width(width);
        if self.profile.is_none() {
            self.map_pointer();
        }
        // updateBar is scheduled every 20 ms, so successive simulation ticks can
        // run it a different number of times. Render on its clock.
        let time = self.vm.eng.time + f64::from(self.acc) - f64::from(LOGIC_DT) - 0.02;
        let saved_fill = self.energy_fill.set_rendered(self.energy_fill.at(time));
        self.rend.emit_frame(self.vm, scale, ox, oy);
        if let Some(saved_fill) = saved_fill {
            self.energy_fill.set_rendered(saved_fill);
        }
        let t_draw = Instant::now();
        if self.rend.atlas_take_dirty() {
            self.ctx.texture_update(self.atlas, self.rend.atlas_pixels());
        }
        let quads: Vec<Quad> = std::mem::take(&mut self.rend.quads);
        self.ctx.begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
        self.ctx.apply_pipeline(&self.pipeline);
        let mut verts: Vec<Vert> = Vec::with_capacity(CHUNK.min(quads.len()) * 4);
        for chunk in quads.chunks(CHUNK) {
            verts.clear();
            for q in chunk {
                let c = [(q.col[0] * 255.0 + 0.5) as u8, (q.col[1] * 255.0 + 0.5) as u8, (q.col[2] * 255.0 + 0.5) as u8, (q.col[3] * 255.0 + 0.5) as u8];
                for i in 0..4 {
                    verts.push(Vert { pos: q.p[i], uv: q.uv[i], color: c });
                }
            }
            self.ctx.buffer_update(self.vbuf, BufferSource::slice(&verts));
            let mut i = 0;
            while i < chunk.len() {
                let q0 = &chunk[i];
                let mut j = i + 1;
                while j < chunk.len() && chunk[j].tex == q0.tex && chunk[j].nearest == q0.nearest && chunk[j].clip == q0.clip {
                    j += 1;
                }
                let tex = self.texture(q0.tex, q0.nearest);
                let [x0, y0, x1, y1] = q0.clip;
                let (cx0, cy0, cx1, cy1) = (x0.max(0), y0.max(0), x1.min(w as i32), y1.min(h as i32));
                if cx1 > cx0 && cy1 > cy0 {
                    self.ctx.apply_scissor_rect(cx0, h as i32 - cy1, cx1 - cx0, cy1 - cy0);
                    self.ctx.apply_bindings(&Bindings { vertex_buffers: vec![self.vbuf], index_buffer: self.ibuf, images: vec![tex] });
                    self.ctx.apply_uniforms(UniformsSource::table(&Uniforms { screen: (w, h) }));
                    self.ctx.draw((i * 6) as i32, ((j - i) * 6) as i32, 1);
                }
                i = j;
            }
        }
        self.rend.quads = quads;
        self.ctx.end_render_pass();
        self.ctx.commit_frame();
        self.p[1] += self.rend.t_emit;
        self.p[2] += t_draw.elapsed().as_secs_f32() * 1000.0;
        self.frames += 1;
        if self.perf {
            let now = Instant::now();
            if let Some(previous) = self.previous_present.replace(now) {
                let ms = (now - previous).as_secs_f32() * 1000.0;
                self.longest_frame_ms = self.longest_frame_ms.max(ms);
                self.missed_frames += u32::from(ms > 20.0);
            }
        }
        if self.perf && self.frames % 120 == 0 {
            let s = self.t0.elapsed().as_secs_f32();
            eprintln!("[perf] gpu {:.1} img/s | per frame (ms): logic {:.1} emit {:.1} draw+submit {:.1} | {}x{}", 120.0 / s, self.p[0] / 120.0, self.p[1] / 120.0, self.p[2] / 120.0, w as i32, h as i32);
            eprintln!("[perf] frame interval | max {:.1} ms, {} >20 ms", self.longest_frame_ms, self.missed_frames);
            self.p = [0.0; 4];
            self.longest_frame_ms = 0.0;
            self.missed_frames = 0;
            self.t0 = Instant::now();
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        if self.focus_input && self.pointer != Some((x, y)) {
            self.focus_input = false;
            clr::focus::clear(self.vm);
        }
        self.pointer = Some((x, y));
        self.map_pointer();
    }

    fn mouse_wheel_event(&mut self, _x: f32, y: f32) {
        if self.map_pointer() {
            self.vm.scroll_wheel(y.signum() * y.abs().min(3.0));
        }
    }

    fn mouse_button_down_event(&mut self, b: MouseButton, x: f32, y: f32) {
        self.focus_input = false;
        clr::focus::clear(self.vm);
        self.pointer = Some((x, y));
        if self.map_pointer() {
            self.button(b, true);
        }
    }

    fn mouse_button_up_event(&mut self, b: MouseButton, x: f32, y: f32) {
        self.mouse_motion_event(x, y);
        self.button(b, false);
    }

    fn char_event(&mut self, c: char, _m: KeyMods, _repeat: bool) {
        self.typed.push(c);
    }

    fn key_down_event(&mut self, k: KeyCode, _m: KeyMods, repeat: bool) {
        match k {
            KeyCode::F1 if !repeat => {
                self.nav_mode = !self.nav_mode;
                self.vm.eng.keys_down.clear();
                if !self.nav_mode {
                    self.focus_input = false;
                    clr::focus::clear(self.vm);
                    self.map_pointer();
                }
            }
            // profile hold action (Vita L): held = page list in place of the player column
            KeyCode::F2 => self.hold = true,
            KeyCode::F11 if !repeat => {
                self.fullscreen = !self.fullscreen;
                window::set_fullscreen(self.fullscreen);
            }
            _ => {}
        }
        if self.vm.eng.text_req.is_some() {
            // Enter / Escape end the text entry; Backspace edits
            match k {
                KeyCode::Enter | KeyCode::Escape => {
                    self.vm.eng.text_req = None;
                    self.text_buf.clear();
                }
                KeyCode::Backspace => self.typed.push('\u{8}'),
                _ => {}
            }
            return;
        }
        if self.nav_mode {
            let a = match k {
                KeyCode::Up | KeyCode::W => Some(Action::Up),
                KeyCode::Down | KeyCode::S => Some(Action::Down),
                KeyCode::Left | KeyCode::A => Some(Action::Left),
                KeyCode::Right | KeyCode::D => Some(Action::Right),
                KeyCode::Enter | KeyCode::Space => Some(Action::Activate),
                KeyCode::Escape => Some(Action::Back),
                KeyCode::Tab | KeyCode::PageDown | KeyCode::E => Some(Action::Next),
                KeyCode::PageUp => Some(Action::Prev),
                _ => None,
            };
            if let Some(a) = a {
                self.navigate(a);
            }
            return;
        }
        self.game_keys(k, true, repeat);
    }

    fn key_up_event(&mut self, k: KeyCode, _m: KeyMods) {
        if k == KeyCode::F2 {
            self.hold = false;
        }
        self.game_keys(k, false, false);
    }
}

/// Opens the GPU window and runs the game until it is closed.
pub fn run_gl(vm: &'static mut Vm, o: Options) {
    let conf = conf::Conf {
        window_title: "NGU Idle".to_string(),
        window_width: o.width as i32,
        window_height: o.height as i32,
        window_resizable: true,
        high_dpi: cfg!(windows),
        sample_count: 1,
        // vsync like the original; NGU_VSYNC=0 disables it (diagnostics: measures the raw frame cost)
        platform: conf::Platform { swap_interval: Some(if std::env::var("NGU_VSYNC").map(|v| v == "0").unwrap_or(false) { 0 } else { 1 }), ..Default::default() },
        ..Default::default()
    };
    start(conf, move || Box::new(Stage::new(vm, &o)));
}

#[cfg(test)]
mod tests {
    use super::energy_fill_at;

    #[test]
    fn fill_smooths_growth_and_resets_without_a_reverse_animation() {
        assert_eq!(energy_fill_at(0.25, 0.75, 0.5), 0.5);
        assert_eq!(energy_fill_at(0.875, 0.0, 0.5), 0.9375);
        assert_eq!(energy_fill_at(0.875, 0.0, 1.0), 0.0);
        assert_eq!(energy_fill_at(0.5, 0.125, 0.5), 0.125);
        assert_eq!(energy_fill_at(0.75, 0.75, 0.5), 0.75);
    }
}
