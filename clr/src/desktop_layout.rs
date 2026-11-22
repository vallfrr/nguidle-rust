//! PC presentation rectangles. Game scripts keep their original RectTransforms.
use crate::ui::{child_xf, xf, Xf};
use crate::unity::{go_of, with_comp, with_go, with_tr, SelKind, CK};
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
    /// console mode: game rows shown on screen (the rest of the 600 rows is zoomed out of view)
    pub console_view_h: f32,
    /// console mode: what scrolls after the last rebuild (only what did not fit), and every scroll offset (game rows),
    /// kept across rebuilds by key (`PAGE`, `COLUMN`, `LIST` or a scroll view's transform)
    pub regions: Vec<ScrollRegion>,
    pub pans: FxMap<usize, f32>,
    /// console mode: nodes whose children are clipped to a rectangle (x0, y0, x1, y1; game rows, y down): a scrolling page's
    /// frame stays put and only its inside slides
    pub clips: FxMap<usize, [f32; 4]>,
    /// scrollbars to draw (track x0, y0, x1, y1 with y down; handle start and length as fractions of the track)
    pub bars: Vec<([f32; 4], f32, f32)>,
    width: f32,
    /// console mode: how far left the page moved (the menu column's width)
    shift: f32,
    canvas: Value,
    rects: FxMap<usize, Xf>,
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
}

pub const PAGE: usize = 1;
pub const COLUMN: usize = 2;
pub const LIST: usize = 3;
/// rows kept around content, between two contents, at the frame edges, and the shortest a scroll view gets
const PAD: f32 = 2.0;
const GAP: f32 = 6.0;
const EDGE_GAP: f32 = 14.0;
const LIST_MIN: f32 = 110.0;
/// share of a page's rows that may be pressed together rather than scrolled
const SQUEEZE: f32 = 0.12;

/// Whether `tr` or one of its descendants (shown) has one of these names.
fn has_named(tr: &Value, names: &[&str]) -> bool {
    let go = go_of(tr);
    let Some((name, shown)) = with_go(&go, |g| (g.name.clone(), g.active && !g.port_hidden)) else { return false };
    shown && (names.contains(&name.as_str()) || children(tr).iter().any(|c| has_named(c, names)))
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
    native_scroll(vm, &FxMap::default(), comp).1
}

