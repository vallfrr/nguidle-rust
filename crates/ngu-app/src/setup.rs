//! The setup window: finds the game, asks the player when needed, prepares it.

use crate::art::Art;
use crate::canvas::*;
use crate::gpu::{view, Gpu};
use miniquad::{window, EventHandler, KeyCode, KeyMods, MouseButton};
use ngu_prep::dirs::Os;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const INK: [f32; 4] = [0.04, 0.04, 0.08, 1.0];
/// The fill colour of the game's energy bar.
const BAR_GREEN: [f32; 4] = [0.34, 0.81, 0.5, 1.0];
const SHADOW: [f32; 4] = [0.05, 0.1, 0.18, 1.0];

/// What a click on a widget does.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    FindGame,
    UseGame(PathBuf),
    /// Open the system's folder chooser.
    Pick,
    Back,
    Play,
}

/// The screen being shown.
pub enum Screen {
    Intro,
    Found(Vec<PathBuf>),
    /// A folder the player chose, and whether the game can be used from there.
    Chosen(PathBuf, Result<(), String>),
    /// Vita: waiting for the output folder to be chosen.
    Output,
    Working,
    Failed(String),
    Done,
}

/// A clickable area of the current frame.
pub struct Hit {
    pub rect: [f32; 4],
    pub action: Action,
}

/// Which program the window belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// NGUIdleRustEngine: prepares the game for this computer, then plays it.
    Pc,
    /// nguidle-prep: writes the `nguidle` folder for a PS Vita.
    Vita,
}

impl Kind {
    /// Title of the window and heading of the panel.
    pub fn title(self) -> &'static str {
        match self {
            Kind::Pc => "NGU Idle Rust Engine",
            Kind::Vita => "NGU Idle Vita Prep",
        }
    }
}

/// Progress shared with the preparation thread.
#[derive(Default)]
pub struct Job {
    pub text: String,
    pub result: Option<Result<(), String>>,
}

/// Everything the setup window knows, apart from graphics.
pub struct Model {
    pub kind: Kind,
    /// The game chosen for a Vita preparation, while the output folder is being chosen.
    pub game: Option<PathBuf>,
    pub choosing_output: bool,
    /// Where the Vita folder was written.
    pub output: Option<PathBuf>,
    pub screen: Screen,
    /// Why the setup is shown (empty on the very first launch).
    pub reason: String,
    pub root: PathBuf,
    pub home: Option<PathBuf>,
    pub job: Arc<Mutex<Job>>,
    pub play: bool,
    /// Set when the folder chooser must be opened by the window.
    pub pick: bool,
}

/// How far along the preparation is, from the step it reports.
fn progress(step: &str) -> f32 {
    match step {
        s if s.starts_with("Checking") => 0.1,
        s if s.starts_with("Copying") => 0.3,
        s if s.starts_with("Preparing") => 0.6,
        s if s.starts_with("Building") => 0.9,
        _ => 0.05,
    }
}

/// Whether the game can be used from `dir`, in words for the player.
fn check_location(dir: &Path) -> Result<(), String> {
    let data =
        ngu_prep::data_dir(dir).map_err(|_| "NGU Idle was not found in this folder.".to_owned())?;
    if ngu_prep::manifest()
        .keys()
        .any(|name| !data.join(name).is_file())
    {
        return Err("Some game files are missing in this folder.".to_owned());
    }
    ngu_prep::prepare::require_player(&data)
        .map_err(|_| "UnityPlayer.dll is missing: choose the NGU IDLE folder.".to_owned())?;
    Ok(())
}

fn short(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    if text.chars().count() > 58 {
        let tail: String = text
            .chars()
            .rev()
            .take(55)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("...{tail}")
    } else {
        text
    }
}

impl Model {
    pub fn new(root: PathBuf, reason: String) -> Model {
        let home =
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        Model {
            kind: Kind::Pc,
            game: None,
            choosing_output: false,
            output: None,
            screen: Screen::Intro,
            reason,
            root,
            home,
            job: Arc::default(),
            play: false,
            pick: false,
        }
    }

