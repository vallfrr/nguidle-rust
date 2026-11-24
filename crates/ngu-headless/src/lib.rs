//! Reference port of the engine: the smallest complete host, with no window and no graphics API.
//!
//! [`Host`] shows everything a real port does, in order, against any [`platform_api::Engine`]:
//! it sets the pointer, applies navigation actions, advances game time, draws a frame into a
//! [`TriangleList`], uploads each texture the frame mentions (and releases the engine's copy), watches
//! the glyph atlas, answers text requests and collects sound requests. Time is virtual, so a run is
//! reproducible and takes no real time.
//!
//! Run it against the synthetic engine in CI ([`platform_api::testing::ScriptedEngine`]) or against the
//! real game with the `ngu-headless` binary. Copy [`Host::run`] when starting a port.
//!
//! A [`Script`] is a list of [`Step`]s, one per line of text (see [`Script::parse`]).

#![deny(missing_docs)]

use platform_api::{
    Action, Engine, MouseButton, Pointer, TriangleList, FINGERPRINT_SEED, TEXTURE_ATLAS,
};
use std::collections::HashSet;
use std::fmt;

/// One thing the simulated player or platform does.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Let `seconds` of virtual time pass, drawing a frame every 1/60 s like a 60 Hz display.
    Wait(f32),
    /// Move the pointer to device pixels `(x, y)`.
    Move(f32, f32),
    /// Press the primary button at the current pointer position.
    Press,
    /// Release the primary button.
    Release,
    /// Move to `(x, y)`, press, wait 0.1 s, release, wait 0.1 s.
    Click(f32, f32),
    /// Apply a navigation action.
    Action(Action),
    /// Scroll the focused list by `(dx, dy)` game pixels.
    Scroll(f32, f32),
    /// Right-click at `(x, y)`: press, one frame, release.
    RightClick(f32, f32),
    /// Tap a key (Unity `KeyCode`, see `platform_api::keys`): down, one frame, up.
    Key(i32),
    /// Turn the mouse wheel by `notches` (positive = up) over the point `(x, y)`.
    Wheel(f32, f32, f32),
    /// Answer the pending text request with this text, or cancel it with `None`.
    Text(Option<String>),
    /// Draw one frame now.
    Draw,
}

fn action(word: &str) -> Option<Action> {
    Some(match word {
        "up" => Action::Up,
        "down" => Action::Down,
        "left" => Action::Left,
        "right" => Action::Right,
        "activate" => Action::Activate,
        "back" => Action::Back,
        "prev" => Action::Prev,
        "next" => Action::Next,
        _ => return None,
    })
}

/// A list of steps, parsed from text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Script(pub Vec<Step>);

impl Script {
    /// Parse one step per line; blank lines and `#` comments are ignored.
    ///
    /// ```text
    /// wait 1.5
    /// click 480 300
    /// action down
    /// action activate
    /// text Bob          # or "text -" to cancel
    /// scroll 0 40
    /// rclick 480 300
    /// key 32            # tap Space (Unity KeyCode)
    /// wheel 480 300 1
    /// ```
    ///
    /// The error names the line and lists the valid verbs.
    pub fn parse(source: &str) -> Result<Script, String> {
        let mut steps = Vec::new();
        for (number, line) in source.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (verb, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let numbers = || -> Result<Vec<f32>, String> {
                rest.split_whitespace()
                    .map(|word| {
                        word.parse::<f32>()
                            .map_err(|_| format!("line {}: `{word}` is not a number", number + 1))
                    })
                    .collect()
            };
            let pair = |values: Vec<f32>| -> Result<(f32, f32), String> {
                match values[..] {
                    [x, y] => Ok((x, y)),
                    _ => Err(format!("line {}: `{verb}` takes two numbers", number + 1)),
                }
            };
            steps.push(match verb {
                "wait" => match numbers()?[..] {
                    [seconds] => Step::Wait(seconds),
                    _ => return Err(format!("line {}: `wait` takes one number (seconds)", number + 1)),
                },
                "move" => {
                    let (x, y) = pair(numbers()?)?;
                    Step::Move(x, y)
                }
                "click" => {
                    let (x, y) = pair(numbers()?)?;
                    Step::Click(x, y)
                }
                "scroll" => {
                    let (x, y) = pair(numbers()?)?;
                    Step::Scroll(x, y)
                }
                "rclick" => {
                    let (x, y) = pair(numbers()?)?;
                    Step::RightClick(x, y)
                }
                "wheel" => match numbers()?[..] {
                    [x, y, notches] => Step::Wheel(x, y, notches),
                    _ => return Err(format!("line {}: `wheel` takes three numbers (x y notches)", number + 1)),
                },
                "key" => match numbers()?[..] {
                    [code] => Step::Key(code as i32),
                    _ => return Err(format!("line {}: `key` takes one number (a Unity KeyCode)", number + 1)),
                },
                "press" => Step::Press,
                "release" => Step::Release,
                "draw" => Step::Draw,
                "action" => Step::Action(action(rest.trim()).ok_or_else(|| {
                    format!("line {}: `action` takes up, down, left, right, activate, back, prev or next", number + 1)
                })?),
                "text" => Step::Text(match rest.trim() {
                    "-" => None,
                    text => Some(text.to_string()),
                }),
                other => {
                    return Err(format!(
                        "line {}: unknown step `{other}` (wait, move, press, release, click, rclick, action, scroll, key, wheel, text, draw)",
                        number + 1
                    ))
                }
            });
        }
        Ok(Script(steps))
    }
}

