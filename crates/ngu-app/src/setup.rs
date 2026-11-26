//! The setup window: finds the game, asks the player when needed, prepares it.

use crate::art::Art;
use crate::browse::Browser;
use crate::canvas::*;
use crate::gpu::{view, Gpu};
use miniquad::{window, EventHandler, KeyCode, KeyMods, MouseButton};
use ngu_prep::dirs::Os;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const INK: [f32; 4] = [0.08, 0.08, 0.13, 1.0];
const WARN: [f32; 4] = [0.65, 0.1, 0.1, 1.0];
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

/// Progress shared with the preparation thread.
#[derive(Default)]
pub struct Job {
    pub text: String,
    pub result: Option<Result<(), String>>,
}

/// Everything the setup window knows, apart from graphics.
pub struct Model {
    pub screen: Screen,
    /// Why the setup is shown (empty on the very first launch).
    pub reason: String,
    pub root: PathBuf,
    pub home: Option<PathBuf>,
    pub offset: usize,
    pub job: Arc<Mutex<Job>>,
    pub play: bool,
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
            screen: Screen::Intro,
            reason,
            root,
            home,
            offset: 0,
            job: Arc::default(),
            play: false,
        }
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
            Action::UseGame(path) => self.start(path),
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
                if ngu_prep::data_dir(&dir).is_ok() {
                    self.start(dir);
                } else {
                    let message =
                        "This folder is not NGU Idle. Open the folder that contains NGU IDLE.exe."
                            .to_owned();
                    self.screen = Screen::Browse(Browser::open(&dir), Some(message));
                }
            }
            Action::Back => self.screen = Screen::Intro,
            Action::Play => self.play = true,
        }
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

    /// Draw the current screen and return the clickable areas.
    pub fn view(&self, c: &mut Canvas, mouse: (f32, f32), time: f32) -> Vec<Hit> {
        c.image(TEX_BACKDROP, [0.0, 0.0, WIDTH, HEIGHT], [1.0; 4]);
        c.fill([0.0, 0.0, WIDTH, HEIGHT], [0.0, 0.0, 0.0, 0.55]);
        c.nine(
            TEX_FRAME,
            200.0,
            26.0,
            [130.0, 40.0, 830.0, 560.0],
            [1.0; 4],
        );
        c.text_centered(Face::Title, 40, crate::NAME, WIDTH / 2.0, 62.0, INK);
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
                    (Face::Title, 24)
                };
                c.text_centered(
                    face,
                    size,
                    label,
                    (rect[0] + rect[2]) / 2.0,
                    rect[1] + (rect[3] - rect[1]) / 2.0 - size as f32 * 0.6,
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
        match &self.screen {
            Screen::Intro => {
                c.image(TEX_CAT, [330.0, 120.0, 630.0, 320.0], [1.0; 4]);
                let first = if self.reason.is_empty() {
                    "Welcome! NGU Idle must be set up once before you can play.".to_owned()
                } else {
                    format!(
                        "{} Choose your NGU Idle folder again to set it up.",
                        self.reason
                    )
                };
                body(c, &first, 340.0, INK);
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
                    body(c, "NGU Idle was not found in the usual Steam places. Please choose its folder.", 150.0, INK);
                } else if games.len() == 1 {
                    body(c, "Found NGU Idle here:", 150.0, INK);
                    body(c, &short(&games[0]), 190.0, INK);
                    button(
                        c,
                        [300.0, 270.0, 660.0, 325.0],
                        "Use this game",
                        Action::UseGame(games[0].clone()),
                        &mut hits,
                    );
                } else {
                    body(c, "Several copies were found. Choose one:", 130.0, INK);
                    for (i, game) in games.iter().take(5).enumerate() {
                        let y = 175.0 + i as f32 * 62.0;
                        button(
                            c,
                            [180.0, y, 780.0, y + 52.0],
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
                body(c, &place, 112.0, INK);
                if let Some(problem) = problem {
                    body(c, problem, 140.0, WARN);
                }
                for (row, entry) in browser
                    .entries
                    .iter()
                    .skip(self.offset)
                    .take(ROWS)
                    .enumerate()
                {
                    let y = 198.0 + row as f32 * 37.0;
                    let name = entry.file_name().map_or_else(
                        || entry.to_string_lossy().into_owned(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    button(
                        c,
                        [180.0, y, 780.0, y + 34.0],
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
                body(
                    c,
                    "Preparing your game. This takes about a minute, please wait.",
                    190.0,
                    INK,
                );
                let text = self.job.lock().unwrap().text.clone();
                body(c, &text, 260.0, INK);
                c.fill([230.0, 340.0, 730.0, 366.0], [0.15, 0.15, 0.2, 1.0]);
                let x = 230.0 + ((time * 0.8).fract()) * 380.0;
                c.fill([x, 344.0, x + 120.0, 362.0], [0.95, 0.75, 0.1, 1.0]);
            }
            Screen::Failed(message) => {
                body(c, "Something went wrong:", 140.0, WARN);
                body(c, message, 190.0, INK);
                button(
                    c,
                    [340.0, 470.0, 620.0, 525.0],
                    "Back",
                    Action::Back,
                    &mut hits,
                );
            }
            Screen::Done => {
                c.image(TEX_CAT, [330.0, 120.0, 630.0, 320.0], [1.0; 4]);
                body(c, "Everything is ready. Have fun!", 350.0, INK);
                button(
                    c,
                    [340.0, 440.0, 620.0, 495.0],
                    "Play",
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
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).spawn();
            }
            window::quit();
        }
    }

    fn draw(&mut self) {
        let time = self.started.elapsed().as_secs_f32();
        let mut canvas = Canvas::new(&mut self.atlas, &self.fonts);
        self.hits = self.model.view(&mut canvas, self.mouse, time);
        let quads = std::mem::take(&mut canvas.quads);
        self.gpu.draw(&quads, &mut self.atlas);
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
            KeyCode::Escape => self.model.apply(Action::Back),
            KeyCode::Enter | KeyCode::KpEnter => {
                if let Some(first) = self.hits.first().map(|h| h.action.clone()) {
                    if matches!(
                        first,
                        Action::FindGame | Action::Play | Action::UseGame(_) | Action::Back
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
    let conf = miniquad::conf::Conf {
        window_title: crate::NAME.to_owned(),
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
            model: Model::new(root, reason),
            art,
            atlas: Atlas::default(),
            fonts,
            gpu,
            hits: Vec::new(),
            mouse: (-1.0, -1.0),
            started: std::time::Instant::now(),
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
    fn long_paths_are_shortened_from_the_left() {
        let long = PathBuf::from("a".repeat(100));
        let text = short(&long);
        assert!(text.starts_with("...") && text.chars().count() == 58);
    }
}