    /// The model of the Vita preparation program.
    pub fn vita() -> Model {
        let home =
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
        let root = ngu_prep::dirs::data_root(Os::current(), &home.unwrap_or_default(), |name| {
            std::env::var(name).ok()
        })
        .with_file_name("NGUIdleVitaPrep");
        let mut model = Model::new(root, String::new());
        model.kind = Kind::Vita;
        model
    }

    /// Vita: the game is chosen, now where to put the `nguidle` folder.
    fn choose_output(&mut self, game: PathBuf) {
        self.game = Some(game);
        self.choosing_output = true;
        self.screen = Screen::Output;
    }

    /// Games found in the usual places.
    pub fn detect(&self) -> Vec<PathBuf> {
        let home = self.home.clone().unwrap_or_default();
        let roots =
            ngu_prep::locate::steam_roots(Os::current(), &home, |name| std::env::var(name).ok());
        ngu_prep::locate::find_games(&roots)
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::FindGame => self.screen = Screen::Found(self.detect()),
            Action::UseGame(path) => match self.kind {
                Kind::Pc => self.start(path),
                Kind::Vita => self.choose_output(path),
            },
            Action::Pick => self.pick = true,
            Action::Back => {
                self.choosing_output = false;
                self.screen = Screen::Intro;
            }
            Action::Play => self.play = true,
        }
    }

    /// What the folder chooser returned: the folder is checked and shown, nothing starts yet.
    pub fn picked(&mut self, folder: Option<PathBuf>) {
        let Some(folder) = folder else { return };
        if self.choosing_output {
            // Vita: the chosen folder receives the nguidle folder, so the job starts at once.
            if let Some(game) = self.game.clone() {
                self.start_vita(game, folder);
            }
        } else {
            let verdict = check_location(&folder);
            self.screen = Screen::Chosen(folder, verdict);
        }
    }

    /// The title of the folder chooser for what is being asked.
    pub fn pick_title(&self) -> &'static str {
        if self.choosing_output {
            "Choose where to create the nguidle folder"
        } else {
            "Choose the NGU Idle folder"
        }
    }

    /// Where the folder chooser should open: the Steam games folder when there is one.
    pub fn pick_start(&self) -> Option<PathBuf> {
        if self.choosing_output {
            return self.home.clone();
        }
        let home = self.home.clone().unwrap_or_default();
        let roots =
            ngu_prep::locate::steam_roots(Os::current(), &home, |name| std::env::var(name).ok());
        roots
            .into_iter()
            .map(|root| root.join("steamapps/common"))
            .find(|path| path.is_dir())
            .or_else(|| self.home.clone())
    }

    /// Runs the Vita preparation on a thread: the `nguidle` folder goes into `output`.
    fn start_vita(&mut self, game: PathBuf, output: PathBuf) {
        self.screen = Screen::Working;
        self.choosing_output = false;
        self.output = Some(output.join("nguidle"));
        let job = Arc::new(Mutex::new(Job {
            text: "Starting...".into(),
            result: None,
        }));
        self.job = job.clone();
        std::thread::spawn(move || {
            let progress = job.clone();
            let result = ngu_prep::prepare::prepare_vita(&game, &output, &mut |text| {
                progress.lock().unwrap().text = text.to_owned()
            })
            .map(|_| ())
            .map_err(|e| e.to_string());
            job.lock().unwrap().result = Some(result);
        });
    }

    fn start(&mut self, game: PathBuf) {
        self.screen = Screen::Working;
        let job = Arc::new(Mutex::new(Job {
            text: "Starting...".into(),
            result: None,
        }));
        self.job = job.clone();
        let root = self.root.clone();
        std::thread::spawn(move || {
            let progress = job.clone();
            let result = ngu_prep::prepare::prepare_pc(&game, &root, &mut |text| {
                progress.lock().unwrap().text = text.to_owned()
            })
            .map(|_| ())
            .map_err(|e| e.to_string());
            job.lock().unwrap().result = Some(result);
        });
    }

    /// Moves a finished preparation to the next screen.
    pub fn poll(&mut self) {
        if !matches!(self.screen, Screen::Working) {
            return;
        }
        let done = self.job.lock().unwrap().result.take();
        match done {
            Some(Ok(())) => self.screen = Screen::Done,
            Some(Err(message)) => self.screen = Screen::Failed(message),
            None => {}
        }
    }

    /// The last click: installs the program and its desktop shortcut, then (PC) starts the game.
    pub fn finish(&self) {
        let (stem, icon_from) = match self.kind {
            Kind::Pc => ("NGUIdleRustEngine", ngu_prep::dirs::game_dir(&self.root)),
            Kind::Vita => ("nguidle-prep", self.game.clone().unwrap_or_default()),
        };
        let spec = crate::install::Spec {
            root: &self.root,
            stem,
            label: self.kind.title(),
            icon: Some(icon_from.join("NGUIdle.exe")),
        };
        let exe = crate::install::install(&spec);
        if self.kind == Kind::Pc {
            let _ = std::process::Command::new(exe).spawn();
        }
    }

    /// Draw the current screen and return the clickable areas.
    pub fn view(&self, c: &mut Canvas, mouse: (f32, f32), time: f32) -> Vec<Hit> {
        let _ = time;
        c.image(TEX_BACKDROP, [0.0, 0.0, WIDTH, HEIGHT], [1.0; 4]);
        c.fill([0.0, 0.0, WIDTH, HEIGHT], [0.0, 0.0, 0.0, 0.45]);
        // The game's own blue panel and bolted frame.
        c.nine_scaled(
            TEX_PANEL,
            50.0,
            4.0,
            8.0,
            [134.0, 44.0, 826.0, 556.0],
            [1.0; 4],
        );
        c.nine(
            TEX_FRAME,
            200.0,
            20.0,
            [130.0, 40.0, 830.0, 560.0],
            [1.0; 4],
        );
        let heading = self.kind.title();
        c.text_centered(Face::Title, 44, heading, WIDTH / 2.0 + 2.0, 66.0, SHADOW);
        c.text_centered(Face::Title, 44, heading, WIDTH / 2.0, 64.0, [1.0; 4]);
        let mut hits = Vec::new();
        // The game's sprites have a 4 pixel border; drawn twice as big they stay crisp.
        let sprite = |c: &mut Canvas, tex: u32, rect: [f32; 4], tint: [f32; 4]| {
            c.nine_scaled(tex, 50.0, 4.0, 8.0, rect, tint);
        };
        let button =
            |c: &mut Canvas, rect: [f32; 4], label: &str, action: Action, hits: &mut Vec<Hit>| {
                let inside = mouse.0 >= rect[0]
                    && mouse.0 <= rect[2]
                    && mouse.1 >= rect[1]
                    && mouse.1 <= rect[3];
                let tint = if inside {
                    [1.0, 0.92, 0.6, 1.0]
                } else {
                    [1.0; 4]
                };
                sprite(c, TEX_BUTTON, rect, tint);
                // Folder names and paths read better in the plain font.
                let plain = label.contains(['/', '\\']);
                let (face, size) = if plain {
                    (Face::Body, 20)
                } else {
                    (Face::Title, 26)
                };
                c.text_middle(
                    face,
                    size,
                    label,
                    (rect[0] + rect[2]) / 2.0,
                    (rect[1] + rect[3]) / 2.0,
                    INK,
                );
                hits.push(Hit { rect, action });
            };
        let body = |c: &mut Canvas, text: &str, y: f32, colour: [f32; 4]| {
            let mut y = y;
            for line in c.wrap(Face::Body, 20, text, 560.0) {
                c.text_centered(Face::Body, 20, &line, WIDTH / 2.0, y, colour);
                y += 26.0;
            }
        };
        // A white field, like the game's input fields.
        let field = |c: &mut Canvas, text: &str, y: f32, colour: [f32; 4]| {
            sprite(c, TEX_BUTTON, [180.0, y, 780.0, y + 40.0], [1.0; 4]);
            c.text_middle(Face::Body, 20, text, WIDTH / 2.0, y + 20.0, colour);
        };
        // A coloured label: green for good news, red for bad, like the game's own buttons.
        let badge = |c: &mut Canvas, tex: u32, text: &str, y: f32| {
            sprite(c, tex, [190.0, y, 770.0, y + 46.0], [1.0; 4]);
            c.text_middle(Face::Title, 26, text, WIDTH / 2.0, y + 23.0, INK);
        };
        match &self.screen {
            Screen::Intro => {
                c.image(TEX_CAT, [330.0, 110.0, 630.0, 310.0], [1.0; 4]);
                let first = if self.kind == Kind::Vita {
                    "This prepares NGU Idle for your PS Vita: it writes a folder named nguidle to copy next to the nguidle.vpk.".to_owned()
                } else if self.reason.is_empty() {
                    "Welcome! NGU Idle must be set up once before you can play.".to_owned()
                } else {
                    format!(
                        "{} Choose your NGU Idle folder again to set it up.",
                        self.reason
                    )
                };
                body(c, &first, 330.0, INK);
                body(
                    c,
                    "Your original game and your saves are only read, never changed.",
                    404.0,
                    INK,
                );
                button(
                    c,
                    [340.0, 460.0, 620.0, 516.0],
                    "Find my game",
                    Action::FindGame,
                    &mut hits,
                );
            }
            Screen::Found(games) if games.len() == 1 => {
                badge(c, TEX_GOOD, "Found NGU Idle on your computer!", 150.0);
                field(c, &short(&games[0]), 214.0, INK);
                button(
                    c,
                    [300.0, 320.0, 660.0, 376.0],
                    "Use this game",
                    Action::UseGame(games[0].clone()),
                    &mut hits,
                );
                button(
                    c,
                    [300.0, 400.0, 660.0, 456.0],
                    "Choose a folder",
                    Action::Pick,
                    &mut hits,
                );
            }
            Screen::Found(games) if games.is_empty() => {
                badge(c, TEX_BAD, "NGU Idle was not found", 150.0);
                body(
                    c,
                    "It is not in the usual Steam places. Please choose its folder.",
                    230.0,
                    INK,
                );
                button(
                    c,
                    [300.0, 320.0, 660.0, 376.0],
                    "Choose a folder",
                    Action::Pick,
                    &mut hits,
                );
            }
            Screen::Found(games) => {
                badge(c, TEX_GOOD, "Several copies were found. Choose one:", 130.0);
                for (i, game) in games.iter().take(4).enumerate() {
                    let y = 200.0 + i as f32 * 58.0;
                    button(
                        c,
                        [180.0, y, 780.0, y + 48.0],
                        &short(game),
                        Action::UseGame(game.clone()),
                        &mut hits,
                    );
                }
                button(
                    c,
                    [300.0, 450.0, 660.0, 506.0],
                    "Choose a folder",
                    Action::Pick,
                    &mut hits,
                );
            }
            Screen::Chosen(path, verdict) => {
                match verdict {
                    Ok(()) => badge(c, TEX_GOOD, "This location is valid", 150.0),
                    Err(_) => badge(c, TEX_BAD, "This location is not valid", 150.0),
                }
                field(c, &short(path), 214.0, INK);
                if let Err(why) = verdict {
                    body(c, why, 272.0, INK);
                } else {
                    button(
                        c,
                        [300.0, 320.0, 660.0, 376.0],
                        "Use this game",
                        Action::UseGame(path.clone()),
                        &mut hits,
                    );
                }
                button(
                    c,
                    [300.0, 400.0, 660.0, 456.0],
                    "Choose a folder",
                    Action::Pick,
                    &mut hits,
                );
            }
            Screen::Output => {
                badge(c, TEX_GOOD, "Where should the nguidle folder go?", 150.0);
                body(
                    c,
                    "A folder named nguidle will be created in the place you choose, ready to copy to your PS Vita.",
                    230.0,
                    INK,
                );
                button(
                    c,
                    [300.0, 320.0, 660.0, 376.0],
                    "Choose a folder",
                    Action::Pick,
                    &mut hits,
                );
            }
            Screen::Working => {
                badge(c, TEX_GOOD, "Preparing your game...", 150.0);
                body(c, "This takes about a minute, please wait.", 226.0, INK);
                let text = self.job.lock().unwrap().text.clone();
                field(c, &text, 280.0, INK);
                // The game's bar: a white box with a coloured fill, the same pictures.
                sprite(c, TEX_BUTTON, [230.0, 350.0, 730.0, 394.0], [1.0; 4]);
                let done = progress(&text);
                c.image(
                    TEX_FILL,
                    [238.0, 358.0, 238.0 + 484.0 * done, 386.0],
                    BAR_GREEN,
                );
            }
            Screen::Failed(message) => {
                badge(c, TEX_BAD, "Something went wrong", 130.0);
                let mut y = 200.0;
                for line in c.wrap(Face::Body, 20, message, 560.0).into_iter().take(5) {
                    field(c, &line, y, INK);
                    y += 46.0;
                }
                button(
                    c,
                    [340.0, 450.0, 620.0, 506.0],
                    "Back",
                    Action::Back,
                    &mut hits,
                );
            }
            Screen::Done => {
                c.image(TEX_CAT, [330.0, 100.0, 630.0, 300.0], [1.0; 4]);
                if self.kind == Kind::Vita {
                    let place = self.output.as_deref().map_or_else(String::new, short);
                    badge(c, TEX_GOOD, "Done!", 312.0);
                    body(
                        c,
                        "Copy this folder into ux0:data on your PS Vita:",
                        374.0,
                        INK,
                    );
                    field(c, &place, 402.0, INK);
                } else {
                    badge(c, TEX_GOOD, "Everything is ready. Have fun!", 312.0);
                    if !cfg!(target_os = "macos") {
                        body(
                            c,
                            "A shortcut named NGU Idle Rust Engine is added to your desktop.",
                            374.0,
                            INK,
                        );
                    }
                }
                button(
                    c,
                    [340.0, 456.0, 620.0, 512.0],
                    if self.kind == Kind::Vita {
                        "Close"
                    } else {
                        "Play"
                    },
                    Action::Play,
                    &mut hits,
                );
            }
        }
        hits
    }

    /// The action of the topmost widget at `at`.
    pub fn click(&mut self, hits: &[Hit], at: (f32, f32)) -> bool {
        let hit = hits.iter().rev().find(|h| {
            at.0 >= h.rect[0] && at.0 <= h.rect[2] && at.1 >= h.rect[1] && at.1 <= h.rect[3]
        });
        match hit {
            Some(hit) => {
                self.apply(hit.action.clone());
                true
            }
            None => false,
        }
    }

    /// The Escape key: one step back on screens where that makes sense, never while working.
    pub fn back(&mut self) {
        if matches!(
            self.screen,
            Screen::Found(_) | Screen::Chosen(..) | Screen::Output | Screen::Failed(_)
        ) {
            self.choosing_output = false;
            self.screen = Screen::Intro;
        }
    }
}

