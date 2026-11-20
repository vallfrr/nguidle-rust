use crate::ui::{view_width, view_xf};
use crate::unity::{active_in_hierarchy, go_of, same, transform_of, with_comp, CK, SelKind};
use crate::value::Value;
use crate::Vm;

#[derive(Default)]
pub(crate) struct Dropdown {
    pub comp: Value,
    first: usize,
    pressed: Option<usize>,
    cursor: Option<usize>,
}

pub(crate) struct Popup {
    pub x: f32,
    pub top: f32,
    pub width: f32,
    pub row_height: f32,
    pub rows: usize,
    pub first: usize,
    pub selected: usize,
    pub cursor: Option<usize>,
    pub caption: Value,
    pub options: Vec<Value>,
}

impl Popup {
    pub fn row_at(&self, point: (f32, f32)) -> Option<usize> {
        let (x, y) = point;
        let row = ((self.top - y) / self.row_height).floor();
        (x >= self.x && x < self.x + self.width && row >= 0.0 && row < self.rows as f32)
            .then(|| self.first + row as usize)
    }

    pub fn scrollbar_width(&self) -> f32 {
        if self.options.len() > self.rows { 10.0 } else { 0.0 }
    }

    fn scroll_at(&self, point: (f32, f32)) -> Option<usize> {
        if self.row_at(point).is_none() || point.0 < self.x + self.width - self.scrollbar_width() {
            return None;
        }
        let fraction = (self.top - point.1) / (self.rows as f32 * self.row_height);
        Some((fraction * (self.options.len() - self.rows) as f32).round() as usize)
    }
}

pub(crate) fn popup(vm: &Vm) -> Option<Popup> {
    let comp = &vm.eng.dropdown.comp;
    if comp.is_null() || !active_in_hierarchy(&go_of(comp)) {
        return None;
    }
    let (caption, options, selected) = with_comp(comp, |c| match &c.k {
        CK::Sel(s) if c.enabled && s.kind == SelKind::Dropdown && s.interactable && !s.options.is_empty() =>
            Some((s.caption.clone(), s.options.clone(), s.value as usize)),
        _ => None,
    }).flatten()?;
    let tr = transform_of(comp);
    let x = view_xf(vm, &tr);
    let w = x.w * x.scale[0].abs();
    let h = x.h * x.scale[1].abs();
    let screen_w = view_width(vm, &tr);
    let screen_h = vm.eng.screen.1 as f32;
    let width = w.max(100.0).min(screen_w);
    let left = (x.origin[0] - x.pivot[0] * w).clamp(0.0, (screen_w - width).max(0.0));
    let bottom = (x.origin[1] - x.pivot[1] * h).clamp(0.0, screen_h);
    let top = (bottom + h).min(screen_h);
    let row_height = h.clamp(22.0, 30.0);
    let desired = options.len().min(8);
    let below = (bottom / row_height).floor() as usize;
    let above = ((screen_h - top) / row_height).floor() as usize;
    let opens_above = below < desired && above > below;
    let rows = desired.min(if opens_above { above } else { below });
    if rows == 0 {
        return None;
    }
    Some(Popup {
        x: left, top: if opens_above { top + row_height * rows as f32 } else { bottom },
        width, row_height, rows, first: vm.eng.dropdown.first.min(options.len() - rows),
        selected, cursor: vm.eng.dropdown.cursor, caption, options,
    })
}

impl Vm {
    pub(crate) fn open_dropdown(&mut self, comp: &Value) {
        self.eng.dropdown = Dropdown { comp: comp.clone(), ..Dropdown::default() };
        if let Some(p) = popup(self) {
            self.eng.dropdown.first = p.selected.min(p.options.len() - p.rows);
        }
    }

    pub(crate) fn hide_dropdown(&mut self, comp: &Value) {
        if same(comp, &self.eng.dropdown.comp) {
            self.eng.dropdown = Dropdown::default();
        }
    }

    pub(crate) fn dropdown_pointer(&mut self) -> bool {
        if self.eng.dropdown.comp.is_null() {
            return false;
        }
        let Some(p) = popup(self) else {
            self.eng.dropdown = Dropdown::default();
            return false;
        };
        if self.eng.keys_edge.remove(&27) {
            self.eng.dropdown = Dropdown::default();
            return true;
        }
        if let Some(first) = p.scroll_at(self.eng.mouse) {
            if self.eng.mouse_down && self.eng.mouse_button == 0 {
                self.eng.dropdown.first = first;
            }
            self.eng.dropdown.pressed = None;
            return true;
        }
        let row = p.row_at(self.eng.mouse);
        if self.eng.mouse_down_edge {
            self.eng.dropdown.cursor = None;
            self.eng.dropdown.pressed = row.filter(|_| self.eng.mouse_button == 0);
        }
        if self.eng.mouse_up_edge {
            let pressed = self.eng.dropdown.pressed;
            let comp = std::mem::take(&mut self.eng.dropdown).comp;
            if let Some(row) = row.filter(|r| Some(*r) == pressed) {
                crate::unity_api::set_dropdown_value(self, &comp, row as i32);
            }
        }
        true
    }

    pub(crate) fn dropdown_scroll(&mut self, notches: f32) -> bool {
        let Some(p) = popup(self) else { return false };
        self.eng.dropdown.first = p.first.saturating_add_signed(-(notches.round() as isize))
            .min(p.options.len() - p.rows);
        self.eng.dropdown.pressed = None;
        true
    }

    /// Handle focus navigation while the dropdown owns input.
    pub fn dropdown_navigate(&mut self, action: char) -> bool {
        let Some(p) = popup(self) else { return false };
        let cursor = p.cursor.unwrap_or(p.selected).min(p.options.len() - 1);
        match action {
            'u' | 'd' => {
                let i = cursor.saturating_add_signed(if action == 'u' { -1 } else { 1 })
                    .min(p.options.len() - 1);
                self.eng.dropdown.cursor = Some(i);
                self.eng.dropdown.first = p.first.clamp(i.saturating_add(1).saturating_sub(p.rows), i);
            }
            'a' => {
                let comp = std::mem::take(&mut self.eng.dropdown).comp;
                crate::unity_api::set_dropdown_value(self, &comp, cursor as i32);
            }
            'b' => self.eng.dropdown = Dropdown::default(),
            _ => {}
        }
        true
    }
}
