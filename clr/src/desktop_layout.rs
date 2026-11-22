//! PC presentation rectangles. Game scripts keep their original RectTransforms.
use crate::ui::{child_xf, xf, Xf};
use crate::unity::{go_of, with_comp, with_go, with_tr, CK};
use crate::value::Value;
use crate::vm::FxMap;
use crate::Vm;
use std::rc::Rc;

const BASE: f32 = 960.0;
const CONTENT_LEFT: f32 = 292.0;

#[derive(Default)]
pub struct DesktopLayout {
    pub pc_resolutions: bool,
    /// console mode (`docs/CONSOLE-MODE.md`): the menu column is hidden and the page takes its width
    pub console: bool,
    /// console mode with L held: the menu list is laid over the player column
    pub console_hold: bool,
    width: f32,
    /// console mode: how far left the page moved (the menu column's width)
    shift: f32,
    canvas: Value,
    rects: FxMap<usize, Xf>,
}

#[derive(Clone, Copy)]
struct Columns {
    left: f32,
    width: f32,
    growth: f32,
    /// horizontal move of the whole table (console mode: the page starts where the menu column was)
    moved: f32,
}

#[derive(Clone, Copy)]
enum Area {
    Flow,
    Table(Columns),
    Title,
    Inventory,
    Adventure,
    Moves,
    Boss,
    Wandoos,
    EdgeBolts,
    Wishes,
    Grid,
    Equipment,
    Bag,
    Slots,
    Fixed,
    /// uniform horizontal stretch of a whole subtree (the console menu list widened to the player column)
    Scale,
}

impl DesktopLayout {
    pub fn set_width(&mut self, width: f32) {
        self.width = if width.is_finite() && width > BASE + 0.01 { width } else { 0.0 };
        if self.width == 0.0 {
            self.rects.clear();
        }
    }

    pub fn width(&self) -> f32 {
        self.width.max(BASE)
    }

    pub(crate) fn rect(&self, tr: &Value) -> Option<Xf> {
        self.rects.get(&id(tr)?).copied()
    }

    fn rebuild(&mut self, vm: &Vm) {
        self.rects.clear();
        let extra = self.width.max(BASE) - BASE;
        if extra <= 0.0 && !self.console {
            return;
        }
        if self.canvas.is_null() {
            self.canvas = vm.eng.gos.iter().find_map(|go| {
                with_go(go, |g| (g.name == "Canvas").then(|| g.transform.clone())).flatten()
            }).unwrap_or_default();
        }
        let canvas = xf(vm, &self.canvas);
        // the menu column and the player column: panels inside full-screen canvases
        let panel = |canvas_name: &str, panel_name: &str| {
            let root = children(&self.canvas).into_iter().find(|root| with_go(&go_of(root), |g| g.name == canvas_name).unwrap_or(false))?;
            let root_rect = child_rect(&canvas, &root)?;
            let node = children(&root).into_iter().find(|n| with_go(&go_of(n), |g| g.name == panel_name).unwrap_or(false))?;
            let rect = child_rect(&root_rect, &node)?;
            Some((node, rect))
        };
        let features = panel("Features Canvas", "Features Panel");
        // console mode: the page starts where the menu column started
        let shift = if self.console { features.as_ref().map_or(0.0, |(_, f)| (CONTENT_LEFT - left(*f)).max(0.0)) } else { 0.0 };
        self.shift = shift;
        if self.console_hold {
            if let (Some((node, f)), Some((_, player))) = (features, panel("Player Panel Canvas", "Player Panel")) {
                self.place(&node, f, horizontal(f, left(player), width(player)), Area::Scale);
            }
        }
        for root in children(&self.canvas) {
            let Some(original) = child_rect(&canvas, &root) else { continue };
            let label = with_go(&go_of(&root), |g| g.name.clone()).unwrap_or_default();
            if label == "Box Canvas" {
                for overlay in children(&root) {
                    let Some(x) = child_rect(&original, &overlay) else { continue };
                    let name = with_go(&go_of(&overlay), |g| g.name.clone()).unwrap_or_default();
                    // The renderer places hover help after text layout.
                    if name.starts_with("Hover Tooltip") {
                        continue;
                    }
                    if !visible(x) {
                        continue;
                    }
                    let full = left(x).abs() < 40.0 && width(x) >= 890.0;
                    let target = if full {
                        horizontal(x, left(x), width(x) + extra)
                    } else {
                        horizontal(x, left(x) + extra / 2.0, width(x))
                    };
                    let area = if name == "Main Menu Screen" { Area::Title } else { Area::Flow };
                    self.place(&overlay, x, target, area);
                }
            } else if content_panel(original) {
                let area = if label == "Inventory Menu Canvas" { Area::Inventory } else { Area::Flow };
                self.place(&root, original, horizontal(original, left(original) - shift, width(original) + extra + shift), area);
            } else if label == "Test Canvas 2" || label == "Train Attack Menu Canvas" {
                // The input strip and initial training panel are placed independently of their parent.
                for child in children(&root) {
                    let Some(x) = child_rect(&original, &child) else { continue };
                    if content_panel(x) {
                        self.place(&child, x, horizontal(x, left(x) - shift, width(x) + extra + shift), Area::Flow);
                    }
                }
            } else if label.starts_with("End Panel") || label == "Tutorial Panel" {
                if visible(original) && width(original) >= 890.0 && left(original).abs() < 40.0 {
                    self.place(&root, original, horizontal(original, left(original), width(original) + extra), Area::Flow);
                }
            }
        }
    }

