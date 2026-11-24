//! A complete, safe way to host the game: [`Session`] implements [`platform_api::Engine`].
//!
//! This is the entry point for a new port written in Rust. It owns the engine state (no globals of its
//! own), turns a host's input into the engine's, steps game time, and exposes the frame to draw. The
//! C ABI in [`crate::capi`] is the same service for C hosts and keeps the Vita-specific scheduling.
//!
//! ```no_run
//! use clr::session::{Session, SessionConfig};
//! use platform_api::{Engine, TriangleList};
//!
//! let mut session = Session::open(SessionConfig::new("prepared/ngu", "profile", 960, 600))?;
//! let mut frame = TriangleList::default();
//! session.advance(1.0 / 60.0);
//! session.draw(&mut frame);
//! assert!(!frame.vertices.is_empty());
//! # Ok::<(), clr::session::SessionError>(())
//! ```
//!
//! Limits: the engine reads the prepared game data through a process-wide location
//! ([`crate::assets::init`]), so open one session per process. The game's save files live in the
//! session's save directory, which the engine reads and writes directly.
#![deny(missing_docs)]

use crate::render::{Renderer, Tex, ATLAS_SIZE};
use crate::{focus, unity_api, Vm};
use platform_api::{
    Action, DrawTarget, Engine, FixedStep, Pointer, TextRequest, Texture, GAME_HEIGHT, GAME_WIDTH,
    STEP_SECONDS, TEXTURE_ATLAS,
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

/// Where the game's data and the player's profile are, and how large the output is.
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// Folder prepared with `tools/nguprep.py` from the player's own game files; it also holds
    /// `Assembly-CSharp.dll` (or an `ngu.pak` that contains it).
    pub data_dir: PathBuf,
    /// Folder for the player's saves. Created if missing.
    pub save_dir: PathBuf,
    /// Output width in device pixels.
    pub width: u32,
    /// Output height in device pixels.
    pub height: u32,
}

impl SessionConfig {
    /// A configuration for the given folders and output size.
    pub fn new(
        data_dir: impl Into<PathBuf>,
        save_dir: impl Into<PathBuf>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            data_dir: data_dir.into(),
            save_dir: save_dir.into(),
            width,
            height,
        }
    }
}

/// Why a session could not be opened. Each message says what to do next.
#[derive(Debug)]
pub enum SessionError {
    /// `Assembly-CSharp.dll` is not in the data folder: prepare the player's game files first
    /// (`docs/PORTING.md`, "Prepare the data").
    MissingAssembly(PathBuf),
    /// The scene or sprites could not be loaded: the data folder was prepared by another engine
    /// revision, or is incomplete. Prepare it again with the tools of this engine revision.
    Scene(String),
    /// The save menu could not start; the save folder may be unwritable.
    SaveMenu(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::MissingAssembly(dir) => write!(
                f,
                "Assembly-CSharp.dll not found in {}: prepare the game data first (docs/PORTING.md, \"Prepare the data\").",
                dir.display()
            ),
            SessionError::Scene(reason) => write!(
                f,
                "the game scene could not be loaded ({reason}): prepare the data again with this engine revision's tools."
            ),
            SessionError::SaveMenu(reason) => write!(f, "the save menu could not start ({reason}): check the save folder."),
        }
    }
}

impl std::error::Error for SessionError {}

struct Press {
    at: (f32, f32),
    from_device: bool,
    down: bool,
}

/// The running game. See the [module documentation](self).
pub struct Session {
    vm: Vm,
    renderer: Renderer,
    size: (u32, u32),
    clock: FixedStep,
    pointer: Pointer,
    presented: (f32, f32),
    using_focus: bool,
    presses: VecDeque<Press>,
    texture: Option<Rc<Tex>>,
}

impl Session {
    /// Load the game and open its home screen.
    pub fn open(config: SessionConfig) -> Result<Session, SessionError> {
        let dir = config.data_dir.to_string_lossy().to_string();
        crate::assets::init(&dir);
        let dll = crate::assets::read("Assembly-CSharp.dll")
            .ok_or_else(|| SessionError::MissingAssembly(config.data_dir.clone()))?;
        let mut vm = Vm::from_bytes(dll);
        vm.eng.screen = (GAME_WIDTH as i32, GAME_HEIGHT as i32);
        vm.eng.save_dir = config.save_dir.to_string_lossy().to_string();
        let _ = std::fs::create_dir_all(&vm.eng.save_dir);
        vm.eng.desktop_panels = true;
        vm.eng.desktop_layout.pc_resolutions = true;
        vm.eng.hover_always = true;
        unity_api::load_prefs(&mut vm);
        vm.load_scene(&dir).map_err(SessionError::Scene)?;
        vm.awake_all();
        vm.enable_visual_motion();
        vm.start_desktop_save_picker()
            .map_err(SessionError::SaveMenu)?;
        let mut session = Session {
            vm,
            renderer: Renderer::new(1, 1),
            size: (0, 0),
            clock: FixedStep::default(),
            pointer: Pointer::NONE,
            presented: (-1.0, -1.0),
            using_focus: false,
            presses: VecDeque::new(),
            texture: None,
        };
        session.resize(config.width.max(1), config.height.max(1));
        Ok(session)
    }

    /// The engine state, for tools and tests that need to look inside (for example the scripted steps of
    /// [`crate::script`]). A port does not need it.
    pub fn vm(&mut self) -> &mut Vm {
        &mut self.vm
    }

    /// A resolution change the game requested (`width`, `height`, `fullscreen`), if any. Honouring it
    /// is the port's choice; the game also works at any size.
    pub fn take_resolution_request(&mut self) -> Option<(u32, u32, bool)> {
        self.vm
            .eng
            .resolution_req
            .take()
            .map(|(w, h, fullscreen, _)| (w as u32, h as u32, fullscreen))
    }

