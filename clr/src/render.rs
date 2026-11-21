//! Unity UI rendering: production of a quad list (backend independent) + software rasterizer (PC tests).
use crate::ui::{view_child_xf, view_width, view_xf, Xf};
use crate::unity::*;
use crate::value::*;
use crate::Vm;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Tex {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>, // RGBA
}

pub const TEX_WHITE: u32 = 0;
pub const TEX_ATLAS: u32 = 1;
pub const ATLAS_SIZE: usize = 1024;

fn select_achievement(mut texture: Tex, locked: bool) -> Tex {
    let (w, h) = (texture.w, texture.h);
    let edge = (0..w * h).filter(|i| i % w == 0 || i % w == w - 1 || *i < w || *i >= w * (h - 1));
    let similar = |pixel: &[u8], color: [u8; 3]| {
        pixel[..3].iter().zip(color).all(|(&a, b)| a.abs_diff(b) <= 17)
    };
    let background = [[120, 134, 143], [90, 130, 142]].into_iter().map(|color| {
        let count = edge.clone().filter(|&i| similar(&texture.px[i * 4..], color)).count();
        (count, color)
    }).max_by_key(|(count, _)| *count).filter(|(count, _)| *count >= (w + h) / 2);
    let selected = crate::save_slots::SELECTION_COLOR.map(|v| (v * 255.0 + 0.5) as u8);
    let darken = |pixel: &mut [u8]| {
        if locked { for channel in &mut pixel[..3] { *channel = (f32::from(*channel) * 0.45) as u8; } }
    };
    if let Some((_, color)) = background {
        let mut seen = vec![false; w * h];
        let mut stack: Vec<_> = edge.collect();
        while let Some(i) = stack.pop() {
            if seen[i] || !similar(&texture.px[i * 4..], color) { continue; }
            seen[i] = true;
            if i % w > 0 { stack.push(i - 1); }
            if i % w + 1 < w { stack.push(i + 1); }
            if i >= w { stack.push(i - w); }
            if i + w < w * h { stack.push(i + w); }
        }
        for (pixel, replace) in texture.px.chunks_exact_mut(4).zip(seen) {
            if replace { pixel.copy_from_slice(&selected); } else { darken(pixel); }
        }
    } else if w > 8 && h > 8 {
        // Artwork without a plain background keeps its full image, inset onto the selection fill.
        let source = texture.px.clone();
        for (i, pixel) in texture.px.chunks_exact_mut(4).enumerate() {
            let (x, y) = (i % w, i / w);
            if (4..w - 4).contains(&x) && (4..h - 4).contains(&y) {
                let src = ((y - 4) * h / (h - 8) * w + (x - 4) * w / (w - 8)) * 4;
                pixel.copy_from_slice(&source[src..src + 4]);
                darken(pixel);
            } else {
                pixel.copy_from_slice(&selected);
            }
        }
    }
    texture
}

#[derive(Clone)]
pub struct Quad {
    /// corners in screen pixels: origin, +u, +u+v, +v
    pub p: [[f32; 2]; 4],
    /// normalised texture coordinates (v down), same corners
    pub uv: [[f32; 2]; 4],
    pub col: [f32; 4],
    pub tex: u32,
    pub nearest: bool,
    pub clip: [i32; 4],
}

struct Atlas {
    px: Vec<u8>,
    x: usize,
    y: usize,
    row_h: usize,
    dirty: bool,
    map: HashMap<(usize, char, u32), (f32, f32, f32, f32, fontdue::Metrics)>,
}

pub struct Renderer {
    /// durations (ms) of the last frame: quad building / software rasterization
    pub t_emit: f32,
    pub t_raster: f32,
    pub w: usize,
    pub h: usize,
    pub fb: Vec<u8>, // RGBA (software backend)
    pub quads: Vec<Quad>,
    pub textures: Vec<Rc<Tex>>, // index = id - 2
    pub tex_keys: Vec<String>,
    tex_ids: HashMap<String, Option<u32>>,
    fonts: HashMap<String, Option<Rc<fontdue::Font>>>,
    atlas: Atlas,
    scale: f32,
    ox: f32,
    oy: f32,
    vh: f32,
    pub draws: usize,
    gen: u32,
    pub visits: usize,
    pub hits: usize,
    /// active ColorTint tints (target graphic -> Selectable state colour), stack during the traversal
    tints: Vec<(Value, [f32; 4])>,
    tooltip: Option<Value>,
    drawing_overlay: bool,
    subframe_seconds: f64,
    /// screen profile regions (`crate::profile`); empty = the whole game space with the `emit` transform
    pub regions: Vec<crate::profile::Region>,
}

#[derive(Clone, Copy)]
struct Clip {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Clip {
    fn intersect(self, other: Self) -> Self {
        Self {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        }
    }
}

struct Run {
    ch: char,
    color: [f32; 4],
    bold: bool,
    size: f32,
}

impl Renderer {
    /// The quad caches stored in GameObjects reference the atlas and textures of ONE renderer:
    /// each renderer (and each atlas reset) takes a unique global generation, otherwise a new renderer
    /// (window resize, capture) reuses stale quads (blurred text, white squares).
    pub fn new(w: usize, h: usize) -> Renderer {
        Renderer {
            t_emit: 0.0,
            t_raster: 0.0,
            w,
            h,
            fb: vec![0; w * h * 4],
            quads: vec![],
            textures: vec![],
            tex_keys: vec![],
            tex_ids: HashMap::new(),
            fonts: HashMap::new(),
            atlas: Atlas { px: vec![0; ATLAS_SIZE * ATLAS_SIZE * 4], x: 1, y: 1, row_h: 0, dirty: true, map: HashMap::new() },
            scale: 1.0,
            ox: 0.0,
            oy: 0.0,
            vh: 600.0,
            draws: 0,
            gen: next_gen(),
            visits: 0,
            hits: 0,
            tints: vec![],
            tooltip: None,
            drawing_overlay: false,
            subframe_seconds: 0.0,
            regions: vec![],
        }
    }

    pub fn tex_by_id(&self, id: u32) -> Option<Rc<Tex>> {
        if id < 2 {
            return None;
        }
        self.textures.get(id as usize - 2).cloned()
    }

    /// Pixels of a texture, reloaded from the archive if the CPU copy was freed.
    pub fn tex_load(&mut self, id: u32) -> Option<Rc<Tex>> {
        let i = (id as usize).checked_sub(2)?;
        let t = self.textures.get(i)?.clone();
        if !t.px.is_empty() {
            return Some(t);
        }
        let rel = format!("tex/{}.png", self.tex_keys[i].replace(':', "_"));
        let n = crate::assets::read(&rel).and_then(|b| load_png_bytes(&b))?;
        let n = Rc::new(n);
        self.textures[i] = n.clone();
        Some(n)
    }

    /// Frees the CPU copy (the GPU keeps one) while keeping the dimensions.
    pub fn tex_release(&mut self, id: u32) {
        if let Some(i) = (id as usize).checked_sub(2) {
            if let Some(t) = self.textures.get(i) {
                let (w, h) = (t.w, t.h);
                if !t.px.is_empty() {
                    self.textures[i] = Rc::new(Tex { w, h, px: vec![] });
                }
            }
        }
    }