    fn place(&mut self, tr: &Value, source: Xf, target: Xf, area: Area) {
        let Some((area, training_viewport)) = with_go(&go_of(tr), |g| (g.active && !g.port_hidden).then(|| (match g.name.as_str() {
            "Attack Panel" | "Defense Panel" | "Augments Menu" | "Blood Magic Menu" |
            "Advanced Training Menu" | "NGU Menu + All NGU" | "NGU Menu" if width(target) > width(source) => {
                Area::Table(Columns {
                    left: left(source),
                    width: width(source),
                    growth: width(target) - width(source),
                    moved: left(target) - left(source),
                })
            }
            "Adventure Menu" => Area::Adventure,
            "Boss Menu" => Area::Boss,
            "Wandoos 98 Menu" => Area::Wandoos,
            "Wishes Menu" => Area::Wishes,
            _ if matches!(area, Area::Flow) && width(target) > width(source) && g.comps.iter().any(|c| {
                with_comp(c, |c| c.enabled && matches!(&c.k, CK::Layout { kind, .. } if kind == "GridLayoutGroup"))
                    .unwrap_or(false)
            }) => Area::Grid,
            _ => area,
        }, g.name == "Train Attack Menu + Scripts"))).flatten() else {
            return;
        };
        if let Some(key) = id(tr) {
            self.rects.insert(key, target);
        }
        for child in children(tr) {
            let Some((x, next, child_area)) = with_go(&go_of(&child), |g| {
                if !g.active || g.port_hidden {
                    return None;
                }
                let x = child_rect(&source, &child)?;
                let (mut next, child_area) = position(&child, g.name.as_str(), &g.comps, area, source, target, x, self.width.max(BASE) - BASE + self.shift);
                next.origin[1] += target.origin[1] - source.origin[1];
                if training_viewport && g.name == "Contents" {
                    // Basic Training's full-height scroll content has no scroll range.
                    // Keep it inside its mask even if the original scene supplied an offset.
                    let view_h = target.h * target.scale[1].abs();
                    let content_h = next.h * next.scale[1].abs();
                    if view_h > 0.0 && (content_h - view_h).abs() < 0.5 {
                        next.origin[1] = target.origin[1] + next.pivot[1] * content_h - target.pivot[1] * view_h;
                    }
                }
                Some((x, next, child_area))
            }).flatten() else { continue };
            self.place(&child, x, next, child_area);
        }
    }
}

pub fn pc_resolution(w: i32, h: i32) -> (i32, i32) {
    match (w, h) {
        (960, 600) | (1280, 800) | (1440, 900) | (1680, 1050) => (w, w * 9 / 16),
        _ => (w, h),
    }
}

pub(crate) fn pc_resolution_label(label: &str) -> Option<String> {
    let (w, h) = label.split_once(" * ")?;
    let (w, h) = (w.parse().ok()?, h.parse().ok()?);
    let (_, height) = pc_resolution(w, h);
    (height != h).then(|| format!("{w} * {height}"))
}

pub(crate) fn rebuild(vm: &mut Vm) {
    if vm.eng.desktop_layout.width <= BASE && !vm.eng.desktop_layout.console {
        return;
    }
    let mut desktop = std::mem::take(&mut vm.eng.desktop_layout);
    desktop.rebuild(vm);
    vm.eng.desktop_layout = desktop;
}

fn id(tr: &Value) -> Option<usize> {
    match tr {
        Value::Obj(o) => Some(Rc::as_ptr(o) as usize),
        _ => None,
    }
}