/// What a run did, for a port's own checks and for regression comparisons.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Frames drawn.
    pub frames: u64,
    /// Quads in the last frame.
    pub last_quads: usize,
    /// Vertices in the last frame.
    pub last_vertices: usize,
    /// Draw batches in the last frame.
    pub last_batches: usize,
    /// Distinct textures uploaded (each is released afterwards).
    pub textures_uploaded: usize,
    /// Times the glyph atlas was uploaded again.
    pub atlas_uploads: u64,
    /// Text requests answered.
    pub text_requests: u64,
    /// Sound requests received.
    pub sounds: u64,
    /// FNV-1a hash of every vertex of every frame, in order: equal runs give equal hashes.
    pub hash: u64,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "frames {} | last frame: {} quads, {} vertices, {} batches | textures {} | atlas uploads {} | text requests {} | sounds {} | hash {:016x}",
            self.frames, self.last_quads, self.last_vertices, self.last_batches, self.textures_uploaded,
            self.atlas_uploads, self.text_requests, self.sounds, self.hash
        )
    }
}

/// The reference host. See the [crate documentation](crate).
pub struct Host<E: Engine> {
    /// The engine being driven.
    pub engine: E,
    frame: TriangleList,
    pointer: Pointer,
    uploaded: HashSet<u32>,
    report: Report,
    pending_text: Option<String>,
}

const FRAME: f32 = 1.0 / 60.0;

impl<E: Engine> Host<E> {
    /// Host `engine`.
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            frame: TriangleList::default(),
            pointer: Pointer::NONE,
            uploaded: HashSet::new(),
            report: Report {
                hash: FINGERPRINT_SEED,
                ..Report::default()
            },
            pending_text: None,
        }
    }

    /// Run `script` and return the report so far.
    pub fn run(&mut self, script: &Script) -> &Report {
        for step in &script.0 {
            match step {
                Step::Wait(seconds) => {
                    let mut left = *seconds;
                    while left > 0.0 {
                        self.frame_at(left.min(FRAME));
                        left -= FRAME;
                    }
                }
                Step::Move(x, y) => {
                    self.pointer = Pointer {
                        x: *x,
                        y: *y,
                        down: self.pointer.down,
                    };
                    self.engine.set_pointer(self.pointer);
                }
                Step::Press | Step::Release => {
                    self.pointer.down = *step == Step::Press;
                    self.engine.set_pointer(self.pointer);
                }
                Step::Click(x, y) => {
                    for (down, wait) in [(false, 0.0), (true, 0.1), (false, 0.1)] {
                        self.pointer = Pointer { x: *x, y: *y, down };
                        self.engine.set_pointer(self.pointer);
                        let mut left: f32 = wait;
                        while left > 0.0 {
                            self.frame_at(FRAME);
                            left -= FRAME;
                        }
                    }
                }
                Step::Action(action) => self.engine.action(*action),
                Step::Scroll(dx, dy) => self.engine.scroll(*dx, *dy),
                Step::Wheel(x, y, notches) => self.engine.wheel(*x, *y, *notches),
                Step::RightClick(x, y) => {
                    self.engine.press(*x, *y, MouseButton::Secondary, true);
                    self.frame_at(FRAME);
                    self.engine.press(*x, *y, MouseButton::Secondary, false);
                    self.frame_at(FRAME);
                }
                Step::Key(code) => {
                    self.engine.key(*code, true, false);
                    self.frame_at(FRAME);
                    self.engine.key(*code, false, false);
                }
                Step::Text(text) => self.pending_text = text.clone(),
                Step::Draw => self.frame_at(0.0),
            }
        }
        &self.report
    }

    /// The report of everything run so far.
    pub fn report(&self) -> &Report {
        &self.report
    }

    /// One iteration of a port's main loop, with `elapsed` seconds since the last one.
    pub fn frame_at(&mut self, elapsed: f32) {
        // 1. Input has been given to the engine already (the steps above); 2. advance game time.
        self.engine.advance(elapsed);
        // 3. Draw into the port's target.
        let quads = self.engine.draw(&mut self.frame);
        self.report.frames += 1;
        self.report.last_quads = quads;
        self.report.last_vertices = self.frame.vertices.len();
        self.report.last_batches = self.frame.batches.len();
        self.report.hash = self.frame.fingerprint(self.report.hash);
        // 4. Upload what the frame needs: textures once, the atlas whenever it changed.
        let wanted: Vec<u32> = self
            .frame
            .batches
            .iter()
            .map(|batch| batch.tex)
            .filter(|id| *id > TEXTURE_ATLAS)
            .collect();
        for id in wanted {
            if self.uploaded.insert(id) {
                if self.engine.texture(id).is_some() {
                    self.report.textures_uploaded += 1;
                }
                self.engine.release_texture(id);
            }
        }
        if self.engine.atlas_changed() && self.engine.texture(TEXTURE_ATLAS).is_some() {
            self.report.atlas_uploads += 1;
        }
        // 5. Text fields and sounds.
        if self.engine.take_text_request().is_some() {
            self.report.text_requests += 1;
            self.engine.submit_text(self.pending_text.take().as_deref());
        }
        self.report.sounds += self.engine.take_sounds().len() as u64;
    }
}
