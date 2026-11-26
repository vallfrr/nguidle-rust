//! The setup window: finds the game, asks the player when needed, prepares it.

use crate::art::Art;
use crate::browse::Browser;
use crate::canvas::*;
use crate::gpu::{view, Gpu};
use miniquad::{window, EventHandler, KeyCode, KeyMods, MouseButton};
use ngu_prep::dirs::Os;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const INK: [f32; 4] = [0.04, 0.04, 0.08, 1.0];
const WARN: [f32; 4] = [0.7, 0.05, 0.05, 1.0];
/// The game's steel-blue panel colour.
const PANEL: [f32; 4] = [0.4, 0.53, 0.64, 1.0];
const GREEN: [f32; 4] = [0.6, 0.92, 0.25, 1.0];
const SHADOW: [f32; 4] = [0.05, 0.1, 0.18, 1.0];
const ROWS: usize = 8;

/// What a click on a widget does.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    FindGame,
    UseGame(PathBuf),
    Browse,
    Open(PathBuf),
    Up,
    UseFolder,
    Back,
    Play,
}

/// The screen being shown.
pub enum Screen {
    Intro,
    Found(Vec<PathBuf>),
    Browse(Browser, Option<String>),
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
    pub offset: usize,
    pub job: Arc<Mutex<Job>>,
    pub play: bool,
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
            offset: 0,
            job: Arc::default(),
            play: false,
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
        self.offset = 0;
        self.screen = Screen::Browse(Browser::at_roots(self.home.clone()), None);
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
            Action::Browse => {
                self.offset = 0;
                self.screen = Screen::Browse(Browser::at_roots(self.home.clone()), None);
            }
            Action::Open(path) => {
                self.offset = 0;
                self.screen = Screen::Browse(Browser::open(&path), None);
            }
            Action::Up => {
                if let Screen::Browse(browser, _) = &self.screen {
                    self.offset = 0;
                    self.screen = Screen::Browse(browser.up(self.home.clone()), None);
                }
            }
            Action::UseFolder => {
                let Screen::Browse(browser, _) = &self.screen else {
                    return;
                };
                let Some(dir) = browser.dir.clone() else {
                    return;
                };
                if self.choosing_output {
                    if let Some(game) = self.game.clone() {
                        self.start_vita(game, dir);
                    }
                } else if ngu_prep::data_dir(&dir).is_ok() {
                    match self.kind {
                        Kind::Pc => self.start(dir),
                        Kind::Vita => self.choose_output(dir),
                    }
                } else {
                    let message =
                        "This folder is not NGU Idle: its game files are missing.".to_owned();
                    self.screen = Screen::Browse(Browser::open(&dir), Some(message));
                }
            }
            Action::Back => {
                self.choosing_output = false;
                self.screen = Screen::Intro;
            }
            Action::Play => self.play = true,
        }
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
        c.image(TEX_BACKDROP, [0.0, 0.0, WIDTH, HEIGHT], [1.0; 4]);
        c.fill([0.0, 0.0, WIDTH, HEIGHT], [0.0, 0.0, 0.0, 0.45]);
        // The game's own steel-blue panel, inside its bolted frame.
        c.fill([134.0, 44.0, 826.0, 556.0], PANEL);
        c.nine(
            TEX_FRAME,
            200.0,
            26.0,
            [130.0, 40.0, 830.0, 560.0],
            [1.0; 4],
        );
        let heading = self.kind.title();
        c.text_centered(Face::Title, 44, heading, WIDTH / 2.0 + 2.0, 66.0, SHADOW);
        c.text_centered(Face::Title, 44, heading, WIDTH / 2.0, 64.0, [1.0; 4]);
        let mut hits = Vec::new();
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
                c.nine(TEX_BUTTON, 50.0, 14.0, rect, tint);
                // Folder names and paths read better in the plain font.
                let plain = matches!(action, Action::Open(_)) || label.contains(['/', '\\']);
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
        // A white field with a black border, like the game's input fields.
        let field = |c: &mut Canvas, text: &str, y: f32, colour: [f32; 4]| {
            c.boxed([190.0, y, 770.0, y + 36.0], [1.0; 4]);
            c.text_middle(Face::Body, 20, text, WIDTH / 2.0, y + 18.0, colour);
        };
        // A green label, like the game's "WTF do I do?" button.
        let badge = |c: &mut Canvas, text: &str, y: f32| {
            c.boxed([250.0, y, 710.0, y + 40.0], GREEN);
            c.text_middle(Face::Title, 24, text, WIDTH / 2.0, y + 20.0, INK);
        };
        match &self.screen {
            Screen::Intro => {
                c.image(TEX_CAT, [330.0, 120.0, 630.0, 320.0], [1.0; 4]);
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
                body(c, &first, 336.0, INK);
                body(
                    c,
                    "Your original game and your saves are only read, never changed.",
                    410.0,
                    INK,
                );
                button(
                    c,
                    [340.0, 470.0, 620.0, 525.0],
                    "Find my game",
                    Action::FindGame,
                    &mut hits,
                );
            }
            Screen::Found(games) => {
                if games.is_empty() {
                    body(c, "NGU Idle was not found in the usual Steam places. Please choose its folder.", 190.0, INK);
                } else if games.len() == 1 {
                    badge(c, "Found NGU Idle on your computer!", 150.0);
                    field(c, &short(&games[0]), 210.0, INK);
                    button(
                        c,
                        [300.0, 290.0, 660.0, 345.0],
                        "Use this game",
                        Action::UseGame(games[0].clone()),
                        &mut hits,
                    );
                } else {
                    badge(c, "Several copies were found. Choose one:", 130.0);
                    for (i, game) in games.iter().take(5).enumerate() {
                        let y = 190.0 + i as f32 * 58.0;
                        button(
                            c,
                            [180.0, y, 780.0, y + 48.0],
                            &short(game),
                            Action::UseGame(game.clone()),
                            &mut hits,
                        );
                    }
                }
                button(
                    c,
                    [300.0, 480.0, 660.0, 535.0],
                    "Choose a folder",
                    Action::Browse,
                    &mut hits,
                );
            }
            Screen::Browse(browser, problem) => {
                let place = browser
                    .dir
                    .as_deref()
                    .map_or_else(|| "Choose a place".to_owned(), short);
                if self.choosing_output {
                    badge(c, "Where should the nguidle folder go?", 100.0);
                    field(c, &place, 146.0, INK);
                } else {
                    field(c, &place, 110.0, INK);
                }
                if let Some(problem) = problem {
                    field(c, problem, 184.0, WARN);
                }
                for (row, entry) in browser
                    .entries
                    .iter()
                    .skip(self.offset)
                    .take(ROWS)
                    .enumerate()
                {
                    let y = 228.0 + row as f32 * 33.0;
                    let name = entry.file_name().map_or_else(
                        || entry.to_string_lossy().into_owned(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    let name = if name.chars().count() > 60 {
                        format!("{}...", name.chars().take(57).collect::<String>())
                    } else {
                        name
                    };
                    button(
                        c,
                        [180.0, y, 780.0, y + 30.0],
                        &name,
                        Action::Open(entry.clone()),
                        &mut hits,
                    );
                }
                button(c, [150.0, 505.0, 330.0, 550.0], "Up", Action::Up, &mut hits);
                if browser.dir.is_some() {
                    button(
                        c,
                        [570.0, 505.0, 810.0, 550.0],
                        "Use this folder",
                        Action::UseFolder,
                        &mut hits,
                    );
                }
            }
            Screen::Working => {
                badge(c, "Preparing your game...", 170.0);
                body(c, "This takes about a minute, please wait.", 236.0, INK);
                let text = self.job.lock().unwrap().text.clone();
                field(c, &text, 300.0, INK);
                // The game's own bar: a white box with a black border and a green fill.
                let done = progress(&text);
                let pulse = 0.9 + 0.1 * (time * 4.0).sin();
                c.boxed([230.0, 370.0, 730.0, 410.0], [1.0; 4]);
                c.fill(
                    [232.0, 372.0, 232.0 + 496.0 * done, 408.0],
                    [0.35 * pulse, 0.8 * pulse, 0.4 * pulse, 1.0],
                );
            }
            Screen::Failed(message) => {
                badge(c, "Something went wrong", 140.0);
                let mut y = 210.0;
                for line in c.wrap(Face::Body, 20, message, 540.0).into_iter().take(6) {
                    field(c, &line, y, WARN);
                    y += 42.0;
                }
                button(
                    c,
                    [340.0, 470.0, 620.0, 525.0],
                    "Back",
                    Action::Back,
                    &mut hits,
                );
            }
            Screen::Done => {
                c.image(TEX_CAT, [330.0, 110.0, 630.0, 310.0], [1.0; 4]);
                if self.kind == Kind::Vita {
                    let place = self.output.as_deref().map_or_else(String::new, short);
                    badge(c, "Done!", 320.0);
                    body(
                        c,
                        "Copy this folder into ux0:data on your PS Vita:",
                        372.0,
                        INK,
                    );
                    field(c, &place, 404.0, INK);
                } else {
                    badge(c, "Everything is ready. Have fun!", 330.0);
                    if !cfg!(target_os = "macos") {
                        body(
                            c,
                            "A shortcut named NGU Idle Rust Engine is added to your desktop.",
                            385.0,
                            INK,
                        );
                    }
                }
                button(
                    c,
                    [340.0, 460.0, 620.0, 515.0],
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
            Screen::Found(_) | Screen::Browse(..) | Screen::Failed(_)
        ) {
            self.choosing_output = false;
            self.screen = Screen::Intro;
        }
    }

    pub fn scroll(&mut self, notches: f32) {
        if let Screen::Browse(browser, _) = &self.screen {
            let last = browser.entries.len().saturating_sub(ROWS);
            self.offset = if notches > 0.0 {
                self.offset.saturating_sub(1)
            } else {
                (self.offset + 1).min(last)
            };
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

    /// The window keeps the view's proportions: its size changes in both directions or not at all.
    fn resize_event(&mut self, width: f32, height: f32) {
        let wanted = (width * HEIGHT / WIDTH).round();
        if (wanted - height).abs() > 1.0 {
            window::set_window_size(width as u32, wanted as u32);
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

    fn mouse_wheel_event(&mut self, _x: f32, y: f32) {
        self.model.scroll(y);
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
                            | Action::Browse
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
    miniquad::start(conf, move || {
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
    fn a_folder_that_is_not_the_game_is_refused_with_a_message() {
        let (mut model, dir) = model();
        model.screen = Screen::Browse(Browser::open(dir.path()), None);
        model.apply(Action::UseFolder);
        let Screen::Browse(_, Some(message)) = &model.screen else {
            panic!("expected a message")
        };
        assert!(message.contains("not NGU Idle"));
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
        assert!(model.choosing_output && matches!(model.screen, Screen::Browse(..)));
        assert_eq!(model.game.as_deref(), Some(Path::new("some game")));
        // Choosing the output starts the job; a game that is not there fails clearly.
        let dir = tempfile::tempdir().unwrap();
        model.screen = Screen::Browse(Browser::open(dir.path()), None);
        model.apply(Action::UseFolder);
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
