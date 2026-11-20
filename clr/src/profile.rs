//! Screen profiles (tasks R2-R5, `docs/LAYOUT-DESIGN.md`): the game keeps drawing into its 960x600 space; a profile cuts that
//! space into regions (player column, page list, game area...) and places each one on the real screen with its own scale.
//! The renderer draws every bound UI subtree with its region's transform; the window maps the pointer back through the same
//! regions, so the game code never knows. Profiles are small text files in `layouts/` (format: `layouts/README.md`).

/// One placed region: game rectangle `src` (x, y, w, h; top-left origin) drawn at `dev = (ox, oy) + game * scale`.
#[derive(Clone, Debug, Default)]
pub struct Region {
    pub name: String,
    pub src: [f32; 4],
    pub scale: f32,
    pub ox: f32,
    pub oy: f32,
    pub visible: bool,
    /// drawn with the region under the game-space mouse (tooltips)
    pub follow: bool,
    /// receives the pointer only while something of it is on screen (pop-ups over everything)
    pub when_drawn: bool,
    /// set by the renderer: one of its subtrees produced quads inside the screen this frame
    pub drawn: bool,
    /// UI nodes (names, at depth 1 or 2 below the root canvas) drawn in this region; `*` = everything not bound elsewhere
    pub nodes: Vec<String>,
}

