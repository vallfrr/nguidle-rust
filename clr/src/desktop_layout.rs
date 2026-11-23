//! PC presentation rectangles. Game scripts keep their original RectTransforms.
use crate::ui::{child_xf, xf, Xf};
use crate::unity::{go_of, with_comp, with_go, with_tr, SelKind, CK};
use crate::value::Value;
use crate::vm::FxMap;
use crate::Vm;
use std::{collections::HashSet, rc::Rc};

const BASE: f32 = 960.0;
const CONTENT_LEFT: f32 = 292.0;

#[derive(Default)]
pub struct DesktopLayout {
    pub pc_resolutions: bool,
    /// console mode (`docs/CONSOLE-MODE.md`): the menu column is hidden and the page takes its width
    pub console: bool,
    /// console mode with L held: the menu list is laid over the player column
    pub console_hold: bool,
    /// console mode: game rows shown on screen (the rest of the 600 rows is zoomed out of view)
    pub console_view_h: f32,
    /// A welcome, progress or story screen owns the full viewport.
    pub(crate) console_modal: bool,
    /// console mode: what scrolls after the last rebuild (only what did not fit), and every scroll offset (game rows),
    /// kept across rebuilds by the scroll block's transform or the left-hand menu list
    pub regions: Vec<ScrollRegion>,
    pub pans: FxMap<usize, f32>,
    /// console mode: nodes whose children are clipped to a rectangle (x0, y0, x1, y1; game rows, y down): a scrolling page's
    /// frame stays put and only its inside slides
    clips: FxMap<usize, ScrollClip>,
    width: f32,
    /// console mode: how far left the page moved (the menu column's width)
    shift: f32,
    canvas: Value,
    rects: FxMap<usize, Xf>,
    text_fit: HashSet<usize>,
}

/// Something that scrolls in console mode. `view` is on screen (x0, y0, x1, y1; rows, y down); `extra` rows are hidden
/// by the console squeeze. A game scroll view (`native`, its ScrollRect) also scrolls by itself over `native_range`.
#[derive(Clone)]
pub struct ScrollRegion {
    pub key: usize,
    pub view: [f32; 4],
    pub extra: f32,
    pub native: Value,
    pub native_range: f32,
    pub native_off: f32,
    pub track: [f32; 4],
    content: f32,
}

#[derive(Clone, Copy)]
struct ScrollClip {
    bounds: [f32; 4],
    rows: Option<&'static str>,
}

struct ScrollBlock {
    node: Value,
    rect: Xf,
    rows: Option<&'static str>,
    header: f32,
}

fn scrolls_with_block(go: &Value, rows: Option<&str>) -> bool {
    with_go(go, |g| {
        !g.name.starts_with("Middle Bolt") && rows.is_none_or(|prefix| g.name.starts_with(prefix))
    }).unwrap_or(false)
}

impl ScrollRegion {
    pub(crate) fn grip(&self, pan: f32) -> (f32, f32) {
        let [x0, top, x1, bottom] = self.track;
        let h = bottom - top;
        let length = (h * (self.view[3] - self.view[1]) / self.content).clamp((x1 - x0).min(h), h);
        let extent = (self.extra + self.native_range).max(0.01);
        let start = top + (h - length) * ((self.native_off + pan) / extent).clamp(0.0, 1.0);
        (start, length)
    }
}

pub const COLUMN: usize = 2;
pub const LIST: usize = 3;
/// rows kept around content, between two contents, at the frame edges, and the shortest a scroll view gets
const PAD: f32 = 1.0;
const GAP: f32 = 6.0;
const TIGHT_GAP: f32 = 2.0;
const EDGE_GAP: f32 = 14.0;
const SETTINGS_EDGE_GAP: f32 = 6.0;
const LIST_MIN: f32 = 110.0;
/// share of a page's rows that may be pressed together rather than scrolled
const SQUEEZE: f32 = 0.12;

fn framed(go: &Value) -> bool {
    with_go(go, |g| g.comps.iter().any(|c| {
        with_comp(c, |c| match &c.k {
            CK::Image(i) if c.enabled && i.typ == 1 => with_comp(&i.sprite, |s| {
                matches!(&s.k, CK::Sprite(sprite) if sprite.border.iter().any(|edge| *edge > 0.0))
            }).unwrap_or(false),
            _ => false,
        }).unwrap_or(false)
    })).unwrap_or(false)
}

pub(crate) fn scroll_bar(go: &Value) -> bool {
    with_go(go, |g| g.comps.iter().any(|c| {
        with_comp(c, |c| matches!(&c.k, CK::Sel(s) if s.kind == SelKind::Scrollbar)).unwrap_or(false)
    })).unwrap_or(false)
}

fn frame_corner(go: &Value) -> bool {
    with_go(go, |g| g.name.starts_with("Middle Bolt")).unwrap_or(false)
}