struct App {
    model: Model,
    art: Art,
    atlas: Atlas,
    fonts: Fonts,
    gpu: Gpu,
    hits: Vec<Hit>,
    mouse: (f32, f32),
    started: std::time::Instant,
    atlas_scale: f32,
}

impl App {
    fn logical(&self, x: f32, y: f32) -> (f32, f32) {
        let (w, h) = window::screen_size();
        let (s, ox, oy) = view(w, h);
        ((x - ox) / s, (y - oy) / s)
    }
}

impl EventHandler for App {
    fn update(&mut self) {
        self.model.poll();
        if self.model.pick {
            self.model.pick = false;
            let chosen =
                crate::dialog::pick_folder(self.model.pick_title(), self.model.pick_start());
            self.model.picked(chosen);
        }
        if self.model.play {
            self.model.finish();
            window::quit();
        }
    }

    fn draw(&mut self) {
        let time = self.started.elapsed().as_secs_f32();
        let (w, h) = window::screen_size();
        let scale = (view(w, h).0 * 8.0).round() / 8.0;
        if scale != self.atlas_scale {
            // Text is rasterised for the current window size, so it stays sharp.
            self.atlas = Atlas::default();
            self.atlas_scale = scale;
        }
        let mut canvas = Canvas::new(&mut self.atlas, &self.fonts).with_scale(scale);
        self.hits = self.model.view(&mut canvas, self.mouse, time);
        let quads = std::mem::take(&mut canvas.quads);
        self.gpu.draw(&quads, &mut self.atlas);
    }