/// A game scroll view's own scroll state (range and offset, rows) and its scrollbar's presented rectangle, if it has one.
fn native_scroll(vm: &Vm, rects: &FxMap<usize, Xf>, comp: &Value) -> (f32, f32, Option<[f32; 4]>) {
    let Some((content, viewport, bar, go)) = with_comp(comp, |c| match &c.k {
        CK::ScrollRect { content, viewport, vscroll, .. } => Some((content.clone(), viewport.clone(), vscroll.clone(), c.go.clone())),
        _ => None,
    }).flatten() else { return (0.0, 0.0, None) };
    let vp = if viewport.is_null() { with_go(&go, |g| g.transform.clone()).unwrap_or_default() } else { viewport };
    let (cx, vx) = (xf(vm, &content), xf(vm, &vp));
    let (ch, vh) = (cx.h * cx.scale[1].abs(), vx.h * vx.scale[1].abs());
    let top = |x: Xf, h: f32| x.origin[1] + (1.0 - x.pivot[1]) * h;
    let range = (ch - vh).max(0.0);
    let off = (top(cx, ch) - top(vx, vh)).clamp(0.0, range);
    let rect = with_comp(&bar, |c| with_go(&c.go, |g| (g.active && !g.port_hidden).then(|| g.transform.clone())).flatten()).flatten()
        .and_then(|tr| rects.get(&id(&tr)?).copied())
        .map(|b| {
            let bh = b.h * b.scale[1].abs();
            let bt = 600.0 - (b.origin[1] + (1.0 - b.pivot[1]) * bh);
            [left(b), bt, left(b) + width(b), bt + bh]
        });
    (range, off, rect)
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
        self.pans.values().sum::<f32>() + self.pans.len() as f32 * 0.001
    }

    pub fn clip_of(&self, tr: &Value) -> Option<[f32; 4]> {
        self.clips.get(&id(tr)?).copied()
    }

    pub fn width(&self) -> f32 {
        if self.console && self.width > 0.0 { self.width } else { self.width.max(BASE) }
    }

    pub(crate) fn rect(&self, tr: &Value) -> Option<Xf> {
        self.rects.get(&id(tr)?).copied()
    }

    fn rebuild(&mut self, vm: &Vm) {
        self.rects.clear();
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
        self.bars.clear();
        self.regions.clear();
        self.clips.clear();
        let views = scroll_views(vm);
        let right = self.width();
        let roots: Vec<(String, Value, Xf)> = children(&self.canvas).into_iter().filter_map(|root| {
            let name = with_go(&go_of(&root), |g| (g.active && !g.port_hidden).then(|| g.name.clone())).flatten()?;
            let x = self.now(&root, &canvas)?;
            Some((name, root, x))
        }).collect();
        // the player column, with the bolts on its section joints
        if let Some((node, rect)) = player.clone().filter(|_| !self.console_hold) {
            let at = id(&node).and_then(|k| self.rects.get(&k).copied()).unwrap_or(rect);
            let mut parts = vec![(node, at)];
            parts.extend(roots.iter().filter(|(n, _, x)| n.starts_with("Middle Bolt") && left(*x) < 200.0).map(|(_, r, x)| (r.clone(), *x)));
            let colw = width(at) + left(at);
            self.fit(vm, COLUMN, &parts, v, [0.0, colw], &views, colw - 18.0, 1.0);
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
        if v >= 599.5 {
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
        let page_left = player.map_or(0.0, |(_, p)| left(p) + width(p));
        // fights must stay whole on screen; elsewhere a small overflow is pressed together, a big one scrolls
        let fight = page.iter().any(|(tr, _)| has_named(tr, &["Adventure Menu", "Boss Menu"]));
        self.fit(vm, PAGE, &page, v, [page_left, right], &views, right - 17.0, if fight { 1.0 } else { SQUEEZE });
        // pop-ups stay centred on the rows shown
        if let Some((_, root, x)) = roots.iter().find(|(n, _, _)| n == "Box Canvas") {
            let lift = (600.0 - v) / 2.0;
            self.remap(root, *x, &move |y| y - lift, 0.0);
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
        if extra <= 0.0 {
            return;
        }
        let pan = self.pans.get(&key).copied().unwrap_or(0.0);
        let total = (content).max(1.0);
        self.bars.push((bar, (native_off + pan) / total, (view[3] - view[1]) / total));
        self.regions.push(ScrollRegion { key, view, extra, native, native_range, native_off });
    }

    /// Squeezes the rows of `parts` (one column or the page) into `target` rows; see `console_vertical`.
    #[allow(clippy::too_many_arguments)]
    /// `squeeze`: the largest share of the rows that may be pressed together (controls get shorter, texts keep their size)
    /// when the rest still does not fit; past it, the part scrolls. 1 = never scroll (player column, fights).
    fn fit(&mut self, vm: &Vm, key: usize, parts: &[(Value, Xf)], target: f32, xs: [f32; 2], views: &FxMap<usize, Value>, bar_x: f32, squeeze: f32) {
        let (mut solid, mut lists) = (vec![], vec![]);
        for (tr, x) in parts {
            self.rows_of(tr, *x, views, &mut solid, &mut lists);
        }
        // runs of rows: 0 solid, 1 list (a scroll view), 2 empty
        let mut cuts: Vec<f32> = vec![0.0, 600.0];
        for (a, b) in solid.iter().copied().chain(lists.iter().map(|(_, a, b)| (*a, *b))) {
            cuts.extend([a.clamp(0.0, 600.0), b.clamp(0.0, 600.0)]);
        }
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        let mut runs: Vec<(u8, f32, f32)> = vec![];
        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let m = (a + b) / 2.0;
            let class = if solid.iter().any(|&(s, e)| s <= m && m < e) { 0 }
                else if lists.iter().any(|&(_, s, e)| s <= m && m < e) { 1 } else { 2 };
            match runs.last_mut() {
                Some(r) if r.0 == class => r.2 = b,
                _ => runs.push((class, a, b)),
            }
        }
        let floor = |c: u8, a: f32, b: f32| -> f32 {
            let len = b - a;
            match c {
                0 => len,
                1 => len.min(LIST_MIN),
                _ => len.min(if a <= 0.5 || b >= 599.5 { EDGE_GAP } else { GAP }),
            }
        };
        let mut need = (600.0 - target).max(0.0);
        let mut size: Vec<f32> = runs.iter().map(|r| r.2 - r.1).collect();
        for class in [2u8, 1u8] {
            let room: f32 = runs.iter().zip(&size).filter(|(r, _)| r.0 == class).map(|(r, s)| s - floor(r.0, r.1, r.2)).sum();
            if room <= 0.0 || need <= 0.0 {
                continue;
            }
            let take = need.min(room);
            for (r, s) in runs.iter().zip(size.iter_mut()).filter(|(r, _)| r.0 == class) {
                *s -= (*s - floor(r.0, r.1, r.2)) * take / room;
            }
            need -= take;
        }
        let mut rem = need;
        let total: f32 = size.iter().sum();
        let squeezed = rem > 0.0 && rem <= total * squeeze;
        if squeezed {
            let k = target / total.max(1.0);
            size.iter_mut().for_each(|s| *s *= k);
            rem = 0.0;
        }
        let pan = self.pan(key, rem);
        let mut map = vec![(0.0f32, -pan)];
        let mut at = -pan;
        for (r, s) in runs.iter().zip(&size) {
            at += s;
            map.push((r.2, at));
        }
        if std::env::var_os("NGU_CONSOLE_DEBUG").is_some() {
            eprintln!("[fit] key {key} target {target} rem {rem} runs {:?} map {:?}", runs.iter().zip(&size).map(|(r, s)| (r.0, r.1 as i32, r.2 as i32, *s as i32)).collect::<Vec<_>>(), map);
        }
        let f = move |y: f32| piecewise(y, &map);
        // scroll views: shortened, their contents slide under them
        let mut inner = FxMap::default();
        for (vp, a, b) in &lists {
            let k = id(vp).unwrap_or(0);
            let shown = f(*b) - f(*a);
            let extra = (b - a - shown).max(0.0);
            inner.insert(k, self.pan(k, extra));
        }
        let edges = if squeezed { 0.0 } else { 60.0 };
        for (tr, x) in parts {
            self.remap_fit(tr, *x, &f, &inner, edges);
            if rem > 0.0 && key == PAGE {
                self.freeze_frame(tr, *x, &f, pan, target);
            }
        }
        for (vp, a, b) in &lists {
            let k = id(vp).unwrap_or(0);
            let (t, e) = (f(*a), f(*b));
            let extra = (b - a - (e - t)).max(0.0);
            if extra <= 0.0 {
                continue;
            }
            let x = self.rects.get(&k).copied();
            let (x0, x1) = x.map_or((xs[0], xs[1]), |x| (left(x), left(x) + width(x)));
            let comp = views.get(&k).cloned().unwrap_or_default();
            let (range, off, bar) = native_scroll(vm, &self.rects, &comp);
            let bar = bar.unwrap_or([x1 - 17.0, t + 6.0, x1 - 7.0, e - 6.0]);
            self.region(k, [x0, t, x1, e], extra, comp, range, off, bar, b - a + range);
        }
        self.region(key, [xs[0], 0.0, xs[1], target], rem, Value::Null, 0.0, 0.0, [bar_x, 10.0, bar_x + 10.0, target - 10.0], target + rem);
    }

    /// A page that scrolls keeps its frame (the bordered panel, or the panel just below the page root) where the unscrolled
    /// layout puts it, cut at the bottom of the screen; only what is inside the border slides, clipped to the inside.
    fn freeze_frame(&mut self, tr: &Value, x: Xf, f: &dyn Fn(f32) -> f32, pan: f32, target: f32) {
        let mut frame = None;
        let mut stack = vec![(tr.clone(), x)];
        while let Some((n, nx)) = stack.pop() {
            let go = go_of(&n);
            let framed = with_go(&go, |g| g.comps.clone()).unwrap_or_default().iter().any(|c| {
                with_comp(c, |k| match &k.k {
                    CK::Image(i) if k.enabled && i.typ == 1 => with_comp(&i.sprite, |s| matches!(&s.k, CK::Sprite(sp) if sp.border.iter().any(|b| *b > 0.0))).unwrap_or(false),
                    _ => false,
                }).unwrap_or(false)
            });
            let h = nx.h * nx.scale[1].abs();
            if framed && h >= 300.0 && width(nx) >= 300.0 {
                frame = Some((n, nx));
                break;
            }
            for ch in children(&n) {
                if let Some(cx) = self.now(&ch, &nx) {
                    stack.push((ch, cx));
                }
            }
        }
        let Some((node, now)) = frame else { return };
        let h = now.h * now.scale[1].abs();
        let top_now = 600.0 - (now.origin[1] + (1.0 - now.pivot[1]) * h);
        let (a, b) = (top_now + pan, top_now + pan + h);
        let _ = (a, b, f);
        let top = (top_now + pan).max(0.0);
        let bottom = (top_now + pan + h).min(target);
        if bottom - top < 100.0 {
            return;
        }
        let fixed = vertical(now, top, bottom - top);
        let Some(key) = id(&node) else { return };
        self.rects.insert(key, fixed);
        const INSET: f32 = 14.0;
        self.clips.insert(key, [left(fixed) + INSET, top + INSET, left(fixed) + width(fixed) - INSET, bottom - INSET]);
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
    fn remap_fit(&mut self, tr: &Value, now: Xf, f: &dyn Fn(f32) -> f32, inner: &FxMap<usize, f32>, edges: f32) {
        let key = id(tr).unwrap_or(0);
        let h = now.h * now.scale[1].abs();
        let top = 600.0 - (now.origin[1] + (1.0 - now.pivot[1]) * h);
        if let Some(&pan) = inner.get(&key) {
            let (a, b) = (f(top), f(top + h));
            self.rects.insert(key, vertical(now, a, (b - a).max(0.0)));
            let shift = a - top - pan;
            for child in children(tr) {
                if let Some(cx) = self.now(&child, &now) {
                    self.remap(&child, cx, &move |y| y + shift, 0.0);
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
        let h = now.h * now.scale[1].abs();
        let top = 600.0 - (now.origin[1] + (1.0 - now.pivot[1]) * h);
        let next = if h >= edges {
            let (a, b) = (f(top), f(top + h));
            vertical(now, a, (b - a).max(0.0))
        } else {
            let c = f(top + h / 2.0);
            vertical(now, c - h / 2.0, h)
        };
        if let Some(key) = id(tr) {
            self.rects.insert(key, next);
        }
        for child in children(tr) {
            let cx = self.rects.get(&id(&child).unwrap_or(0)).copied().or_else(|| child_rect(&now, &child));
            if let Some(cx) = cx {
                self.remap(&child, cx, f, edges);
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