    pub fn atlas_pixels(&self) -> &[u8] {
        &self.atlas.px
    }
    pub fn atlas_take_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.atlas.dirty, false)
    }

    fn tex(&mut self, dir: &str, key: &str) -> Option<(u32, Rc<Tex>)> {
        if let Some(t) = self.tex_ids.get(key) {
            return t.map(|id| (id, self.textures[id as usize - 2].clone()));
        }
        let _ = dir;
        let (source, selection) = match key.split_once('#') {
            Some((source, "selected")) if source.starts_with("steam-achievements/") => (source, Some(false)),
            Some((source, "selected-locked")) if source.starts_with("steam-achievements/") => (source, Some(true)),
            _ => (key, None),
        };
        let rel = format!("tex/{}.png", source.replace(':', "_"));
        match crate::assets::read(&rel).and_then(|b| load_png_bytes(&b)) {
            Some(t) => {
                let id = self.textures.len() as u32 + 2;
                let t = if let Some(locked) = selection { select_achievement(t, locked) } else { t };
                let t = Rc::new(t);
                self.textures.push(t.clone());
                self.tex_keys.push(key.to_string());
                self.tex_ids.insert(key.to_string(), Some(id));
                Some((id, t))
            }
            None => {
                self.tex_ids.insert(key.to_string(), None);
                None
            }
        }
    }

    fn font(&mut self, dir: &str, file: &str) -> Option<Rc<fontdue::Font>> {
        if let Some(f) = self.fonts.get(file) {
            return f.clone();
        }
        let _ = dir;
        let f = crate::assets::read(&format!("fonts/{}", file)).and_then(|b| fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).ok()).map(Rc::new);
        self.fonts.insert(file.to_string(), f.clone());
        f
    }

    fn pick_font(&mut self, vm: &Vm, font: &Value, bold: bool) -> Option<(usize, Rc<fontdue::Font>)> {
        let dir = vm.eng.assets_dir.clone();
        let (name, file) = with_comp(font, |c| match &c.k {
            CK::Font { name, file } => (name.clone(), file.clone()),
            _ => (String::new(), String::new()),
        })
        .unwrap_or_default();
        let f = if file.is_empty() || name == "Arial" {
            if bold { "sharedassets0.assets_2490.ttf" } else { "sharedassets0.assets_2492.ttf" }.to_string()
        } else if bold && name == "LiberationSans-Regular" {
            "sharedassets0.assets_2490.ttf".to_string()
        } else {
            file
        };
        let id = f.bytes().fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize));
        self.font(&dir, &f).map(|x| (id, x))
    }

    // ---------------------------------------------------------------- quad production
    /// Builds the quad list of the current frame (self.quads).
    pub fn emit(&mut self, vm: &mut Vm, scale: f32, ox: f32, oy: f32) {
        self.run_layout(vm);
        crate::desktop_layout::rebuild(vm);
        crate::save_slots::reflow(vm);
        self.quads.clear();
        self.scale = scale;
        self.ox = ox;
        self.oy = oy;
        self.vh = vm.eng.screen.1 as f32;
        self.draws = 0;
        self.visits = 0;
        self.hits = 0;
        if self.atlas.y > ATLAS_SIZE * 3 / 4 {
            // atlas almost full: start over
            self.atlas.map.clear();
            self.atlas.x = 1;
            self.atlas.y = 1;
            self.atlas.row_h = 0;
            self.atlas.px.iter_mut().for_each(|b| *b = 0);
            self.atlas.dirty = true;
            self.gen = next_gen();
        }
        if let Some(d) = self.regions.iter().position(|r| r.nodes.iter().any(|n| n == "*")) {
            let r = &self.regions[d];
            (self.scale, self.ox, self.oy) = (r.scale, r.ox, r.oy);
        }
        let full = self.frame_clip(vm);
        if self.tooltip.is_none() {
            self.tooltip = Some(vm.eng.gos.iter().find_map(|go| {
                with_go(go, |g| (g.name == "Hover Tooltip").then(|| g.transform.clone())).flatten()
            }).unwrap_or_default());
        }
        self.regions.iter_mut().for_each(|r| r.drawn = false);
        let roots = vm.eng.canvases.iter().filter(|(_, tr)| {
            with_tr(tr, |t| t.parent.is_null()).unwrap_or(false) &&
                with_go(&go_of(tr), |g| g.active && !g.port_hidden).unwrap_or(false)
        }).cloned().collect::<Vec<_>>();
        let stats = std::env::var("CLR_EMITSTATS").is_ok();
        let sidebar_edge = crate::save_slots::covers_game_content(vm)
            .then(|| self.to_dev([crate::save_slots::CONTENT_LEFT, 0.0])[0].floor() as i32);
        for (_, tr) in roots {
            let x = view_xf(vm, &tr);
            let (q0, t0) = (self.quads.len(), std::time::Instant::now());
            let mut clip = full;
            if let Some(edge) = sidebar_edge.filter(|_| !crate::save_slots::owns(&go_of(&tr))) {
                clip.x1 = clip.x1.min(edge);
            }
            self.draw_node(vm, &tr, x, clip, 0);
            if stats {
                let n = with_go(&go_of(&tr), |g| g.name.clone()).unwrap_or_default();
                crate::elog!("  racine {:<28} {:>5} quads {:>9.3?}", n, self.quads.len() - q0, t0.elapsed());
            }
        }
        if let (Some(edge), Some(junctions)) = (sidebar_edge, crate::save_slots::frame_junctions(vm)) {
            // The sidebar's top and bottom bolts bridge the seam, above the content frame.
            let clip = Clip { x0: full.x0.max(edge), ..full };
            for tr in junctions.iter().filter(|tr| !tr.is_null()) {
                self.draw_node(vm, tr, view_xf(vm, tr), clip, 1);
            }
        }
        if !crate::save_slots::is_open(vm) && !vm.at_desktop_start() {
            self.drawing_overlay = true;
            for (transform, position) in crate::motion::followers(vm) {
                self.draw_node(vm, &transform, position, full, 2);
            }
            self.drawing_overlay = false;
        }
        self.draw_dropdown(vm);
        if let Some(tooltip) = self.tooltip.clone().filter(|tr| {
            !crate::save_slots::choosing_save(vm) && active_in_hierarchy(&go_of(tr))
        }) {
            let mut x = view_xf(vm, &tooltip);
            let (mx, my) = vm.eng.mouse;
            let width = vm.eng.desktop_layout.width();
            if vm.eng.desktop_panels && self.regions.is_empty() &&
                (0.0..width).contains(&mx) && (0.0..self.vh).contains(&my) &&
                vm.eng.invokes.iter().any(|v| v.method == "updateTooltip")
            {
                let (w, h) = (x.w * x.scale[0].abs(), x.h * x.scale[1].abs());
                let place = |pointer: f32, length: f32, limit: f32| {
                    let origin = if pointer + 10.0 + length <= limit { pointer + 10.0 }
                        else { pointer - length - 10.0 };
                    origin.clamp(0.0, (limit - length).max(0.0))
                };
                x.origin = [place(mx, w, width) + w * x.pivot[0], place(my, h, self.vh) + h * x.pivot[1]];
            }
            let clip = if self.regions.is_empty() { full } else {
                Clip { x0: 0, y0: 0, x1: self.w as i32, y1: self.h as i32 }
            };
            self.drawing_overlay = true;
            self.draw_node(vm, &tooltip, x, clip, 2);
            self.drawing_overlay = false;
        }
    }

    /// Emission + software rasterization into `fb`.
    pub fn render(&mut self, vm: &mut Vm, scale: f32, ox: f32, oy: f32) {
        self.emit_frame(vm, scale, ox, oy);
        let t = std::time::Instant::now();
        self.raster();
        self.t_raster = t.elapsed().as_secs_f32() * 1000.0;
    }

    /// Quad list of the frame; `t_emit` receives the time taken (ms). The GPU backends draw `self.quads`.
    pub fn emit_frame(&mut self, vm: &mut Vm, scale: f32, ox: f32, oy: f32) {
        let t = std::time::Instant::now();
        let undo = if vm.eng.motion.enabled {
            Some(crate::motion::present(vm, vm.eng.time + self.subframe_seconds))
        } else { None };
        self.emit(vm, scale, ox, oy);
        if let Some(undo) = undo { undo.apply(vm); }
        self.t_emit = t.elapsed().as_secs_f32() * 1000.0;
    }

    pub fn set_subframe_time(&mut self, seconds: f32) {
        self.subframe_seconds = f64::from(seconds.max(0.0));
    }

    /// Changes the device size without losing the textures and the glyph atlas (GPU backends keep no framebuffer).
    pub fn resize(&mut self, w: usize, h: usize, keep_fb: bool) {
        self.w = w;
        self.h = h;
        self.fb = if keep_fb { vec![0; w * h * 4] } else { vec![] };
    }

    fn draw_dropdown(&mut self, vm: &mut Vm) {
        let Some(popup) = crate::dropdown::popup(vm) else { return };
        let Some(mut text) = with_comp(&popup.caption, |c| {
            if let CK::Text(t) = &c.k { Some(TextSnap::from(t)) } else { None }
        }).flatten() else { return };
        let saved = (self.scale, self.ox, self.oy);
        if let Some(i) = crate::profile::region_at_game(&self.regions, popup.x + popup.width / 2.0, self.vh - popup.top) {
            let r = &self.regions[i];
            (self.scale, self.ox, self.oy) = (r.scale, r.ox, r.oy);
        }
        let clip = self.frame_clip(vm);
        let h = popup.row_height;
        let height = h * popup.rows as f32;
        let width = popup.width - popup.scrollbar_width();
        let mut x = Xf { origin: [popup.x, popup.top], scale: [1.0, 1.0], rot: 0.0, w: width, h, pivot: [0.0, 1.0] };
        self.quad(&x, [-1.0, -height - 1.0, popup.width + 2.0, height + 2.0], None, [0.0; 4], [0.12, 0.15, 0.17, 1.0], clip);
        text.align = 3;
        text.cached = None;
        for row in 0..popup.rows {
            let item = popup.first + row;
            x.origin = [popup.x, popup.top - row as f32 * h];
            let highlight = popup.cursor.unwrap_or(usize::MAX) == item;
            let color = if highlight { [1.0, 0.89, 0.52, 1.0] }
                else if popup.cursor.is_none() && popup.row_at(vm.eng.mouse) == Some(item) { [0.82, 0.90, 0.98, 1.0] }
                else if item == popup.selected { [0.90, 0.93, 0.95, 1.0] } else { [1.0; 4] };
            self.quad(&x, [0.0, -h, width, h], None, [0.0; 4], color, clip);
            text.text = crate::unity_api::opt_data(&popup.options[item], |o| o.text.clone()).unwrap_or_default();
            x.origin[0] += 5.0;
            x.w = width - 10.0;
            self.draw_text(vm, &Value::Null, &x, &text, clip);
        }
        if popup.scrollbar_width() > 0.0 {
            x.origin = [popup.x + width, popup.top];
            let bar = popup.scrollbar_width();
            let thumb_h = height * popup.rows as f32 / popup.options.len() as f32;
            let offset = height * popup.first as f32 / popup.options.len() as f32;
            self.quad(&x, [0.0, -height, bar, height], None, [0.0; 4], [0.88, 0.88, 0.88, 1.0], clip);
            self.quad(&x, [1.0, -offset - thumb_h, bar - 2.0, thumb_h], None, [0.0; 4], [0.36, 0.50, 0.62, 1.0], clip);
        }
        (self.scale, self.ox, self.oy) = saved;
    }

    /// Draws a node; nodes bound to a profile region (depth 1-2 below the root canvas) switch to that region's transform.
    fn draw_node(&mut self, vm: &mut Vm, tr: &Value, x: Xf, clip: Clip, depth: u8) {
        if self.regions.is_empty() || depth == 0 || depth > 2 {
            return self.draw_node_inner(vm, tr, x, clip, depth);
        }
        let go = go_of(tr);
        let name = with_go(&go, |g| g.name.clone()).unwrap_or_default();
        let path = if depth == 1 {
            name
        } else {
            let parent = with_tr(tr, |t| t.parent.clone()).unwrap_or(Value::Null);
            format!("{}/{name}", with_go(&go_of(&parent), |g| g.name.clone()).unwrap_or_default())
        };
        let Some(mut i) = self.regions.iter().position(|r| r.nodes.iter().any(|n| *n == path)) else {
            return self.draw_node_inner(vm, tr, x, clip, depth);
        };
        if !self.regions[i].visible {
            return;
        }
        let mut c = clip;
        if self.regions[i].follow {
            let (mx, my) = vm.eng.mouse;
            match crate::profile::region_at_game(&self.regions, mx, self.vh - my) {
                Some(j) => i = j,
                None => i = self.regions.iter().position(|r| r.nodes.iter().any(|n| n == "*")).unwrap_or(i),
            }
        } else {
            let d = self.regions[i].dest();
            c = Clip { x0: c.x0.max(d[0].floor() as i32), y0: c.y0.max(d[1].floor() as i32), x1: c.x1.min((d[0] + d[2]).ceil() as i32), y1: c.y1.min((d[1] + d[3]).ceil() as i32) };
        }
        let saved = (self.scale, self.ox, self.oy);
        let r = &self.regions[i];
        (self.scale, self.ox, self.oy) = (r.scale, r.ox, r.oy);
        let start = self.quads.len();
        self.draw_node_inner(vm, tr, x, c, depth);
        let (w, h) = (self.w as f32, self.h as f32);
        if self.quads[start..].iter().any(|q| q.p.iter().any(|p| p[0] >= 0.0 && p[1] >= 0.0 && p[0] < w && p[1] < h)) {
            self.regions[i].drawn = true;
        }
        (self.scale, self.ox, self.oy) = saved;
    }

    fn draw_node_inner(&mut self, vm: &mut Vm, tr: &Value, x: Xf, clip: Clip, depth: u8) {
        self.visits += 1;
        let go = go_of(tr);
        // containers above bound regions: their subtree mixes transforms that change without the game noticing
        let cacheable = self.regions.is_empty() || depth >= 2;
        let key = [x.origin[0], x.origin[1], x.scale[0], x.scale[1], x.rot, x.w, x.h, x.pivot[0], x.pivot[1], self.scale, self.ox, self.oy, vm.eng.desktop_layout.width()];
        let ck = [clip.x0, clip.y0, clip.x1, clip.y1];
        let gen = self.gen;
        // valid cache?
        let hit = with_go(&go, |g| {
            if g.dirty || !cacheable {
                return None;
            }
            g.cache.as_ref().filter(|c| c.gen == gen && c.key == key && c.clip == ck).map(|c| c.quads.clone())
        })
        .flatten();
        if let Some(q) = hit {
            self.hits += 1;
            self.quads.extend(q.iter().cloned());
            self.draws += q.len();
            return;
        }
        let (active, comps) = match with_go(&go, |g| (g.active && !g.port_hidden, g.comps.clone())) {
            Some(v) => v,
            None => return,
        };
        if !active {
            return;
        }
        let graphic_clip = self.graphic_clip(vm, tr, clip);
        let start = self.quads.len();
        let mut child_clip = clip;
        // Selectable with ColorTint transition: state colour (Unity: normal / hover / press / select / disabled)
        // multiplied with its target graphic's. ponytail: instant change (Unity fades over 0.1 s), colorMultiplier ignored (1).
        let tint = comps.iter().find_map(|c| {
            with_comp(c, |k| match &k.k {
                CK::Sel(s) if k.enabled && s.transition == 1 => {
                    let st = if !s.interactable { 4 } else if same(&go, &vm.eng.ptr.pressed) { 2 } else if same(&go, &vm.eng.ptr.hover) { 1 } else { 0 };
                    Some((s.target_graphic.clone(), s.colors[st]))
                }
                _ => None,
            })
            .flatten()
        });
        let pushed = tint.is_some();
        if let Some(t) = tint {
            self.tints.push(t);
        }
        // Mask with showMaskGraphic = false: the node's image is used for clipping but not drawn (Unity)
        let hide_graphic = comps.iter().any(|c| with_comp(c, |k| k.enabled && matches!(k.k, CK::Mask { show_graphic: false })).unwrap_or(false));
        let focused_graphic = if same(&go, &vm.eng.focus) {
            comps.iter().find_map(|c| with_comp(c, |k| match &k.k {
                CK::Sel(s) if k.enabled && s.interactable && !s.target_graphic.is_null() =>
                    Some(s.target_graphic.clone()),
                _ => None,
            }).flatten()).or_else(|| comps.iter().find(|c| {
                with_comp(c, |k| k.enabled && matches!(k.k, CK::Image(_) | CK::RawImage(_))).unwrap_or(false)
            }).cloned())
        } else {
            None
        };
        for c in &comps {
            let enabled = with_comp(c, |k| k.enabled).unwrap_or(false);
            if !enabled {
                continue;
            }
            enum D {
                Img(ImageSnap),
                Txt(TextSnap),
                Mask,
                None,
            }
            let d = with_comp(c, |k| match &k.k {
                CK::Image(i) => D::Img(ImageSnap::from(i)),
                CK::RawImage(i) => D::Img(ImageSnap::from(i)),
                CK::Text(t) => D::Txt(TextSnap::from(t)),
                CK::Mask { .. } => D::Mask,
                _ => D::None,
            })
            .unwrap_or(D::None);
            match d {
                D::Img(_) if hide_graphic => {}
                D::Img(mut i) => {
                    if let Some(m) = self.tint_for(c) {
                        (0..4).for_each(|k| i.color[k] *= m[k]);
                    }
                    let focused = focused_graphic.as_ref().is_some_and(|target| same(target, c));
                    self.draw_image(vm, &x, &i, graphic_clip, focused)
                }
                D::Txt(mut t) => {
                    if vm.eng.desktop_layout.pc_resolutions {
                        if let Some(label) = crate::desktop_layout::pc_resolution_label(&t.text) {
                            t.text = label;
                        }
                    }
                    if let Some(m) = self.tint_for(c) {
                        (0..4).for_each(|k| t.color[k] *= m[k]);
                    }
                    self.draw_text(vm, c, &x, &t, graphic_clip)
                }
                D::Mask => {
                    let r = self.rect_aabb(&x, [-x.pivot[0] * x.w, -x.pivot[1] * x.h, x.w, x.h]);
                    child_clip = Clip { x0: child_clip.x0.max(r.x0), y0: child_clip.y0.max(r.y0), x1: child_clip.x1.min(r.x1), y1: child_clip.y1.min(r.y1) };
                }
                D::None => {}
            }
        }
        let children = with_tr(tr, |t| t.children.clone()).unwrap_or_default();
        for ch in children {
            if self.tooltip.as_ref().is_some_and(|tooltip| same(tooltip, &ch)) || crate::motion::is_follower(vm, &ch) {
                continue;
            }
            let child = if self.drawing_overlay {
                with_tr(&ch, |t| crate::ui::child_xf(&x, t))
            } else {
                view_child_xf(vm, &x, &ch)
            };
            let cx = match child {
                Some(v) => v,
                None => continue,
            };
            self.draw_node(vm, &ch, cx, child_clip, depth + 1);
        }
        if pushed {
            self.tints.pop();
        }
        // memorise the subtree
        with_go(&go, |g| {
            g.dirty = false;
            if cacheable {
                g.cache = Some(Box::new(NodeCache { gen, key, clip: ck, quads: Rc::new(self.quads[start..].to_vec()) }));
            } else {
                g.cache = None;
            }
        });
    }

    fn tint_for(&self, comp: &Value) -> Option<[f32; 4]> {
        self.tints.iter().rev().find(|(t, _)| same(t, comp)).map(|t| t.1)
    }

    // ---------------------------------------------------------------- geometry
    fn projected_clip(&self, width: f32) -> Clip {
        Clip {
            x0: self.ox.round() as i32,
            y0: self.oy.round() as i32,
            x1: (self.ox + width * self.scale).round() as i32,
            y1: (self.oy + self.vh * self.scale).round() as i32,
        }
    }

    fn frame_clip(&self, vm: &Vm) -> Clip {
        let device = Clip { x0: 0, y0: 0, x1: self.w as i32, y1: self.h as i32 };
        match self.regions.as_slice() {
            [] => device.intersect(self.projected_clip(vm.eng.desktop_layout.width().max(vm.eng.screen.0 as f32))),
            [region] => {
                let [x, y, w, h] = region.dest();
                device.intersect(Clip { x0: x.round() as i32, y0: y.round() as i32, x1: (x + w).round() as i32, y1: (y + h).round() as i32 })
            }
            _ => device,
        }
    }

    fn graphic_clip(&self, vm: &Vm, tr: &Value, clip: Clip) -> Clip {
        if self.regions.is_empty() && !self.drawing_overlay {
            // A parent can contain both parked and projected children. Bound its
            // own graphics without clipping the independently placed children.
            // The tooltip overlay uses the existing frame clip for its entire subtree.
            clip.intersect(self.projected_clip(view_width(vm, tr)))
        } else {
            clip
        }
    }

    fn to_dev(&self, p: [f32; 2]) -> [f32; 2] {
        [self.ox + p[0] * self.scale, self.oy + (self.vh - p[1]) * self.scale]
    }

    fn corners(&self, x: &Xf, r: [f32; 4]) -> ([f32; 2], [f32; 2], [f32; 2]) {
        let (c, s) = (x.rot.cos(), x.rot.sin());
        let p = |lx: f32, ly: f32| {
            let (sx, sy) = (lx * x.scale[0], ly * x.scale[1]);
            self.to_dev([x.origin[0] + c * sx - s * sy, x.origin[1] + s * sx + c * sy])
        };
        let o = p(r[0], r[1]);
        let u = p(r[0] + r[2], r[1]);
        let v = p(r[0], r[1] + r[3]);
        (o, [u[0] - o[0], u[1] - o[1]], [v[0] - o[0], v[1] - o[1]])
    }

    fn rect_aabb(&self, x: &Xf, r: [f32; 4]) -> Clip {
        let (o, u, v) = self.corners(x, r);
        let xs = [o[0], o[0] + u[0], o[0] + v[0], o[0] + u[0] + v[0]];
        let ys = [o[1], o[1] + u[1], o[1] + v[1], o[1] + u[1] + v[1]];
        let mn = |a: &[f32; 4]| a.iter().cloned().fold(f32::MAX, f32::min);
        let mx = |a: &[f32; 4]| a.iter().cloned().fold(f32::MIN, f32::max);
        Clip { x0: mn(&xs).floor() as i32, y0: mn(&ys).floor() as i32, x1: mx(&xs).ceil() as i32, y1: mx(&ys).ceil() as i32 }
    }

    /// Adds a quad: local rect `r`, texture + uv in sprite pixels (v up).
    #[allow(clippy::too_many_arguments)]
    fn quad(&mut self, x: &Xf, r: [f32; 4], tex: Option<&(u32, Rc<Tex>)>, uv: [f32; 4], col: [f32; 4], clip: Clip) {
        if r[2] == 0.0 || r[3] == 0.0 || col[3] <= 0.0 || clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
            return;
        }
        let (o, u, v) = self.corners(x, r);
        let p = [o, [o[0] + u[0], o[1] + u[1]], [o[0] + u[0] + v[0], o[1] + u[1] + v[1]], [o[0] + v[0], o[1] + v[1]]];
        // quick cull: completely outside the clip
        let xs = [p[0][0], p[1][0], p[2][0], p[3][0]];
        let ys = [p[0][1], p[1][1], p[2][1], p[3][1]];
        if xs.iter().cloned().fold(f32::MIN, f32::max) < clip.x0 as f32
            || xs.iter().cloned().fold(f32::MAX, f32::min) > clip.x1 as f32
            || ys.iter().cloned().fold(f32::MIN, f32::max) < clip.y0 as f32
            || ys.iter().cloned().fold(f32::MAX, f32::min) > clip.y1 as f32
        {
            return;
        }
        let (id, tuv) = match tex {
            Some((id, t)) => {
                let (tw, th) = (t.w as f32, t.h as f32);
                // sprite uv (v up) -> normalised, v down
                let (u0, u1) = (uv[0] / tw, uv[2] / tw);
                let (v0, v1) = ((th - uv[1]) / th, (th - uv[3]) / th);
                (*id, [[u0, v0], [u1, v0], [u1, v1], [u0, v1]])
            }
            None => (TEX_WHITE, [[0.0; 2]; 4]),
        };
        self.draws += 1;
        self.quads.push(Quad { p, uv: tuv, col, tex: id, nearest: false, clip: [clip.x0, clip.y0, clip.x1, clip.y1] });
    }

    fn draw_image(&mut self, vm: &mut Vm, x: &Xf, im: &ImageSnap, clip: Clip, focused: bool) {
        let dir = vm.eng.assets_dir.clone();
        let local = [-x.pivot[0] * x.w, -x.pivot[1] * x.h, x.w, x.h];
        let sp = with_comp(&im.sprite, |k| match &k.k {
            CK::Sprite(s) => Some((s.tex.clone(), s.rect, s.border, s.ppu)),
            _ => None,
        })
        .flatten();
        let (tex, rect, border, ppu) = match sp {
            Some((t, r, b, p)) => {
                let key = with_comp(&t, |k| match &k.k {
                    CK::Texture(t) => t.key.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
                (self.tex(&dir, &key), r, b, p)
            }
            None => (None, [0.0; 4], [0.0; 4], 100.0),
        };
        if tex.is_none() && !im.sprite.is_null() {
            return;
        }
        let col = im.color;
        match (im.typ, &tex) {
            (1, Some(_)) if border.iter().any(|b| *b > 0.0) => {
                let k = 100.0 / ppu.max(1.0);
                let (bl, bb, br, bt) = (border[0] * k, border[1] * k, border[2] * k, border[3] * k);
                let (w, h) = (local[2], local[3]);
                let xs = [0.0, bl.min(w * 0.5), (w - br).max(w * 0.5), w];
                let ys = [0.0, bb.min(h * 0.5), (h - bt).max(h * 0.5), h];
                let us = [rect[0], rect[0] + border[0], rect[0] + rect[2] - border[2], rect[0] + rect[2]];
                let vs = [rect[1], rect[1] + border[1], rect[1] + rect[3] - border[3], rect[1] + rect[3]];
                for j in 0..3 {
                    for i in 0..3 {
                        if i == 1 && j == 1 && !im.fill_center {
                            continue;
                        }
                        let r = [local[0] + xs[i], local[1] + ys[j], xs[i + 1] - xs[i], ys[j + 1] - ys[j]];
                        let color = if focused && i == 1 && j == 1 {
                            [1.0, 0.89, 0.52, col[3]]
                        } else {
                            col
                        };
                        self.quad(x, r, tex.as_ref(), [us[i], vs[j], us[i + 1], vs[j + 1]], color, clip);
                    }
                }
            }
            (3, _) => {
                let a = im.fill_amount.clamp(0.0, 1.0);
                let (mut r, mut uv) = (local, [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]]);
                match im.fill_method {
                    0 => {
                        let wf = r[2] * a;
                        let uf = (uv[2] - uv[0]) * a;
                        if im.fill_origin == 1 {
                            r[0] += r[2] - wf;
                            uv[0] = uv[2] - uf;
                        } else {
                            uv[2] = uv[0] + uf;
                        }
                        r[2] = wf;
                    }
                    1 => {
                        let hf = r[3] * a;
                        let vf = (uv[3] - uv[1]) * a;
                        if im.fill_origin == 1 {
                            r[1] += r[3] - hf;
                            uv[1] = uv[3] - vf;
                        } else {
                            uv[3] = uv[1] + vf;
                        }
                        r[3] = hf;
                    }
                    _ => {}
                }
                self.quad(x, r, tex.as_ref(), uv, col, clip);
            }
            _ => {
                let mut r = local;
                if im.preserve_aspect && rect[2] > 0.0 && rect[3] > 0.0 {
                    let (ar, tr_) = (rect[2] / rect[3], r[2] / r[3]);
                    if ar > tr_ {
                        let nh = r[2] / ar;
                        r[1] += (r[3] - nh) * 0.5;
                        r[3] = nh;
                    } else {
                        let nw = r[3] * ar;
                        r[0] += (r[2] - nw) * 0.5;
                        r[2] = nw;
                    }
                }
                self.quad(x, r, tex.as_ref(), [rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]], col, clip);
                if focused {
                    let inset = 3.0f32.min(r[2].min(r[3]) / 4.0);
                    let inside = [r[0] + inset, r[1] + inset, r[2] - 2.0 * inset, r[3] - 2.0 * inset];
                    self.quad(x, inside, None, [0.0; 4], [1.0, 0.85, 0.37, 0.48], clip);
                }
            }
        }
    }

    // ---------------------------------------------------------------- text
    fn glyph(&mut self, fid: usize, font: &fontdue::Font, ch: char, px: f32) -> Option<(f32, f32, f32, f32, fontdue::Metrics)> {
        let key = (fid, ch, (px * 10.0) as u32);
        if let Some(g) = self.atlas.map.get(&key) {
            return Some(*g);
        }
        let (m, bm) = font.rasterize(ch, px.max(1.0));
        if m.width == 0 || m.height == 0 {
            let g = (0.0, 0.0, 0.0, 0.0, m);
            self.atlas.map.insert(key, g);
            return Some(g);
        }
        let a = &mut self.atlas;
        if a.x + m.width + 1 > ATLAS_SIZE {
            a.x = 1;
            a.y += a.row_h + 1;
            a.row_h = 0;
        }
        if a.y + m.height + 1 > ATLAS_SIZE {
            return None;
        }
        for gy in 0..m.height {
            for gx in 0..m.width {
                let i = ((a.y + gy) * ATLAS_SIZE + a.x + gx) * 4;
                a.px[i] = 255;
                a.px[i + 1] = 255;
                a.px[i + 2] = 255;
                a.px[i + 3] = bm[gy * m.width + gx];
            }
        }
        let s = ATLAS_SIZE as f32;
        let g = (a.x as f32 / s, a.y as f32 / s, (a.x + m.width) as f32 / s, (a.y + m.height) as f32 / s, m);
        a.x += m.width + 1;
        a.row_h = a.row_h.max(m.height);
        a.dirty = true;
        a.map.insert(key, g);
        Some(g)
    }

    fn draw_text(&mut self, vm: &mut Vm, comp: &Value, x: &Xf, t: &TextSnap, clip: Clip) {
        if t.text.is_empty() || t.color[3] <= 0.0 || x.w <= 0.0 {
            return;
        }
        let key = text_key(t, x);
        let lay = match t.cached.as_ref().filter(|c| c.0 == key) {
            Some(c) => c.1.clone(),
            None => {
                let l = match self.layout_text(vm, x, t) {
                    Some(l) => Rc::new(l),
                    None => return,
                };
                let l2 = l.clone();
                with_comp(comp, |k| {
                    if let CK::Text(td) = &mut k.k {
                        td.layout = Some((key, l2));
                    }
                });
                l
            }
        };
        // vertical truncation area
        let tclip = if t.vov == 0 {
            let c = self.rect_aabb(x, [-x.pivot[0] * x.w, -x.pivot[1] * x.h, x.w, x.h]);
            Clip { x0: clip.x0.max(c.x0), y0: clip.y0.max(c.y0), x1: clip.x1.min(c.x1), y1: clip.y1.min(c.y1) }
        } else {
            clip
        };
        if tclip.x0 >= tclip.x1 || tclip.y0 >= tclip.y1 {
            return;
        }
        let dev_scale = self.scale * x.scale[1].abs();
        let (c, s) = (x.rot.cos(), x.rot.sin());
        for g in lay.glyphs.iter() {
            let (f, fid) = if g.bold { (&lay.fb, lay.fb_id) } else { (&lay.fr, lay.fr_id) };
            let px = g.size * dev_scale;
            if let Some((u0, v0, u1, v1, gm)) = self.glyph(fid, f, g.ch, px) {
                if gm.width == 0 {
                    continue;
                }
                let (lx, ly) = (g.pen * x.scale[0], g.base_y * x.scale[1]);
                let wp = [x.origin[0] + c * lx - s * ly, x.origin[1] + s * lx + c * ly];
                let dp = self.to_dev(wp);
                // placement measured against Unity (main screen 960x600, 10 text areas): x by floor, y by round, per glyph.
                // Tried and rejected: round/round (2x more differences), common offset fitted on the 1st glyph (round: +28 %, floor: +57 %).
                let gx = (dp[0] + gm.xmin as f32).floor();
                let gy = (dp[1] - gm.ymin as f32 - gm.height as f32).round();
                let (gw, gh) = (gm.width as f32, gm.height as f32);
                let n = if g.fake_bold { 2 } else { 1 };
                for i in 0..n {
                    let dx = i as f32;
                    self.draws += 1;
                    self.quads.push(Quad {
                        p: [[gx + dx, gy], [gx + dx + gw, gy], [gx + dx + gw, gy + gh], [gx + dx, gy + gh]],
                        uv: [[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
                        col: g.color,
                        tex: TEX_ATLAS,
                        nearest: true,
                        clip: [tclip.x0, tclip.y0, tclip.x1, tclip.y1],
                    });
                }
            }
        }
    }

    /// Preferred dimensions of a Text: (width without wrapping, height for `width`).
    pub fn measure_text_comp(&mut self, vm: &mut Vm, comp: &Value, width: Option<f32>) -> Option<(f32, f32)> {
        let t = with_comp(comp, |k| if let CK::Text(t) = &k.k { Some(TextSnap::from(t)) } else { None }).flatten()?;
        if t.text.is_empty() {
            return Some((0.0, 0.0));
        }
        let runs = parse_rich(&t.text, t.rich, t.color, t.size as f32, t.style & 1 != 0);
        let fr = self.pick_font(vm, &t.font, false)?;
        let fb = self.pick_font(vm, &t.font, true)?;
        let wrap = t.hov == 0 && width.is_some();
        let rw = width.unwrap_or(f32::MAX);
        let (mut max_w, mut cur_w, mut lines) = (0f32, 0f32, 1usize);
        let mut last_space_w: Option<f32> = None;
        for r in &runs {
            if r.ch == '\n' {
                max_w = max_w.max(cur_w);
                cur_w = 0.0;
                lines += 1;
                last_space_w = None;
                continue;
            }
            let f = if r.bold { &fb.1 } else { &fr.1 };
            let adv = f.metrics(r.ch, r.size).advance_width.round(); // Unity (FreeType): whole-pixel advances
            if wrap && r.ch != ' ' && cur_w + adv > rw + 0.01 && cur_w > 0.0 {
                match last_space_w {
                    Some(ls) => {
                        max_w = max_w.max(ls);
                        cur_w -= ls;
                    }
                    None => {
                        max_w = max_w.max(cur_w);
                        cur_w = 0.0;
                    }
                }
                lines += 1;
                last_space_w = None;
            }
            cur_w += adv;
            if r.ch == ' ' {
                last_space_w = Some(cur_w);
            }
        }
        max_w = max_w.max(cur_w);
        let m = fr.1.horizontal_line_metrics(t.size as f32)?;
        let lh = m.new_line_size.max(t.size as f32) * t.line_spacing;
        Some((max_w, lines as f32 * lh))
    }

    fn layout_text(&mut self, vm: &mut Vm, x: &Xf, t: &TextSnap) -> Option<TextLayout> {
        let runs = parse_rich(&t.text, t.rich, t.color, t.size as f32, t.style & 1 != 0);
        let mut size = t.size as f32;
        let mut lines;
        let mut line_h;
        let (rw, rh) = (x.w, x.h);
        let font_reg = self.pick_font(vm, &t.font, false);
        let font_bold = self.pick_font(vm, &t.font, true);
        let (fr, fb) = match (&font_reg, &font_bold) {
            (Some(a), Some(b)) => (a.clone(), b.clone()),
            _ => return None,
        };
        let base_size = size;
        let layout = |size_scale: f32, lines_out: &mut Vec<Vec<(usize, f32)>>, lh: &mut f32| {
            lines_out.clear();
            let mut cur: Vec<(usize, f32)> = vec![];
            let mut cur_w = 0f32;
            let mut last_space: Option<usize> = None;
            let mut i = 0;
            while i < runs.len() {
                let r = &runs[i];
                if r.ch == '\n' {
                    lines_out.push(std::mem::take(&mut cur));
                    cur_w = 0.0;
                    last_space = None;
                    i += 1;
                    continue;
                }
                let f = if r.bold { &fb.1 } else { &fr.1 };
                let adv = f.metrics(r.ch, r.size * size_scale).advance_width.round(); // Unity (FreeType): whole-pixel advances
                if t.hov == 0 && r.ch != ' ' && cur_w + adv > rw + 0.01 && !cur.is_empty() {
                    if let Some(ls) = last_space {
                        let tail: Vec<(usize, f32)> = cur.split_off(ls + 1);
                        lines_out.push(std::mem::take(&mut cur));
                        cur = tail;
                        cur_w = cur.iter().map(|c| c.1).sum();
                        last_space = None;
                    } else {
                        lines_out.push(std::mem::take(&mut cur));
                        cur_w = 0.0;
                    }
                }
                cur.push((i, adv));
                cur_w += adv;
                if r.ch == ' ' {
                    last_space = Some(cur.len() - 1);
                }
                i += 1;
            }
            lines_out.push(cur);
            let m = fr.1.horizontal_line_metrics(base_size * size_scale).unwrap_or(fontdue::LineMetrics { ascent: base_size, descent: 0.0, line_gap: 0.0, new_line_size: base_size * 1.15 });
            *lh = m.new_line_size.max(base_size * size_scale) * t.line_spacing;
        };
        lines = vec![];
        line_h = 0.0;
        layout(1.0, &mut lines, &mut line_h);
        if t.best_fit {
            let mut s = t.max_size as f32;
            let mut found = false;
            while s >= t.min_size as f32 {
                let k = s / t.size.max(1) as f32;
                layout(k, &mut lines, &mut line_h);
                if lines.len() as f32 * line_h <= rh + 0.01 && lines.iter().all(|l| l.iter().map(|c| c.1).sum::<f32>() <= rw + 0.01) {
                    size = s;
                    found = true;
                    break;
                }
                s -= 1.0;
            }
            if !found {
                size = t.min_size as f32;
                let k = size / t.size.max(1) as f32;
                layout(k, &mut lines, &mut line_h);
            }
        }
        let k = size / t.size.max(1) as f32;
        let m = fr.1.horizontal_line_metrics(size).unwrap_or(fontdue::LineMetrics { ascent: size * 0.9, descent: -size * 0.2, line_gap: 0.0, new_line_size: size * 1.15 });
        // Unity ignores trailing empty lines (text ending with "\n") for vertical alignment (verified: shop button)
        let shown = lines.len() - lines.iter().rev().take_while(|l| l.is_empty()).count().min(lines.len().saturating_sub(1));
        let block_h = shown as f32 * line_h;
        let (ha, va) = (t.align % 3, t.align / 3);
        let top = -x.pivot[1] * x.h + x.h;
        let left = -x.pivot[0] * x.w;
        let y0 = match va {
            0 => top,
            1 => top - (rh - block_h) * 0.5,
            _ => top - (rh - block_h),
        };
        let mut glyphs = vec![];
        for (li, line) in lines.iter().enumerate() {
            let lw: f32 = line.iter().map(|c| c.1).sum();
            let lw = if line.last().map(|c| runs[c.0].ch == ' ').unwrap_or(false) { lw - line.last().unwrap().1 } else { lw };
            let x0 = match ha {
                0 => left,
                1 => left + (rw - lw) * 0.5,
                _ => left + rw - lw,
            };
            let base_y = y0 - li as f32 * line_h - m.ascent;
            let mut pen = x0;
            for &(ri, adv) in line {
                let r = &runs[ri];
                if r.ch != ' ' && r.ch != '\t' {
                    let same_font = std::ptr::eq(&*fr.1, &*fb.1);
                    glyphs.push(LGlyph { pen, base_y, ch: r.ch, size: r.size * k, color: r.color, bold: r.bold, fake_bold: r.bold && same_font });
                }
                pen += adv;
            }
        }
        Some(TextLayout { glyphs, fr: fr.1.clone(), fb: fb.1.clone(), fr_id: fr.0, fb_id: fb.0 })
    }

    // ---------------------------------------------------------------- software backend
    /// Software rasterization, in horizontal bands over all cores (each pixel is independent;
    /// quad order is kept within each band, so the result is identical to single-threaded rendering).
    pub fn raster(&mut self) {
        let quads = std::mem::take(&mut self.quads);
        // textures resolved once (read-only during rasterization)
        let texs: Vec<Option<Rc<Tex>>> = quads.iter().map(|q| if q.tex >= 2 { self.tex_by_id(q.tex) } else { None }).collect();
        let (w, h) = (self.w, self.h);
        let ctx = Ctx { quads: &quads, texs: &texs, atlas: &self.atlas.px, w, h };
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 16).min(h.max(1));
        let band = h.div_ceil(n);
        std::thread::scope(|s| {
            for (bi, chunk) in self.fb.chunks_mut(band * w * 4).enumerate() {
                let ctx = &ctx;
                s.spawn(move || raster_band(ctx, chunk, bi * band));
            }
        });
        self.quads = quads;
    }

    pub fn save_png(&self, path: &str) {
        let f = std::fs::File::create(path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&self.fb).unwrap();
    }
}