    /// The window keeps the view's proportions and takes only the sizes in `sizes::SIZES`.
    fn resize_event(&mut self, width: f32, height: f32) {
        // On Windows the drag itself is held to the allowed sizes; here the rest is corrected
        // once the size is known (other systems, maximising).
        let (w, h) = if cfg!(windows) {
            (width as u32, (width * HEIGHT / WIDTH).round() as u32)
        } else {
            crate::sizes::nearest(width)
        };
        if (w as f32 - width).abs() > 1.0 || (h as f32 - height).abs() > 1.0 {
            window::set_window_size(w, h);
        }
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        self.mouse = self.logical(x, y);
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button == MouseButton::Left {
            let at = self.logical(x, y);
            self.model.click(&self.hits, at);
        }
    }

    fn key_down_event(&mut self, key: KeyCode, _mods: KeyMods, _repeat: bool) {
        match key {
            KeyCode::Escape => self.model.back(),
            KeyCode::Enter | KeyCode::KpEnter => {
                if let Some(first) = self.hits.first().map(|h| h.action.clone()) {
                    if matches!(
                        first,
                        Action::FindGame
                            | Action::Play
                            | Action::UseGame(_)
                            | Action::Pick
                            | Action::Back
                    ) {
                        self.model.apply(first);
                    }
                }
            }
            _ => {}
        }
        let _ = &self.art;
    }
}