impl Region {
    pub fn dest(&self) -> [f32; 4] {
        [self.ox + self.src[0] * self.scale, self.oy + self.src[1] * self.scale, self.src[2] * self.scale, self.src[3] * self.scale]
    }
    fn contains_dev(&self, x: f32, y: f32) -> bool {
        let d = self.dest();
        x >= d[0] && y >= d[1] && x < d[0] + d[2] && y < d[1] + d[3]
    }
    fn contains_game(&self, x: f32, y: f32) -> bool {
        x >= self.src[0] && y >= self.src[1] && x < self.src[0] + self.src[2] && y < self.src[1] + self.src[3]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    /// next to the previous `left` regions, scaled to the screen height
    Left,
    /// over the left edge without taking room (shown in place of another region)
    Over,
    /// the remaining width; taller than the screen = vertical pan with the pointer
    Rest,
    /// the whole screen, fitted and centred (today's letterbox)
    Fit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Show {
    Always,
    Hold,
    NotHold,
}

#[derive(Clone, Debug)]
struct Spec {
    name: String,
    src: [f32; 4],
    place: Place,
    show: Show,
    follow: bool,
    when_drawn: bool,
    nodes: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Profile {
    specs: Vec<Spec>,
}

impl Profile {
    /// `region <name> <x> <y> <w> <h> <left|over|rest|fit> [hold|!hold] [follow] [when-drawn] nodes=<name>,<name>...`
    pub fn parse(text: &str) -> Result<Profile, String> {
        let mut specs = vec![];
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let err = |m: &str| format!("layout line {}: {m}", n + 1);
            let (head, nodes) = line.split_once("nodes=").ok_or_else(|| err("missing nodes="))?;
            let w: Vec<&str> = head.split_whitespace().collect();
            if w.len() < 7 || w[0] != "region" {
                return Err(err("expected: region <name> <x> <y> <w> <h> <place> ..."));
            }
            let mut src = [0.0; 4];
            for i in 0..4 {
                src[i] = w[2 + i].parse().map_err(|_| err("bad number"))?;
            }
            let place = match w[6] {
                "left" => Place::Left,
                "over" => Place::Over,
                "rest" => Place::Rest,
                "fit" => Place::Fit,
                _ => return Err(err("place must be left, over, rest or fit")),
            };
            let mut s = Spec { name: w[1].into(), src, place, show: Show::Always, follow: false, when_drawn: false, nodes: vec![] };
            for f in &w[7..] {
                match *f {
                    "hold" => s.show = Show::Hold,
                    "!hold" => s.show = Show::NotHold,
                    "follow" => s.follow = true,
                    "when-drawn" => s.when_drawn = true,
                    _ => return Err(err(&format!("unknown flag {f}"))),
                }
            }
            s.nodes = nodes.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
            specs.push(s);
        }
        if !specs.iter().any(|s| s.nodes.iter().any(|n| n == "*")) {
            return Err("layout: one region must have nodes=*".into());
        }
        Ok(Profile { specs })
    }

    /// Places the regions on a `w` x `h` screen. `hold` = the profile's hold action (Vita L), `py` = pointer y (pans `rest`).
    pub fn place(&self, w: f32, h: f32, hold: bool, py: f32) -> Vec<Region> {
        let mut x = 0.0f32;
        let mut out = vec![];
        for s in &self.specs {
            let visible = match s.show {
                Show::Always => true,
                Show::Hold => hold,
                Show::NotHold => !hold,
            };
            let (scale, ox, oy) = match s.place {
                Place::Left | Place::Over => {
                    let k = h / s.src[3];
                    let dx = if s.place == Place::Over { 0.0 } else { x };
                    if s.place == Place::Left {
                        x += s.src[2] * k;
                    }
                    (k, dx - s.src[0] * k, -s.src[1] * k)
                }
                Place::Rest => {
                    let k = (w - x).max(1.0) / s.src[2];
                    let extra = (s.src[3] * k - h).max(0.0);
                    let pan = extra * (py / h).clamp(0.0, 1.0);
                    (k, x - s.src[0] * k, -s.src[1] * k - pan)
                }
                Place::Fit => {
                    let k = (w / s.src[2]).min(h / s.src[3]);
                    (k, (w - s.src[2] * k) / 2.0 - s.src[0] * k, (h - s.src[3] * k) / 2.0 - s.src[1] * k)
                }
            };
            out.push(Region { name: s.name.clone(), src: s.src, scale, ox, oy, visible, follow: s.follow, when_drawn: s.when_drawn, drawn: false, nodes: s.nodes.clone() });
        }
        out
    }
}

/// Screen point -> game point (top-left origin): the top-most visible region under it (later lines are on top).
pub fn dev_to_game(regions: &[Region], x: f32, y: f32) -> Option<(f32, f32)> {
    regions
        .iter()
        .rev()
        .find(|r| r.visible && !r.follow && (!r.when_drawn || r.drawn) && r.contains_dev(x, y))
        .map(|r| ((x - r.ox) / r.scale, (y - r.oy) / r.scale))
}

/// Region whose game rectangle holds the game point (top-left origin), for tooltips and the focus frame.
pub fn region_at_game(regions: &[Region], x: f32, y: f32) -> Option<usize> {
    (0..regions.len()).rev().find(|&i| {
        let r = &regions[i];
        r.visible && !r.follow && !r.when_drawn && r.contains_game(x, y)
    })
}

/// Built-in profiles (`layouts/*.txt` hold the same text, to read and copy).
pub fn builtin(name: &str) -> Option<&'static str> {
    match name {
        "pc" => Some(include_str!("../../layouts/pc.txt")),
        "vita" => Some(include_str!("../../layouts/vita.txt")),
        _ => None,
    }
}

/// Profile from a built-in name or a file path.
pub fn load(name_or_path: &str) -> Result<Profile, String> {
    let text = match builtin(name_or_path) {
        Some(t) => t.to_string(),
        None => std::fs::read_to_string(name_or_path).map_err(|e| format!("{name_or_path}: {e}"))?,
    };
    Profile::parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_place_and_map_back() {
        // pc = today's letterbox: one fitted region
        let pc = load("pc").unwrap().place(1920.0, 1080.0, false, 0.0);
        let (gx, gy) = dev_to_game(&pc, 960.0, 540.0).unwrap();
        assert!((gx - 480.0).abs() < 0.01 && (gy - 300.0).abs() < 0.01);
        // vita: player column at the left at height scale, game area fills the rest and pans
        let v = load("vita").unwrap();
        let top = v.place(960.0, 544.0, false, 0.0);
        let bottom = v.place(960.0, 544.0, false, 544.0);
        let game = top.iter().position(|r| r.nodes.iter().any(|n| n == "*")).unwrap();
        let d = top[game].dest();
        assert!((d[0] + d[2] - 960.0).abs() < 0.01, "game area reaches the right edge");
        // the pan keeps the mapping onto the whole game height
        assert!(dev_to_game(&top, 900.0, 0.0).unwrap().1.abs() < 0.01);
        assert!((dev_to_game(&bottom, 900.0, 543.99).unwrap().1 - 600.0).abs() < 0.1);
        // hold shows the page list in place of the player column
        let held = v.place(960.0, 544.0, true, 0.0);
        let at = |rs: &[Region]| rs.iter().rev().find(|r| r.visible && r.contains_dev(10.0, 200.0)).map(|r| r.name.clone());
        assert_eq!(at(&top).as_deref(), Some("player"));
        assert_eq!(at(&held).as_deref(), Some("pages"));
    }
}