pub struct NodeCache {
    pub gen: u32,
    pub key: [f32; 13],
    pub clip: [i32; 4],
    pub quads: Rc<Vec<Quad>>,
}

pub struct LGlyph {
    pub pen: f32,
    pub base_y: f32,
    pub ch: char,
    pub size: f32,
    pub color: [f32; 4],
    pub bold: bool,
    pub fake_bold: bool,
}

pub struct TextLayout {
    pub glyphs: Vec<LGlyph>,
    pub fr: Rc<fontdue::Font>,
    pub fb: Rc<fontdue::Font>,
    pub fr_id: usize,
    pub fb_id: usize,
}

fn text_key(t: &TextSnap, x: &Xf) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = crate::vm::FxHasher::default();
    t.text.hash(&mut h);
    (t.size, t.style, t.best_fit, t.min_size, t.max_size, t.align, t.rich, t.hov, t.vov).hash(&mut h);
    t.line_spacing.to_bits().hash(&mut h);
    for c in t.color {
        c.to_bits().hash(&mut h);
    }
    x.w.to_bits().hash(&mut h);
    x.h.to_bits().hash(&mut h);
    x.pivot[0].to_bits().hash(&mut h);
    x.pivot[1].to_bits().hash(&mut h);
    if let Value::Obj(o) = &t.font {
        (Rc::as_ptr(o) as usize).hash(&mut h);
    }
    h.finish()
}