/// Opens the setup window. `reason` explains why it is shown (empty on a first launch).
pub fn run(root: PathBuf, reason: String) {
    let mut model = Model::new(root, reason);
    model.kind = Kind::Pc;
    open(model);
}

/// Opens the Vita preparation window.
pub fn run_vita() {
    open(Model::vita());
}

fn open(model: Model) {
    let conf = miniquad::conf::Conf {
        window_title: model.kind.title().to_owned(),
        window_width: 960,
        window_height: 600,
        window_resizable: true,
        high_dpi: cfg!(windows),
        sample_count: 1,
        platform: miniquad::conf::Platform {
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };
    let title = conf.window_title.clone();
    miniquad::start(conf, move || {
        crate::sizes::lock_while_dragging(&title);
        let art = Art::load();
        let gpu = Gpu::new(&art);
        let fonts = Fonts {
            title: Art::load().title,
            body: Art::load().body,
        };
        Box::new(App {
            model,
            art,
            atlas: Atlas::default(),
            fonts,
            gpu,
            hits: Vec::new(),
            mouse: (-1.0, -1.0),
            started: std::time::Instant::now(),
            atlas_scale: 1.0,
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> (Model, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (Model::new(dir.path().join("app"), String::new()), dir)
    }

    fn draw(model: &Model) -> Vec<Hit> {
        let art = Art::load();
        let fonts = Fonts {
            title: art.title,
            body: art.body,
        };
        let mut atlas = Atlas::default();
        let mut canvas = Canvas::new(&mut atlas, &fonts);
        model.view(&mut canvas, (-1.0, -1.0), 0.0)
    }

    #[test]
    fn first_screen_offers_to_find_the_game_and_a_click_moves_on() {
        let (mut model, _dir) = model();
        let hits = draw(&model);
        assert_eq!(hits.len(), 1);
        let rect = hits[0].rect;
        assert!(model.click(
            &hits,
            ((rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0)
        ));
        assert!(matches!(model.screen, Screen::Found(_)));
    }

    #[test]
    fn a_chosen_folder_is_checked_and_only_a_valid_one_offers_to_use_it() {
        let (mut model, dir) = model();
        // Cancelling the chooser changes nothing.
        model.picked(None);
        assert!(matches!(model.screen, Screen::Intro));
        // A folder that is not the game is called not valid, with a reason, and offers no start.
        model.picked(Some(dir.path().to_path_buf()));
        let Screen::Chosen(_, Err(why)) = &model.screen else {
            panic!("expected a refusal")
        };
        assert!(why.contains("not found"));
        let hits = draw(&model);
        assert!(hits.iter().all(|h| !matches!(h.action, Action::UseGame(_))));
        assert!(hits.iter().any(|h| h.action == Action::Pick));
        // A game folder without the player library is refused too.
        let data = dir.path().join("NGUIdle_Data");
        for name in ngu_prep::manifest().keys() {
            let file = data.join(name);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, b"x").unwrap();
        }
        model.picked(Some(dir.path().to_path_buf()));
        assert!(
            matches!(&model.screen, Screen::Chosen(_, Err(why)) if why.contains("UnityPlayer"))
        );
        // With every file it is valid and offers to use it.
        std::fs::write(dir.path().join("UnityPlayer.dll"), b"x").unwrap();
        model.picked(Some(dir.path().to_path_buf()));
        assert!(matches!(model.screen, Screen::Chosen(_, Ok(()))));
        assert!(draw(&model)
            .iter()
            .any(|h| matches!(h.action, Action::UseGame(_))));
        // Choosing a folder asks the window for the system chooser.
        model.apply(Action::Pick);
        assert!(model.pick);
    }

    #[test]
    fn a_failed_preparation_shows_its_message_and_back_returns_to_the_start() {
        let (mut model, dir) = model();
        model.screen = Screen::Working;
        model.job.lock().unwrap().result = Some(Err("broken".into()));
        model.poll();
        assert!(matches!(&model.screen, Screen::Failed(m) if m == "broken"));
        assert!(!draw(&model).is_empty());
        model.apply(Action::Back);
        assert!(matches!(model.screen, Screen::Intro));
        drop(dir);
    }

    #[test]
    fn using_a_missing_folder_fails_without_touching_anything() {
        let (mut model, dir) = model();
        model.apply(Action::UseGame(dir.path().join("nothing")));
        for _ in 0..200 {
            model.poll();
            if !matches!(model.screen, Screen::Working) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(matches!(model.screen, Screen::Failed(_)));
        assert!(!dir.path().join("app").exists());
    }

    #[test]
    fn escape_goes_back_but_never_interrupts_the_preparation() {
        let (mut model, _dir) = model();
        model.screen = Screen::Working;
        model.back();
        assert!(matches!(model.screen, Screen::Working));
        model.screen = Screen::Found(vec![]);
        model.back();
        assert!(matches!(model.screen, Screen::Intro));
        model.screen = Screen::Done;
        model.back();
        assert!(matches!(model.screen, Screen::Done));
    }

    #[test]
    fn the_vita_program_asks_for_the_game_then_for_the_output_folder() {
        let mut model = Model::vita();
        assert_eq!(model.kind.title(), "NGU Idle Vita Prep");
        assert!(!draw(&model).is_empty());
        model.apply(Action::UseGame(PathBuf::from("some game")));
        assert!(model.choosing_output && matches!(model.screen, Screen::Output));
        assert_eq!(model.game.as_deref(), Some(Path::new("some game")));
        assert!(draw(&model).iter().any(|h| h.action == Action::Pick));
        assert!(model.pick_title().contains("nguidle"));
        // Cancelling the chooser changes nothing.
        model.picked(None);
        assert!(matches!(model.screen, Screen::Output));
        // Choosing the output starts the job; a game that is not there fails clearly.
        let dir = tempfile::tempdir().unwrap();
        model.picked(Some(dir.path().to_path_buf()));
        for _ in 0..200 {
            model.poll();
            if !matches!(model.screen, Screen::Working) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(matches!(model.screen, Screen::Failed(_)));
        assert!(!dir.path().join("nguidle").exists());
        model.back();
        model.apply(Action::Back);
        assert!(!model.choosing_output && matches!(model.screen, Screen::Intro));
    }

    #[test]
    fn long_paths_are_shortened_from_the_left() {
        let long = PathBuf::from("a".repeat(100));
        let text = short(&long);
        assert!(text.starts_with("...") && text.chars().count() == 58);
    }
}