    /// Pixels per game pixel, and the vertical offset that centres the 960 x 600 screen.
    fn view(&self) -> (f32, f32) {
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        let scale = (w / GAME_WIDTH).min(h / GAME_HEIGHT);
        (scale, (h - GAME_HEIGHT * scale) / 2.0)
    }

    /// Device pixels to game coordinates (y up), or off-screen.
    fn to_game(&self, x: f32, y: f32) -> (f32, f32) {
        let (scale, top) = self.view();
        let gy = (y - top) / scale;
        let width = self.size.0 as f32 / scale;
        if x >= 0.0 && x / scale < width && (0.0..GAME_HEIGHT).contains(&gy) {
            (x / scale, GAME_HEIGHT - gy)
        } else {
            (-1.0, -1.0)
        }
    }

    fn pointer_in_game(&self) -> (f32, f32) {
        if self.using_focus {
            focus::pointer(&self.vm).unwrap_or((-1.0, -1.0))
        } else {
            self.to_game(self.pointer.x, self.pointer.y)
        }
    }

    fn tick(&mut self) {
        match self.presses.pop_front() {
            Some(press) => {
                self.vm.eng.mouse = if press.from_device {
                    self.to_game(press.at.0, press.at.1)
                } else {
                    press.at
                };
                self.vm.eng.mouse_button = 0;
                self.vm.eng.mouse_down = press.down;
                self.vm.eng.mouse_down_edge = press.down;
                self.vm.eng.mouse_up_edge = !press.down;
            }
            None => self.vm.eng.mouse = self.pointer_in_game(),
        }
        self.vm.frame(STEP_SECONDS);
    }
}

impl Engine for Session {
    fn resize(&mut self, width: u32, height: u32) {
        if (width, height) == self.size {
            return;
        }
        self.size = (width, height);
        let (scale, _) = self.view();
        self.vm.eng.desktop_layout.set_width(width as f32 / scale);
        self.renderer.resize(width as usize, height as usize, false);
        self.renderer.prepare_layout(&mut self.vm);
    }

    fn advance(&mut self, seconds: f32) {
        for _ in 0..self.clock.advance(seconds) {
            self.tick();
        }
    }

    fn set_pointer(&mut self, pointer: Pointer) {
        if self.using_focus && (pointer.x, pointer.y) != (self.pointer.x, self.pointer.y) {
            focus::clear(&mut self.vm);
            self.using_focus = false;
        }
        if pointer.down != self.pointer.down && self.presses.len() < 128 {
            self.using_focus = false;
            self.presses.push_back(Press {
                at: (pointer.x, pointer.y),
                from_device: true,
                down: pointer.down,
            });
        }
        self.pointer = pointer;
    }

    fn action(&mut self, action: Action) {
        if self.vm.eng.text_req.is_some() {
            return;
        }
        self.using_focus = true;
        let (click, _) = focus::pad_action(&mut self.vm, action.code(), false);
        if let Some(at) = click {
            self.presses.push_back(Press {
                at,
                from_device: false,
                down: true,
            });
            self.presses.push_back(Press {
                at,
                from_device: false,
                down: false,
            });
        }
    }

    fn scroll(&mut self, dx: f32, dy: f32) {
        self.using_focus = true;
        focus::scroll(&mut self.vm, dx.clamp(-90.0, 90.0), dy.clamp(-90.0, 90.0));
    }

    fn draw(&mut self, target: &mut dyn DrawTarget) -> usize {
        let (scale, top) = self.view();
        let point = self.pointer_in_game();
        if self.presses.is_empty() && self.presented != point {
            self.presented = point;
            self.vm.eng.mouse = point;
            if !self.vm.eng.mouse_down
                || self.vm.pointer_holds_scroll_view()
                || self.vm.dropdown_is_open()
            {
                self.vm.pointer_update();
            }
            self.vm.update_color_picker_pointer();
        }
        self.renderer.set_subframe_time(self.clock.remainder());
        self.renderer.emit_frame(&mut self.vm, scale, 0.0, top);
        target.draw_quads(&self.renderer.quads);
        self.renderer.quads.len()
    }

    fn texture(&mut self, id: u32) -> Option<Texture<'_>> {
        if id == TEXTURE_ATLAS {
            return Some(Texture {
                width: ATLAS_SIZE as u32,
                height: ATLAS_SIZE as u32,
                rgba: self.renderer.atlas_pixels(),
            });
        }
        let tex = self.renderer.tex_load(id)?;
        let held = self.texture.insert(tex);
        Some(Texture {
            width: held.w as u32,
            height: held.h as u32,
            rgba: &held.px,
        })
    }

    fn atlas_changed(&mut self) -> bool {
        self.renderer.atlas_take_dirty()
    }

    fn release_texture(&mut self, id: u32) {
        self.renderer.tex_release(id);
    }

    fn take_text_request(&mut self) -> Option<TextRequest> {
        let request = self.vm.eng.text_req.as_mut().filter(|r| r.pending)?;
        request.pending = false;
        Some(TextRequest {
            text: request.text.clone(),
            max_len: usize::try_from(request.max).ok().filter(|max| *max > 0),
            numeric: request.numeric,
        })
    }

    fn submit_text(&mut self, text: Option<&str>) {
        if let Some(request) = self.vm.eng.text_req.take() {
            if let Some(text) = text {
                self.vm
                    .input_commit(&request.comp, &text.chars().take(1024).collect::<String>());
            }
        }
    }

    fn take_sounds(&mut self) -> Vec<String> {
        std::mem::take(&mut self.vm.eng.sound_queue)
    }
}