struct ImageSnap {
    sprite: Value,
    color: [f32; 4],
    typ: i32,
    fill_center: bool,
    fill_method: i32,
    fill_amount: f32,
    fill_origin: i32,
    preserve_aspect: bool,
}
impl ImageSnap {
    fn from(i: &ImageD) -> ImageSnap {
        ImageSnap { sprite: i.sprite.clone(), color: i.color, typ: i.typ, fill_center: i.fill_center, fill_method: i.fill_method, fill_amount: i.fill_amount, fill_origin: i.fill_origin, preserve_aspect: i.preserve_aspect }
    }
}

struct TextSnap {
    text: String,
    font: Value,
    size: i32,
    style: i32,
    best_fit: bool,
    min_size: i32,
    max_size: i32,
    align: i32,
    rich: bool,
    hov: i32,
    vov: i32,
    line_spacing: f32,
    color: [f32; 4],
    cached: Option<(u64, Rc<TextLayout>)>,
}
impl TextSnap {
    fn from(t: &TextD) -> TextSnap {
        TextSnap { text: t.text.clone(), font: t.font.clone(), size: t.size, style: t.style, best_fit: t.best_fit, min_size: t.min_size, max_size: t.max_size, align: t.align, rich: t.rich, hov: t.hov, vov: t.vov, line_spacing: t.line_spacing, color: t.color, cached: t.layout.clone() }
    }
}

