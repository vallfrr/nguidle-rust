use super::*;

impl Vm {
    pub fn enable_browser_saves(&mut self) {
        let timer = named_go(self, "Rebirth Time Text").unwrap_or_default();
        let timer_label = with_go(&timer, |go| go.comps.iter().find(|c| {
            with_comp(c, |c| matches!(c.k, CK::Text(_))) == Some(true)
        }).cloned()).flatten().unwrap_or_default();
        self.eng.save_slots.browser = Some(BrowserSlots {
            online: true, locked: 0, granted: None, requested: None, timer_label, file_request: None,
        });
    }

    pub fn browser_profile_changed(&mut self, online: bool, locked: u8, reload: bool) {
        let mut state = std::mem::take(&mut self.eng.save_slots);
        if let Some(browser) = &mut state.browser {
            let changed = online != browser.online || locked != browser.locked;
            browser.online = online;
            browser.locked = locked;
            if changed || reload {
                let dir = PathBuf::from(&self.eng.save_dir);
                if state.home_active {
                    let _ = show_home(self, &mut state, &dir);
                } else if matches!(state.mode, Some(Mode::Create | Mode::Save | Mode::Load)) {
                    state.entries = load_entries(&dir);
                    if !online { state.status = "Save server unavailable.".into(); }
                    else if state.status == "Save server unavailable." { state.status.clear(); }
                    draw_state(self, &state);
                }
            }
        }
        self.eng.save_slots = state;
    }

    pub fn browser_requested_slot(&self) -> Option<usize> {
        self.eng.save_slots.browser.as_ref()?.requested.as_ref().map(|(slot, _)| *slot)
    }

    pub fn browser_active_slot(&self) -> Option<usize> {
        self.eng.save_slots.active_slot
    }

    pub(crate) fn browser_saves_enabled(&self) -> bool {
        self.eng.save_slots.browser.is_some()
    }

    pub fn browser_finish_request(&mut self, granted: bool) -> Result<(), String> {
        let mut state = std::mem::take(&mut self.eng.save_slots);
        let request = state.browser.as_mut().and_then(|b| b.requested.take());
        let result = if let Some((slot, action)) = request {
            let dir = PathBuf::from(&self.eng.save_dir);
            let result = if granted {
                state.browser.as_mut().unwrap().granted = Some(slot);
                let result = perform(self, &mut state, &dir, action);
                state.browser.as_mut().unwrap().granted = None;
                result
            } else {
                Err("Save unavailable. It may be in use on another device.".into())
            };
            match &result {
                Ok(message) => state.status = message.clone(),
                Err(message) => {
                    state.status = message.clone();
                    if state.home_active {
                        if let Some(home) = &state.home { label(&home.details, message.clone()); }
                    }
                }
            }
            draw_state(self, &state);
            result.map(|_| ())
        } else {
            Err("The requested save operation was cancelled.".into())
        };
        self.eng.save_slots = state;
        result
    }

    pub fn browser_current_save(&mut self) -> Result<String, String> {
        if self.browser_active_slot().is_none() || self.at_desktop_start() {
            return Err("Load or create a game before exporting.".into());
        }
        current_snapshot(self).map(|save| save.to_string())
    }

    pub fn browser_export(&mut self, slot: Option<usize>) -> Result<String, String> {
        match slot {
            None => self.browser_current_save(),
            Some(index) if index < ROW_COUNT && !self.eng.save_slots.blocked(index) => {
                read_save(&slot_path(Path::new(&self.eng.save_dir), index)).map(|(data, _)| data)
            }
            _ => Err("Choose an available save.".into()),
        }
    }

    pub fn browser_file_request(&mut self) -> Option<(&'static str, Option<usize>)> {
        self.eng.save_slots.browser.as_mut()?.file_request.take()
    }

    pub fn browser_import_control(&self) -> Option<(usize, [f32; 4])> {
        let state = &self.eng.save_slots;
        let index = state.browser_import_target()?;
        let rect = crate::focus::node_rect(self, &state.ui.as_ref()?.browser_import.0)?;
        Some((index, rect))
    }

    pub fn browser_save_notice(&mut self, message: String) {
        let mut state = std::mem::take(&mut self.eng.save_slots);
        state.status = message;
        if state.home_active {
            if let Some(home) = &state.home { label(&home.details, state.status.clone()); }
        } else {
            draw_state(self, &state);
        }
        self.eng.save_slots = state;
    }

    pub fn browser_autosave_label(&mut self, message: &str) {
        if self.at_desktop_start() { return; }
        let Some(browser) = &self.eng.save_slots.browser else { return };
        let caption = &browser.timer_label;
        if let Some(old) = with_comp(caption, |c| match &c.k {
            CK::Text(text) => Some(text.text.clone()),
            _ => None,
        }).flatten() {
            let prefix = old.lines().take(2).collect::<Vec<_>>().join("\n");
            let next = format!("{prefix}\n{message}");
            if old != next { label(caption, next); }
        }
    }

    /// Install a browser upload into an explicitly chosen empty numbered slot.
    pub fn import_browser_save(&mut self, name: &str, slot: usize) -> Result<(), String> {
        if !(1..=SLOT_COUNT).contains(&slot) || self.eng.save_dir.is_empty() {
            return Err("Choose an empty numbered slot.".into());
        }
        // The browser host stages the file in this profile with a generated basename.
        if !name.ends_with(".txt") || !name.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)) {
            return Err("Invalid upload name.".into());
        }
        let dir = PathBuf::from(&self.eng.save_dir);
        let (data, _) = read_save(&dir.join("import").join(name))?;
        write_save_file(&slot_path(&dir, slot), data.as_bytes(), false)
            .map_err(|error| if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!("Slot {slot} is occupied. Choose an empty slot.")
            } else { format!("Could not import the save: {error}") })?;
        if matches!(self.eng.save_slots.mode, Some(Mode::Create | Mode::Save | Mode::Load)) {
            let mut state = std::mem::take(&mut self.eng.save_slots);
            state.entries = load_entries(&dir);
            draw_state(self, &state);
            self.eng.save_slots = state;
        }
        Ok(())
    }
}