fn children(tr: &Value) -> Vec<Value> {
    with_tr(tr, |t| t.children.clone()).unwrap_or_default()
}

fn child_rect(parent: &Xf, tr: &Value) -> Option<Xf> {
    with_tr(tr, |t| child_xf(parent, t))
}

fn width(x: Xf) -> f32 {
    x.w * x.scale[0].abs()
}

fn left(x: Xf) -> f32 {
    x.origin[0] - x.pivot[0] * width(x)
}

fn center(x: Xf) -> f32 {
    left(x) + width(x) / 2.0
}

fn horizontal(mut x: Xf, left: f32, w: f32) -> Xf {
    if x.scale[0] != 0.0 {
        x.w = w / x.scale[0].abs();
        x.origin[0] = left + x.pivot[0] * w;
    }
    x
}

fn visible(x: Xf) -> bool {
    let h = x.h * x.scale[1].abs();
    let y = x.origin[1] - x.pivot[1] * h;
    width(x) > 1.0 && h > 1.0 && left(x) < BASE && left(x) + width(x) > 0.0 && y < 600.0 && y + h > 0.0
}

fn content_panel(x: Xf) -> bool {
    let right = left(x) + width(x);
    visible(x) && left(x) >= CONTENT_LEFT - 10.0 && width(x) >= 640.0 && (927.0..=995.0).contains(&right)
}

fn position(tr: &Value, name: &str, comps: &[Value], area: Area, source: Xf, target: Xf, x: Xf, extra: f32) -> (Xf, Area) {
    let delta = left(target) - left(source);
    let grow = width(target) - width(source);
    let keep = |at: f32| horizontal(x, left(x) + delta + at, width(x));
    let stretch = |at: f32, amount: f32| horizontal(x, left(x) + delta + at, width(x) + amount);
    if name.to_ascii_lowercase().starts_with("ghost") {
        return (x, Area::Fixed);
    }
    match area {
        Area::Title => {
            let target = match name {
                "Title (1)" => stretch(0.0, grow),
                "Kartridge Button" | "Kong Button" | "By me" => keep(grow),
                "Desktop Home Options" => return (keep(grow / 2.0), Area::Fixed),
                "Load File Button" | "Start New Game" | "Brian" | "Chicken" => keep(grow / 2.0),
                "Autosave Pod Border" => stretch(0.0, grow * 0.30),
                "Cloud Save Border" => stretch(grow * 0.70, grow * 0.30),
                "LeftPic" => keep(grow * 0.15),
                "RightPic" => keep(grow * 0.85),
                _ => keep(0.0),
            };
            (target, Area::Flow)
        }
        Area::Inventory => match name {
            "Inventory Menu" => (stretch(0.0, grow), Area::Inventory),
            "Current Equipment" => (stretch(0.0, grow), Area::Equipment),
            "Inventory" if width(x) > 600.0 => (stretch(0.0, grow), Area::Bag),
            _ => (flow(name, comps, source, target, x), Area::Flow),
        },
        Area::Equipment => match name {
            "Equipment Display" | "Scrollrect" => (stretch(grow * 0.65, grow * 0.35), Area::Flow),
            "Stats Breakdown" => (keep(grow * 0.55), Area::Fixed),
            name if name.contains("Button") => (keep(grow * 0.55), Area::Fixed),
            _ => (keep(grow * 0.14), Area::Fixed),
        },
        Area::Bag => match name {
            "Equipment Text (3)" => (keep(grow / 2.0), Area::Fixed),
            "Equipment Text (1)" => (keep(grow), Area::Fixed),
            "Inventory" => (keep(0.0), Area::Slots),
            _ => (keep(0.0), Area::Fixed),
        },
        Area::Adventure if name == "Moves Panel" => (stretch(0.0, grow), Area::Moves),
        Area::Moves => {
            // The three authored rows mix stretched anchors and fixed rectangles.
            let column = ((center(x) - center(source)) / 105.0 + 2.5).round().clamp(0.0, 5.0);
            let offset = center(source) + (column - 2.5) * 105.0 - center(x);
            (keep(offset + grow * column / 5.0), Area::Fixed)
        }
        Area::Boss => match name {
            "Player Portrait Frame" | "Player Name" | "Back Button" | "Forward Button" =>
                (keep(grow * 0.12), Area::Fixed),
            "Player HP" => (keep(grow * 0.12 - (center(x) - center(source) + 192.0)), Area::Fixed),
            "Boss Portrait Frame" | "Boss Name" | "Boss Info" | "BadgeButton" =>
                (keep(grow * 0.88), Area::Fixed),
            "Boss HP Bar + Script" => (keep(grow * 0.88 - (center(x) - center(source) - 192.0)), Area::Fixed),
            _ => (flow(name, comps, source, target, x), Area::Flow),
        },
        Area::Wandoos if name == "Controller" => (stretch(0.0, grow), Area::EdgeBolts),
        Area::EdgeBolts if name.starts_with("Middle Bolt") => {
            let shift = if center(x) > center(source) { grow } else { 0.0 };
            (keep(shift), Area::Fixed)
        }
        Area::Wishes if name == "Slots" => (stretch(0.0, grow), Area::Grid),
        Area::Grid => {
            let span = width(source) - width(x);
            let fraction = if span > 0.0 { ((left(x) - left(source)) / span).clamp(0.0, 1.0) } else { 0.0 };
            (keep(grow * fraction), Area::Fixed)
        }
        Area::Slots if name.starts_with("Slot (") => {
            let column = ((left(x) - 326.0) / 50.0).round().rem_euclid(12.0);
            (keep(extra * column / 11.0), Area::Fixed)
        }
        Area::Fixed | Area::Slots => (keep(0.0), Area::Fixed),
        Area::Scale => {
            let r = width(target) / width(source).max(1e-3);
            (horizontal(x, left(target) + (left(x) - left(source)) * r, width(x) * r), Area::Scale)
        }
        Area::Table(columns) => {
            if width(x) >= columns.width * 0.65 || name.starts_with("Middle Bolt") {
                return (flow(name, comps, source, target, x), Area::Table(columns));
            }
            // In the scene, some rows report only the size of their first cell.
            // Use the table coordinates for the other cells and the column headings.
            let (min, max) = children(tr).iter().filter_map(|ch| child_rect(&x, ch)).fold(
                (left(x), left(x) + width(x)),
                |(start, end), cell| (start.min(left(cell)), end.max(left(cell) + width(cell))),
            );
            let at = (left(x) + width(x) / 2.0 - columns.left) / columns.width;
            let next = if max - min > columns.width * 0.65 { Area::Table(columns) } else { Area::Fixed };
            (horizontal(x, left(x) + columns.moved + columns.growth * at.clamp(0.0, 1.0), width(x)), next)
        }
        _ => (flow(name, comps, source, target, x), Area::Flow),
    }
}