pub fn load_png_bytes(b: &[u8]) -> Option<Tex> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(b));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let mut px = vec![255u8; w * h * 4];
    match info.color_type {
        png::ColorType::Rgba => px.copy_from_slice(&buf[..w * h * 4]),
        png::ColorType::Rgb => {
            for i in 0..w * h {
                px[i * 4..i * 4 + 3].copy_from_slice(&buf[i * 3..i * 3 + 3]);
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..w * h {
                px[i * 4] = buf[i];
                px[i * 4 + 1] = buf[i];
                px[i * 4 + 2] = buf[i];
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..w * h {
                px[i * 4] = buf[i * 2];
                px[i * 4 + 1] = buf[i * 2];
                px[i * 4 + 2] = buf[i * 2];
                px[i * 4 + 3] = buf[i * 2 + 1];
            }
        }
        _ => return None,
    }
    Some(Tex { w, h, px })
}

fn sample(t: &Tex, u: f32, v: f32) -> [f32; 4] {
    let (x, y) = (u - 0.5, v - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let g = |ix: i32, iy: i32| -> [f32; 4] {
        let cx = ix.clamp(0, t.w as i32 - 1) as usize;
        let cy = iy.clamp(0, t.h as i32 - 1) as usize;
        let i = (cy * t.w + cx) * 4;
        [t.px[i] as f32 / 255.0, t.px[i + 1] as f32 / 255.0, t.px[i + 2] as f32 / 255.0, t.px[i + 3] as f32 / 255.0]
    };
    let (a, b, c, d) = (g(x0 as i32, y0 as i32), g(x0 as i32 + 1, y0 as i32), g(x0 as i32, y0 as i32 + 1), g(x0 as i32 + 1, y0 as i32 + 1));
    let mut o = [0.0; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bot = c[k] + (d[k] - c[k]) * fx;
        o[k] = top + (bot - top) * fy;
    }
    o
}

fn parse_rich(s: &str, rich: bool, base: [f32; 4], size: f32, bold0: bool) -> Vec<Run> {
    let mut out = vec![];
    let mut colors = vec![base];
    let mut sizes = vec![size];
    let mut bold = bold0;
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if rich && cs[i] == '<' {
            if let Some(j) = cs[i..].iter().position(|c| *c == '>') {
                let tag: String = cs[i + 1..i + j].iter().collect();
                let t = tag.to_lowercase();
                let mut handled = true;
                if t == "b" {
                    bold = true;
                } else if t == "/b" {
                    bold = bold0;
                } else if t == "i" || t == "/i" {
                } else if t == "/color" {
                    if colors.len() > 1 {
                        colors.pop();
                    }
                } else if t == "/size" {
                    if sizes.len() > 1 {
                        sizes.pop();
                    }
                } else if let Some(v) = t.strip_prefix("color=") {
                    colors.push(parse_color(v, base));
                } else if let Some(v) = t.strip_prefix("size=") {
                    sizes.push(v.parse().unwrap_or(size));
                } else {
                    handled = false;
                }
                if handled {
                    i += j + 1;
                    continue;
                }
            }
        }
        out.push(Run { ch: cs[i], color: *colors.last().unwrap(), bold, size: *sizes.last().unwrap() });
        i += 1;
    }
    out
}

fn parse_color(v: &str, base: [f32; 4]) -> [f32; 4] {
    let named = match v {
        "red" => Some([1.0, 0.0, 0.0]),
        "green" => Some([0.0, 0.5, 0.0]),
        "blue" => Some([0.0, 0.0, 1.0]),
        "white" => Some([1.0, 1.0, 1.0]),
        "black" => Some([0.0, 0.0, 0.0]),
        "yellow" => Some([1.0, 0.92, 0.016]),
        "cyan" => Some([0.0, 1.0, 1.0]),
        "magenta" => Some([1.0, 0.0, 1.0]),
        "orange" => Some([1.0, 0.5, 0.0]),
        "grey" | "gray" => Some([0.5, 0.5, 0.5]),
        _ => None,
    };
    if let Some(c) = named {
        return [c[0], c[1], c[2], base[3]];
    }
    let h = v.trim_start_matches('#');
    let p = |a: usize| u8::from_str_radix(h.get(a..a + 2).unwrap_or("ff"), 16).unwrap_or(255) as f32 / 255.0;
    if h.len() >= 6 {
        [p(0), p(2), p(4), if h.len() >= 8 { p(6) * base[3] } else { base[3] }]
    } else {
        base
    }
}

/// Data shared read-only between the rasterization threads.
/// ponytail: `Rc<Tex>` is not `Sync`, but no counter is touched during rasterization (references only).
struct Ctx<'a> {
    quads: &'a [Quad],
    texs: &'a [Option<Rc<Tex>>],
    atlas: &'a [u8],
    w: usize,
    h: usize,
}
unsafe impl Sync for Ctx<'_> {}