/// The game's vertical scroll views, by the transform that clips them (viewport, else the ScrollRect itself).
fn scroll_views(vm: &Vm) -> FxMap<usize, Value> {
    let mut out = FxMap::default();
    for comp in &vm.eng.scrollrects {
        let vp = with_comp(comp, |c| match &c.k {
            CK::ScrollRect { viewport, content, vertical, .. } if c.enabled && *vertical && !content.is_null() =>
                Some(if viewport.is_null() { with_go(&c.go, |g| g.transform.clone()).unwrap_or_default() } else { viewport.clone() }),
            _ => None,
        }).flatten();
        if let Some(k) = vp.as_ref().and_then(id) {
            out.insert(k, comp.clone());
        }
    }
    out
}

/// How far a game scroll view has scrolled by itself (rows).
pub fn native_offset(vm: &Vm, comp: &Value) -> f32 {
    native_scroll(vm, comp).1
}

/// A game scroll view's own scroll state (range and offset in rows).
fn native_scroll(vm: &Vm, comp: &Value) -> (f32, f32) {
    let Some((content, viewport, go)) = with_comp(comp, |c| match &c.k {
        CK::ScrollRect { content, viewport, .. } => Some((content.clone(), viewport.clone(), c.go.clone())),
        _ => None,
    }).flatten() else { return (0.0, 0.0) };
    let vp = if viewport.is_null() { with_go(&go, |g| g.transform.clone()).unwrap_or_default() } else { viewport };
    let (cx, vx) = (xf(vm, &content), xf(vm, &vp));
    let (ch, vh) = (cx.h * cx.scale[1].abs(), vx.h * vx.scale[1].abs());
    let top = |x: Xf, h: f32| x.origin[1] + (1.0 - x.pivot[1]) * h;
    let range = (ch - vh).max(0.0);
    let off = (top(cx, ch) - top(vx, vh)).clamp(0.0, range);
    (range, off)
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
        // console mode zooms in: the logical width can be smaller than the game's 960 (the menu column's room is reused)
        let min = if self.console { 0.0 } else { BASE + 0.01 };
        self.width = if width.is_finite() && width > min { width } else { 0.0 };
        if self.width == 0.0 {
            self.rects.clear();
        }
    }

    /// Changes whenever a console scroll offset changes (cached quads of the subtrees below a fixed frame depend on it).
    pub fn pan_stamp(&self) -> f32 {
        self.pans.values().sum::<f32>() + self.pans.len() as f32 * 0.001 + f32::from(u8::from(self.console))
    }

    pub(crate) fn clip_scroll_child(&self, parent: &Value, child: &Value) -> Option<[f32; 4]> {
        let clip = self.clips.get(&id(parent)?)?;
        scrolls_with_block(&go_of(child), clip.rows).then_some(clip.bounds)
    }

    pub(crate) fn content_left(&self) -> f32 {
        CONTENT_LEFT - self.shift
    }

    pub(crate) fn fits_text(&self, tr: &Value) -> bool {
        id(tr).is_some_and(|key| self.text_fit.contains(&key))
    }

    pub(crate) fn reset_console(&mut self) {
        self.console = false;
        self.console_hold = false;
        self.console_view_h = 0.0;
        self.console_modal = false;
        self.shift = 0.0;
        self.rects.clear();
        self.clips.clear();
        self.text_fit.clear();
        self.pans.clear();
        self.regions.clear();
    }

    pub fn width(&self) -> f32 {
        if self.console && self.width > 0.0 { self.width } else { self.width.max(BASE) }
    }

    pub(crate) fn rect(&self, tr: &Value) -> Option<Xf> {
        self.rects.get(&id(tr)?).copied()
    }

    fn rebuild(&mut self, vm: &Vm) {
        self.rects.clear();
        self.clips.clear();
        self.text_fit.clear();
        self.shift = 0.0;
        self.console_modal = false;
        let extra = self.width() - BASE;
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
        let player = panel("Player Panel Canvas", "Player Panel");
        let (features_v, player_v) = (features.clone(), player.clone());
        // console mode: the page starts where the menu column started
        let shift = if self.console { features.as_ref().map_or(0.0, |(_, f)| (CONTENT_LEFT - left(*f)).max(0.0)) } else { 0.0 };
        self.shift = shift;
        if self.console_hold {
            if let (Some((node, f)), Some((_, player))) = (features.clone(), player.clone()) {
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
        if self.console && self.console_view_h > 0.0 {
            self.console_vertical(vm, canvas, features_v, player_v);
        }
    }

    /// Console mode, after the horizontal layout: fits each column and the page into the rows shown on screen.
    /// Order of the squeeze: empty bands first, then the game's own scroll views (a skill list, the augment list...);
    /// only what still does not fit scrolls, with a scrollbar. At 100 % nothing moves.
    fn console_vertical(&mut self, vm: &Vm, canvas: Xf, features: Option<(Value, Xf)>, player: Option<(Value, Xf)>) {
        let v = self.console_view_h.min(600.0);
        self.regions.clear();
        let views = scroll_views(vm);
        let right = self.width();
        let roots: Vec<(String, Value, Xf)> = children(&self.canvas).into_iter().filter_map(|root| {
            let name = with_go(&go_of(&root), |g| (g.active && !g.port_hidden).then(|| g.name.clone())).flatten()?;
            let x = self.now(&root, &canvas)?;
            Some((name, root, x))
        }).collect();
        let boxes = roots.iter().find(|(n, _, _)| n == "Box Canvas");
        let fills_screen = |at: Xf| visible(at) && left(at).abs() < 40.0 &&
            width(at) >= 800.0 && (vertical_bounds(at).0).abs() < 40.0;
        let full_screen = boxes.and_then(|(_, root, x)| {
            children(root).into_iter().find_map(|node| {
                let is_screen = with_go(&go_of(&node), |g| g.active && !g.port_hidden &&
                    matches!(g.name.as_str(), "Main Menu Screen" | "Splash Screen" | "Start Panel")).unwrap_or(false);
                let at = self.now(&node, x)?;
                (is_screen && fills_screen(at)).then_some((node, at))
            })
        }).or_else(|| roots.iter().find(|(name, _, at)| name.starts_with("End Panel") && fills_screen(*at))
            .map(|(_, node, at)| (node.clone(), *at)));
        if let Some((screen, at)) = full_screen.filter(|_| !crate::save_slots::covers_game_content(vm)) {
            self.console_modal = true;
            if v < 599.5 {
                self.prepare_story(&screen, at);
            }
            let has_notes = with_go(&go_of(&screen), |g| g.name == "Splash Screen").unwrap_or(false);
            self.fit(vm, &[(screen, at)], v, &views, has_notes);
            return;
        }
        // the player column, with the bolts on its section joints
        if let Some((node, rect)) = player.clone().filter(|_| !self.console_hold) {
            let at = id(&node).and_then(|k| self.rects.get(&k).copied()).unwrap_or(rect);
            let mut parts = vec![(node, at)];
            for (_, bolt, x) in roots.iter().filter(|(n, _, x)| n.starts_with("Middle Bolt") && left(*x) < 200.0) {
                let y = 600.0 - x.origin[1];
                if y < 30.0 || y > 570.0 {
                    // The fixed corner follows the frame edge, even when the column's contents squeeze.
                    let mut fixed = *x;
                    if y > 570.0 {
                        fixed.origin[1] += 600.0 - v;
                    }
                    self.rects.insert(id(bolt).unwrap(), fixed);
                } else {
                    parts.push((bolt.clone(), *x));
                }
            }
            self.fit(vm, &parts, v, &views, false);
        }
        // the menu list (L held): widened to the column and as much taller, scrolled to the cursor
        if let Some((node, f)) = features.filter(|_| self.console_hold) {
            let at = id(&node).and_then(|k| self.rects.get(&k).copied()).unwrap_or(f);
            let r = width(at) / width(f).max(1.0);
            let top = 600.0 - (f.origin[1] + (1.0 - f.pivot[1]) * f.h * f.scale[1].abs());
            let total = f.h * f.scale[1].abs() * r;
            let extra = (total - v).max(0.0);
            let pan = self.pan(LIST, extra);
            self.remap(&node, at, &move |y| top + (y - top) * r - pan, 0.0);
            let x1 = left(at) + width(at);
            self.region(LIST, [left(at), 0.0, x1, v], extra, Value::Null, 0.0, 0.0, [x1 - 16.0, 8.0, x1 - 6.0, v - 8.0], total);
        }
        if let Some((panel, shade)) = crate::save_slots::content_panels(vm) {
            let left = self.content_left();
            let at = horizontal(xf(vm, &panel), left, right - left);
            let shade_rect = horizontal(xf(vm, &shade), 0.0, left);
            self.rects.insert(id(&shade).unwrap(), vertical(shade_rect, 0.0, v));
            self.fit(vm, &[(panel, at)], v, &views, false);
            return;
        }
        // the page: every visible root except the columns and the pop-ups
        // (the game shows a menu by moving its panel onto the menu anchor: the canvas itself may stay parked)
        let mut page: Vec<(Value, Xf)> = vec![];
        for (n, r, x) in &roots {
            if matches!(n.as_str(), "Player Panel Canvas" | "Features Canvas" | "Small Menu Anchor" | "Big Menu Anchor" | "Box Canvas")
                || n.starts_with("Middle Bolt") {
                continue;
            }
            if visible(*x) {
                page.push((r.clone(), *x));
            } else {
                page.extend(children(r).into_iter().filter_map(|c| self.now(&c, x).filter(|cx| visible(*cx)).map(|cx| (c, cx))));
            }
        }
        if v >= 599.5 || !self.fit_settings_columns(&page, v, &views) {
            self.fit(vm, &page, v, &views, true);
        }
        // pop-ups stay centred on the rows shown
        if let Some((_, root, x)) = boxes {
            let lift = (600.0 - v) / 2.0;
            self.remap(root, *x, &move |y| y - lift, 0.0);
        }
    }

    fn prepare_story(&mut self, screen: &Value, at: Xf) {
        let screen_go = go_of(screen);
        let name = with_go(&screen_go, |g| g.name.clone()).unwrap_or_default();
        let screen_width = width(at);
        let width = screen_width.min(1060.0);
        let left = left(at) + (screen_width - width) / 2.0;
        if name == "Start Panel" {
            let text = crate::save_slots::tr(&crate::save_slots::child(&screen_go, "Text"));
            if let Some(x) = self.now(&text, &at) {
                self.rects.insert(id(&text).unwrap(), horizontal(x, left + width * 0.1, width * 0.8));
            }
        } else if name.starts_with("End Panel") {
            let spot_go = crate::save_slots::child(&screen_go, "Ui Spot");
            let spot = crate::save_slots::tr(&spot_go);
            let art = crate::save_slots::tr(&crate::save_slots::child(&spot_go, "Final Boss Frame"));
            let text = crate::save_slots::tr(&crate::save_slots::child(&spot_go, "Text"));
            let button = crate::save_slots::tr(&crate::save_slots::child(&screen_go, "Button"));
            let Some(spot_rect) = self.now(&spot, &at) else { return };
            let (Some(art_rect), Some(text_rect), Some(button_rect)) =
                (self.now(&art, &spot_rect), self.now(&text, &spot_rect), self.now(&button, &at))
            else { return };
            let inset = 14.0;
            let art_width = width * 0.34;
            let split = left + 2.0 * inset + art_width;
            self.place(&art, art_rect, horizontal(art_rect, left + inset, art_width), Area::Scale);
            let (art_top, _) = vertical_bounds(art_rect);
            let scale = art_width / (art_rect.w * art_rect.scale[0].abs());
            let resized = self.now(&art, &spot_rect).unwrap_or(art_rect);
            self.remap(&art, resized, &|y| art_top + (y - art_top) * scale, 0.0);
            let button_width = button_rect.w * button_rect.scale[0].abs();
            self.place(&button, button_rect, horizontal(button_rect, left + inset + (art_width - button_width) / 2.0, button_width), Area::Fixed);
            let text_width = left + width - split - inset;
            self.rects.insert(id(&text).unwrap(), horizontal(text_rect, split, text_width));
            self.text_fit.extend(id(&text));
        }
    }

    /// Current presented rectangle of a node (laid out this pass, else from its parent).
    fn now(&self, tr: &Value, parent: &Xf) -> Option<Xf> {
        self.rects.get(&id(tr)?).copied().or_else(|| child_rect(parent, tr))
    }

    fn pan(&mut self, key: usize, extra: f32) -> f32 {
        let p = self.pans.entry(key).or_insert(0.0);
        *p = p.clamp(0.0, extra);
        *p
    }

    #[allow(clippy::too_many_arguments)]
    fn region(&mut self, key: usize, view: [f32; 4], extra: f32, native: Value, native_range: f32, native_off: f32, bar: [f32; 4], content: f32) {
        if extra + native_range <= 0.01 {
            return;
        }
        if bar[3] > bar[1] {
            self.regions.push(ScrollRegion { key, view, extra, native, native_range, native_off, track: bar, content: content.max(1.0) });
        }
    }

    /// Settings columns are independent: a button in one column does not occupy a blank row in its neighbours.
    fn fit_settings_columns(&mut self, page: &[(Value, Xf)], target: f32, views: &FxMap<usize, Value>) -> bool {
        let Some((menu, x)) = page.iter().find_map(|(root, rect)| {
            std::iter::once((root.clone(), *rect))
                .chain(children(root).into_iter().filter_map(|child| self.now(&child, rect).map(|at| (child, at))))
                .find(|(node, at)| visible(*at) && with_go(&go_of(node), |g| {
                    g.active && !g.port_hidden && g.name == "Settings Menu"
                }) == Some(true))
        }) else { return false };
        let overflow = 600.0 - target;
        let mut columns = FxMap::default();
        for child in children(&menu) {
            if !with_go(&go_of(&child), |g| g.active && !g.port_hidden &&
                (g.name.starts_with("Settings Panel ") || g.name == "Augment Settings 1")).unwrap_or(false) {
                continue;
            }
            let Some(at) = self.now(&child, &x) else { return false };
            let (top, bottom) = vertical_bounds(at);
            let mut solid = Vec::new();
            let mut lists = Vec::new();
            self.rows_of(&child, at, views, &mut solid, &mut lists);
            if !lists.is_empty() || bottom - top <= overflow {
                return false;
            }
            let runs = row_runs([top, bottom], &solid, &lists);
            let mut sizes: Vec<_> = runs.iter().map(|(_, a, b)| b - a).collect();
            let mut remaining = overflow;
            trim_rows(&runs, &mut sizes, &mut remaining, [top, bottom], SETTINGS_EDGE_GAP);
            if remaining > 0.01 {
                let available = bottom - top - overflow + remaining;
                if remaining > available * SQUEEZE {
                    return false;
                }
                let ratio = (available - remaining) / available;
                sizes.iter_mut().for_each(|size| *size *= ratio);
            }
            let map = row_map(&runs, &sizes, top, top);
            columns.insert(id(&child).unwrap(), map);
        }
        if columns.len() != 4 {
            return false;
        }
        let (top, bottom) = vertical_bounds(x);
        self.rects.insert(id(&menu).unwrap(), vertical(x, top, bottom - top - overflow));
        for child in children(&menu) {
            if let Some(at) = self.now(&child, &x) {
                if let Some(map) = id(&child).and_then(|key| columns.get(&key)) {
                    self.remap(&child, at, &|y| piecewise(y, map), 0.0);
                }
            }
        }
        true
    }

    /// Fits the static layout first and hands any scrolling to a single authored block.
    fn fit(&mut self, vm: &Vm, parts: &[(Value, Xf)], target: f32, views: &FxMap<usize, Value>, allow_scroll: bool) {
        let block = allow_scroll.then(|| self.scroll_block(parts, views, target)).flatten();
        let chosen: FxMap<_, _> = block.iter().map(|block| {
            let key = id(&block.node).unwrap();
            self.rects.insert(key, block.rect);
            (key, views.get(&key).cloned().unwrap_or_default())
        }).collect();
        let (mut solid, mut lists) = (vec![], vec![]);
        for (tr, x) in parts {
            self.rows_of(tr, *x, &chosen, &mut solid, &mut lists);
        }
        let runs = row_runs([0.0, 600.0], &solid, &lists);
        let mut need = (600.0 - target).max(0.0);
        let mut size: Vec<f32> = runs.iter().map(|r| r.2 - r.1).collect();
        trim_rows(&runs, &mut size, &mut need, [0.0, 600.0], EDGE_GAP);
        let total: f32 = size.iter().sum();
        let squeezed = need > 0.01;
        if squeezed {
            let k = target / total.max(1.0);
            size.iter_mut().for_each(|s| *s *= k);
        }
        let map = row_map(&runs, &size, 0.0, 0.0);
        if std::env::var_os("NGU_CONSOLE_DEBUG").is_some() {
            eprintln!("[fit] target {target} remaining {need} runs {:?} map {:?}", runs.iter().zip(&size).map(|(r, s)| (r.0, r.1 as i32, r.2 as i32, *s as i32)).collect::<Vec<_>>(), map);
        }
        let f = move |y: f32| piecewise(y, &map);
        // scroll views: shortened, their contents slide under them
        let mut inner = FxMap::default();
        let rows = block.as_ref().and_then(|block| block.rows);
        for (vp, a, b) in &lists {
            let k = id(vp).unwrap_or(0);
            let shown = f(*b) - f(*a);
            let extra = (b - a - shown).max(0.0);
            inner.insert(k, (self.pan(k, extra), rows));
        }
        let edges = if squeezed { 0.0 } else { 60.0 };
        for (tr, x) in parts {
            self.remap_fit(tr, *x, &f, &inner, edges);
        }
        for (vp, a, b) in lists {
            let k = id(&vp).unwrap_or(0);
            let (t, e) = (f(a), f(b));
            let extra = (b - a - (e - t)).max(0.0);
            let Some(x) = self.rects.get(&k).copied() else { continue };
            let (x0, x1) = (left(x), left(x) + width(x));
            let comp = chosen.get(&k).cloned().unwrap_or_default();
            let (range, off) = native_scroll(vm, &comp);
            if extra + range > 0.01 {
                let inset = if framed(&go_of(&vp)) && comp.is_null() { 5.0 } else { 0.0 };
                let header = block.as_ref().map_or(0.0, |block| block.header);
                let start = t + header.max(inset);
                let gutter = if inset > 0.0 { x1 - 15.0 } else { (x1 + 1.0).min(self.width() - 15.0) };
                let clip = [x0 + inset, start, (x1 - inset).min(gutter), e - inset];
                if inset > 0.0 {
                    self.reserve_scrollbar_gutter(&vp, x, rows, clip[0], clip[2]);
                }
                self.clips.insert(k, ScrollClip { bounds: clip, rows });
                let bar = [gutter, start + 3.0, gutter + 10.0, e - 6.0];
                self.region(k, clip, extra, comp, range, off, bar, b - a + range - header - inset);
            }
        }
    }

    fn reserve_scrollbar_gutter(&mut self, block: &Value, at: Xf, rows: Option<&str>, left_edge: f32, right_edge: f32) {
        let Some(content) = self.bounds_of_children(block, at, rows) else { return };
        let content_left = left(content).max(left_edge);
        let content_right = left(content) + width(content);
        if content_right <= right_edge || content_left >= right_edge { return; }
        let scale = (right_edge - content_left) / (content_right - content_left);
        let map = |rect: Xf| {
            let l = content_left + (left(rect) - content_left) * scale;
            horizontal(rect, l, width(rect) * scale)
        };
        for child in children(block) {
            if scrolls_with_block(&go_of(&child), rows) {
                if let Some(rect) = self.now(&child, &at) {
                    self.map_rects(&child, rect, &map);
                }
            }
        }
    }

    fn scroll_block(&self, parts: &[(Value, Xf)], views: &FxMap<usize, Value>, target: f32) -> Option<ScrollBlock> {
        // These grids are plain Unity containers, so identify them by their authored parent and name.
        const BLOCKS: &[(&str, &str)] = &[
            ("Inventory", "Inventory"),
            ("Wishes Menu", "Slots"),
            ("Cards Menu", "Deck Slots"),
            ("Achievements Menu", "Slots"),
            ("Settings Menu", "Settings Panel 3"),
        ];
        let mut stack: Vec<_> = parts.iter().map(|(tr, x)| (tr.clone(), *x, String::new())).collect();
        let mut best = None;
        while let Some((node, at, parent)) = stack.pop() {
            let go = go_of(&node);
            let Some(name) = with_go(&go, |g| (g.active && !g.port_hidden).then(|| g.name.clone())).flatten() else {
                continue;
            };
            if !visible(at) {
                continue;
            }
            let rows = (parent == "Contents" && name == "Defense Panel").then_some("Defense (");
            let named = rows.is_some() || BLOCKS.contains(&(parent.as_str(), name.as_str()));
            // The bag holder has a placeholder rectangle. Its slot grid defines the actual viewport.
            let at = if named && rows.is_none() && name != "Settings Panel 3" {
                self.bounds_of_children(&node, at, None).unwrap_or(at)
            } else { at };
            let (top, bottom) = vertical_bounds(at);
            let h = bottom - top;
            let native = views.contains_key(&id(&node).unwrap_or(0));
            let priority = if named && target < 599.5 { 3 }
                else if native { 2 }
                else if target < 599.5 && top >= 125.0 && (180.0..400.0).contains(&h) && width(at) >= 140.0
                    && framed(&go) { 1 } else { 0 };
            if priority != 0 && h > LIST_MIN && !children(&node).is_empty() {
                let score = (priority, h * width(at));
                if best.as_ref().is_none_or(|(old, _)| score > *old) {
                    let header = rows.and_then(|prefix| self.bounds_of_children(&node, at, Some(prefix)))
                        .map_or(0.0, |first| (vertical_bounds(first).0 - top).max(0.0));
                    best = Some((score, ScrollBlock { node: node.clone(), rect: at, rows, header }));
                }
            }
            if !native || name == "Train Attack Menu + Scripts" {
                for child in children(&node) {
                    if let Some(rect) = self.now(&child, &at) {
                        stack.push((child, rect, name.clone()));
                    }
                }
            }
        }
        best.map(|(_, node)| node)
    }

    fn bounds_of_children(&self, node: &Value, at: Xf, rows: Option<&str>) -> Option<Xf> {
        let mut rects = children(node).into_iter().filter_map(|child| {
            let go = go_of(&child);
            with_go(&go, |g| g.active && !g.port_hidden).filter(|visible| *visible)?;
            if !scrolls_with_block(&go, rows) { return None; }
            self.now(&child, &at)
        });
        let first = rects.next()?;
        let (top, bottom) = vertical_bounds(first);
        let [x0, y0, x1, y1] = rects.fold([left(first), top, left(first) + width(first), bottom], |[l, t, r, b], x| {
            let (top, bottom) = vertical_bounds(x);
            [l.min(left(x)), t.min(top), r.max(left(x) + width(x)), b.max(bottom)]
        });
        Some(vertical(horizontal(at, x0, x1 - x0), y0, y1 - y0))
    }

    /// Rows taken by visible content (texts, controls, pictures) and by the scroll views the game shows, below `tr`.
    fn rows_of(&self, tr: &Value, x: Xf, views: &FxMap<usize, Value>, solid: &mut Vec<(f32, f32)>, lists: &mut Vec<(Value, f32, f32)>) {
        let go = go_of(tr);
        let Some(comps) = with_go(&go, |g| (g.active && !g.port_hidden).then(|| g.comps.clone())).flatten() else { return };
        let h = x.h * x.scale[1].abs();
        let top = 600.0 - (x.origin[1] + (1.0 - x.pivot[1]) * h);
        let (l, w) = (left(x), width(x));
        if top >= 600.0 || top + h <= 0.0 || l >= self.width() || l + w <= 0.0 {
            return;
        }
        if views.contains_key(&id(tr).unwrap_or(0)) && h > LIST_MIN {
            lists.push((tr.clone(), top, top + h));
            return;
        }
        let mut content = false;
        for c in &comps {
            let stop = with_comp(c, |k| {
                if !k.enabled {
                    return false;
                }
                match &k.k {
                    CK::Text(t) => content |= !t.text.trim().is_empty(),
                    CK::Sel(s) if s.kind == SelKind::Scrollbar => return true,
                    CK::Sel(_) => content = true,
                    CK::Image(i) | CK::RawImage(i) => content |= i.typ != 1 && i.color[3] > 0.01 && w > 0.5 && h > 0.5,
                    _ => {}
                }
                false
            }).unwrap_or(false);
            if stop {
                return;
            }
        }
        if content && h < 560.0 {
            if std::env::var_os("NGU_CONSOLE_DEBUG").is_some() && h > 60.0 {
                eprintln!("[fit] solid {:.0}-{:.0} {}", top, top + h, with_go(&go, |g| g.name.clone()).unwrap_or_default());
            }
            solid.push((top - PAD, top + h + PAD));
        }
        for ch in children(tr) {
            if let Some(cx) = self.now(&ch, &x) {
                self.rows_of(&ch, cx, views, solid, lists);
            }
        }
    }

    /// `remap` that keeps scroll views whole: a view gets its new edges, its contents only slide (by its inner pan).
    fn remap_fit(&mut self, tr: &Value, now: Xf, f: &dyn Fn(f32) -> f32, inner: &FxMap<usize, (f32, Option<&str>)>, edges: f32) {
        let key = id(tr).unwrap_or(0);
        let h = now.h * now.scale[1].abs();
        let top = 600.0 - (now.origin[1] + (1.0 - now.pivot[1]) * h);
        if let Some(&(pan, rows)) = inner.get(&key) {
            let (a, b) = (f(top), f(top + h));
            self.rects.insert(key, vertical(now, a, (b - a).max(0.0)));
            let shift = a - top - pan;
            for child in children(tr) {
                if let Some(cx) = self.now(&child, &now) {
                    let go = go_of(&child);
                    let offset = if frame_corner(&go) {
                        if 600.0 - cx.origin[1] < top + h / 2.0 { a - top } else { b - top - h }
                    } else if scrolls_with_block(&go, rows) { shift } else { a - top };
                    self.remap(&child, cx, &move |y| y + offset, 0.0);
                }
            }
            return;
        }
        let next = if h >= edges {
            let (a, b) = (f(top), f(top + h));
            vertical(now, a, (b - a).max(0.0))
        } else {
            let c = f(top + h / 2.0);
            vertical(now, c - h / 2.0, h)
        };
        if next.h + 0.5 < now.h && now.h >= 18.0 && with_go(&go_of(tr), |g| {
            g.comps.iter().any(|c| with_comp(c, |c| {
                c.enabled && matches!(&c.k, CK::Text(text) if !text.best_fit && text.vov == 0 && !text.text.trim().is_empty())
            }).unwrap_or(false))
        }).unwrap_or(false) {
            // Keep complete multiline labels readable when their section gets shorter.
            self.text_fit.insert(key);
        }
        self.rects.insert(key, next);
        for child in children(tr) {
            if let Some(cx) = self.now(&child, &now) {
                self.remap_fit(&child, cx, f, inner, edges);
            }
        }
    }

    /// Moves a subtree vertically through `f` (rows, y down). Nodes at least `edges` tall map both edges (panels stretch
    /// or shrink); smaller ones map their centre and keep their size (buttons and texts are not squashed).
    fn remap(&mut self, tr: &Value, now: Xf, f: &dyn Fn(f32) -> f32, edges: f32) {
        self.map_rects(tr, now, &|rect| {
            let (top, bottom) = vertical_bounds(rect);
            if bottom - top >= edges {
                let (a, b) = (f(top), f(bottom));
                vertical(rect, a, (b - a).max(0.0))
            } else {
                let h = bottom - top;
                let center = f(top + h / 2.0);
                vertical(rect, center - h / 2.0, h)
            }
        });
    }

    fn map_rects(&mut self, tr: &Value, now: Xf, map: &impl Fn(Xf) -> Xf) {
        if let Some(key) = id(tr) {
            self.rects.insert(key, map(now));
        }
        for child in children(tr) {
            if let Some(cx) = self.now(&child, &now) {
                self.map_rects(&child, cx, map);
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
                let (mut next, child_area) = position(&child, g.name.as_str(), &g.comps, area, source, target, x, self.width() - BASE + self.shift);
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
    if vm.eng.desktop_layout.width() <= BASE && !vm.eng.desktop_layout.console {
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

/// Piecewise-linear map of rows: (from, to) pairs; past the last pair rows shift.

fn piecewise(y: f32, map: &[(f32, f32)]) -> f32 {
    for w in map.windows(2) {
        let ((a0, b0), (a1, b1)) = (w[0], w[1]);
        if y <= a1 {
            return b0 + (y - a0) * (b1 - b0) / (a1 - a0).max(1e-3);
        }
    }
    let (a, b) = map[map.len() - 1];
    y - a + b
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowKind {
    Content,
    List,
    Gap,
}

fn row_runs(bounds: [f32; 2], solid: &[(f32, f32)], lists: &[(Value, f32, f32)]) -> Vec<(RowKind, f32, f32)> {
    let [start, end] = bounds;
    let mut cuts = vec![start, end];
    for (a, b) in solid.iter().copied().chain(lists.iter().map(|(_, a, b)| (*a, *b))) {
        cuts.extend([a.clamp(start, end), b.clamp(start, end)]);
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    let mut runs: Vec<(RowKind, f32, f32)> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let middle = (a + b) / 2.0;
        let kind = if solid.iter().any(|&(s, e)| s <= middle && middle < e) {
            RowKind::Content
        } else if lists.iter().any(|&(_, s, e)| s <= middle && middle < e) {
            RowKind::List
        } else {
            RowKind::Gap
        };
        match runs.last_mut() {
            Some(run) if run.0 == kind => run.2 = b,
            _ => runs.push((kind, a, b)),
        }
    }
    runs
}

fn trim_rows(runs: &[(RowKind, f32, f32)], sizes: &mut [f32], need: &mut f32, [start, end]: [f32; 2], edge_gap: f32) {
    for (kind, minimum) in [(RowKind::Gap, GAP), (RowKind::Gap, TIGHT_GAP), (RowKind::List, LIST_MIN)] {
        if *need <= 0.01 {
            break;
        }
        let available = |&(k, a, b): &(RowKind, f32, f32), size: f32| {
            if k != kind { return 0.0; }
            let edge = k == RowKind::Gap && (a - start <= 0.5 || end - b <= 0.5);
            (size - if edge { edge_gap } else { minimum }).max(0.0)
        };
        let room: f32 = runs.iter().zip(sizes.iter()).map(|(r, &s)| available(r, s)).sum();
        if room > 0.0 {
            let take = (*need).min(room);
            for (run, size) in runs.iter().zip(sizes.iter_mut()) {
                *size -= available(run, *size) * take / room;
            }
            *need -= take;
        }
    }
}

fn row_map(runs: &[(RowKind, f32, f32)], sizes: &[f32], start: f32, mut at: f32) -> Vec<(f32, f32)> {
    let mut map = vec![(start, at)];
    for (run, size) in runs.iter().zip(sizes) {
        at += size;
        map.push((run.2, at));
    }
    map
}

fn vertical_bounds(x: Xf) -> (f32, f32) {
    let height = x.h * x.scale[1].abs();
    let top = 600.0 - (x.origin[1] + (1.0 - x.pivot[1]) * height);
    (top, top + height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrollbar_thumb_reaches_both_ends_of_its_own_track() {
        let block = ScrollRegion {
            key: 10, view: [0.0, 10.0, 100.0, 110.0],
            extra: 40.0, native: Value::Null, native_range: 60.0,
            native_off: 60.0, track: [100.0, 10.0, 110.0, 110.0], content: 200.0,
        };
        assert_eq!(block.grip(0.0), (40.0, 50.0));
        assert_eq!(block.grip(40.0), (60.0, 50.0));
    }

    #[test]
    fn tighten_inner_gaps_before_shortening_lists() {
        let bounds = [0.0, 240.0];
        let solid = [(18.0, 30.0), (44.0, 60.0), (210.0, 222.0)];
        let lists = [(Value::Null, 60.0, 210.0)];
        let runs = row_runs(bounds, &solid, &lists);
        let mut sizes: Vec<_> = runs.iter().map(|(_, start, end)| end - start).collect();
        let mut overflow = 20.0;
        trim_rows(&runs, &mut sizes, &mut overflow, bounds, EDGE_GAP);

        assert_eq!(overflow, 0.0);
        assert_eq!(sizes, [14.0, 12.0, 2.0, 16.0, 150.0, 12.0, 14.0]);
        let map = row_map(&runs, &sizes, 0.0, 0.0);
        assert_eq!(piecewise(240.0, &map), 220.0);
    }
}

/// `x` with its top edge at `top` (rows, y down) and height `h`.
fn vertical(mut x: Xf, top: f32, h: f32) -> Xf {
    let s = x.scale[1].abs();
    if s != 0.0 {
        x.h = h / s;
        x.origin[1] = 600.0 - top - (1.0 - x.pivot[1]) * h;
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