fn flow(name: &str, comps: &[Value], source: Xf, target: Xf, x: Xf) -> Xf {
    let w = width(source);
    let extra = width(target) - w;
    let offset = left(x) - left(source);
    let child_w = width(x);
    let at = left(target) + offset;
    if extra.abs() < 0.01 {
        return horizontal(x, at, child_w);
    }
    let mut image = false;
    let mut sliced = false;
    let mut text = false;
    let mut control = false;
    for comp in comps {
        with_comp(comp, |c| match &c.k {
            CK::Image(i) | CK::RawImage(i) if c.enabled => {
                image = !i.sprite.is_null();
                sliced |= i.typ == 1 && with_comp(&i.sprite, |s| {
                    matches!(&s.k, CK::Sprite(s) if s.border.iter().any(|b| *b > 0.0))
                }).unwrap_or(false);
            }
            CK::Text(_) => text = true,
            CK::Sel(_) => control = true,
            _ => {}
        });
    }
    let full = w > 1.0 && child_w >= w * 0.90 && offset.abs() < 20.0;
    let fits = w > 1.0 && offset >= -16.0 && offset + child_w <= w + 16.0;
    let h = x.h * x.scale[1].abs();
    let compact_art = image && name.ends_with("Frame") && h >= child_w * 0.72 && h <= child_w * 1.5;
    let expand = fits && x.rot.abs() < 0.001 && !control && !compact_art && child_w >= 72.0 &&
        (sliced || name == "Pit Image" || !image && (full || child_w >= w * 0.65 || text && child_w >= 145.0));
    if expand {
        let fraction = if full { 0.0 } else { (offset / w).clamp(0.0, 1.0) };
        let gain = if full { extra } else { extra * child_w / w };
        return horizontal(x, at + fraction * extra, child_w + gain);
    }
    let fraction = if w > child_w + 1.0 {
        (offset / (w - child_w)).clamp(0.0, 1.0)
    } else {
        0.5
    };
    horizontal(x, at + fraction * extra, child_w)
}