/// Rasterizes all quads into rows `y_off .. y_off + fb.len()/(4w)`.
fn raster_band(c: &Ctx, fb: &mut [u8], y_off: usize) {
    for p in fb.chunks_exact_mut(4) {
        p[0] = 0;
        p[1] = 0;
        p[2] = 0;
        p[3] = 255;
    }
    let (w, rows) = (c.w, fb.len() / (4 * c.w));
    let (y_lo, y_hi) = (y_off as i32, (y_off + rows) as i32);
    for (qi, q) in c.quads.iter().enumerate() {
        let (o, u, v) = (q.p[0], [q.p[1][0] - q.p[0][0], q.p[1][1] - q.p[0][1]], [q.p[3][0] - q.p[0][0], q.p[3][1] - q.p[0][1]]);
        let xs = [q.p[0][0], q.p[1][0], q.p[2][0], q.p[3][0]];
        let ys = [q.p[0][1], q.p[1][1], q.p[2][1], q.p[3][1]];
        let bx0 = (xs.iter().cloned().fold(f32::MAX, f32::min).floor() as i32).max(q.clip[0]).max(0);
        let bx1 = (xs.iter().cloned().fold(f32::MIN, f32::max).ceil() as i32).min(q.clip[2]).min(c.w as i32);
        let by0 = (ys.iter().cloned().fold(f32::MAX, f32::min).floor() as i32).max(q.clip[1]).max(0).max(y_lo);
        let by1 = (ys.iter().cloned().fold(f32::MIN, f32::max).ceil() as i32).min(q.clip[3]).min(c.h as i32).min(y_hi);
        if bx0 >= bx1 || by0 >= by1 {
            continue;
        }
        let det = u[0] * v[1] - u[1] * v[0];
        if det.abs() < 1e-6 {
            continue;
        }
        let inv = [v[1] / det, -v[0] / det, -u[1] / det, u[0] / det];
        let tex: Option<&Tex> = c.texs[qi].as_deref();
        for py in by0..by1 {
            for px in bx0..bx1 {
                let (dx, dy) = (px as f32 + 0.5 - o[0], py as f32 + 0.5 - o[1]);
                let s = inv[0] * dx + inv[1] * dy;
                let t = inv[2] * dx + inv[3] * dy;
                if !(0.0..=1.0).contains(&s) || !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let mut col = q.col;
                if q.tex != TEX_WHITE {
                    let uu = q.uv[0][0] + (q.uv[1][0] - q.uv[0][0]) * s + (q.uv[3][0] - q.uv[0][0]) * t;
                    let vv = q.uv[0][1] + (q.uv[1][1] - q.uv[0][1]) * s + (q.uv[3][1] - q.uv[0][1]) * t;
                    let tc = if q.tex == TEX_ATLAS {
                        let (tx, ty) = ((uu * ATLAS_SIZE as f32) as usize, (vv * ATLAS_SIZE as f32) as usize);
                        let i = (ty.min(ATLAS_SIZE - 1) * ATLAS_SIZE + tx.min(ATLAS_SIZE - 1)) * 4;
                        [1.0, 1.0, 1.0, c.atlas[i + 3] as f32 / 255.0]
                    } else if let Some(tx) = tex {
                        sample(tx, uu * tx.w as f32, vv * tx.h as f32)
                    } else {
                        [1.0; 4]
                    };
                    col = [col[0] * tc[0], col[1] * tc[1], col[2] * tc[2], col[3] * tc[3]];
                }
                let a = col[3];
                if a <= 0.0 {
                    continue;
                }
                let i = ((py as usize - y_off) * w + px as usize) * 4;
                let d = &mut fb[i..i + 4];
                for k in 0..3 {
                    d[k] = (col[k] * 255.0 * a + d[k] as f32 * (1.0 - a)) as u8;
                }
            }
        }
    }
}

fn next_gen() -> u32 {
    static G: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    G.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
