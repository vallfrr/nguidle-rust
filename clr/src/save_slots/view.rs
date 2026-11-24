use super::*;

pub(super) fn build(vm: &mut Vm) -> Screen {
    let settings = named_go(vm, "Settings Menu").unwrap_or_default();
    let image_style = |go: &Value| {
        style(go, |k| {
            if let CK::Image(i) = k {
                Some(i.as_ref().clone())
            } else {
                None
            }
        })
    };
    let text_style = |go: &Value| {
        style(go, |k| {
            if let CK::Text(t) = k {
                Some(t.as_ref().clone())
            } else {
                None
            }
        })
    };
    let settings_title = child(&settings, "Title");
    let settings_panel = child(&settings, "Settings Panel 2");
    let settings_back = child(&settings, "Back to Info n Stuff");
    let background = image_style(&settings);
    let section_frame = image_style(&settings_panel);
    let button_frame = image_style(&settings_back);
    let title_style = text_style(&settings_title);
    let subtitle_style = text_style(&child(&settings_title, "subtitle"));
    let section_style = text_style(&child(&settings_panel, "Text"));
    let body_style = text_style(&child(&settings_back, "Text"));
    let button_tint = style(&settings_back, |k| {
        if let CK::Sel(s) = k {
            Some((s.colors, s.transition))
        } else {
            None
        }
    });
    let root = new_node(
        vm,
        "Save Management Canvas",
        &Value::Null,
        [0.0, 1.0],
        0.0,
        600.0,
    );
    attach(
        vm,
        &root,
        "UnityEngine.Canvas",
        CK::Canvas(Box::new(CanvasD {
            mode: 0,
            sorting: 10000,
            override_sorting: true,
            scale_factor: 1.0,
        })),
    );
    vm.eng.canvases.push((10000, tr(&root)));
    vm.eng.canvases.sort_by_key(|(sorting, _)| *sorting);
    let panel = new_node(vm, "Save Management", &tr(&root), [0.0, 0.0], 0.0, 600.0);
    with_tr(&tr(&panel), |t| {
        t.apos[0] = CONTENT_LEFT;
        t.size[0] = vm.eng.desktop_layout.width() - CONTENT_LEFT;
    });
    let panel_art = image(vm, &panel, background.as_ref(), true);
    let title_sidebar = new_node(vm, "Load Game Sidebar", &tr(&root), [0.0, 0.0], 0.0, 600.0);
    with_tr(&tr(&title_sidebar), |t| t.size[0] = CONTENT_LEFT);
    image(vm, &title_sidebar, background.as_ref(), true);
    let main_menu = named_go(vm, "Main Menu Screen").unwrap_or_default();
    let chicken = image_style(&child(&main_menu, "Chicken"));
    if let Some(mut chicken) = chicken {
        chicken.preserve_aspect = true;
        let art = new_node(vm, "Load Game Illustration", &tr(&title_sidebar), [0.15, 0.85], 178.0, 244.0);
        image(vm, &art, Some(&chicken), false);
    }
    let locked_sidebar = new_node(vm, "Locked Sidebar", &tr(&root), [0.0, 0.0], 0.0, 600.0);
    with_tr(&tr(&locked_sidebar), |t| t.size[0] = CONTENT_LEFT);
    let shade = image(vm, &locked_sidebar, None, false);
    with_comp(&shade, |comp| {
        if let CK::Image(image) = &mut comp.k {
            image.color = [0.17, 0.22, 0.28, 0.28];
        }
    });
    let parent = tr(&panel);
    let banner = new_node(vm, "Title", &parent, [0.19, 0.81], 14.0, 58.0);
    let title = text(vm, &banner, title_style.as_ref(), Some(40), 4);
    let intro = new_node(vm, "Subtitle", &parent, [0.06, 0.94], 72.0, 26.0);
    let detail = text(
        vm,
        &intro,
        subtitle_style.as_ref().or(body_style.as_ref()),
        Some(14),
        4,
    );

    let left = new_node(
        vm,
        "Save Slots Panel",
        &parent,
        [0.035, 0.624],
        110.0,
        418.0,
    );
    image(vm, &left, section_frame.as_ref(), false);
    let mut themed = vec![
        (panel.clone(), settings.clone()),
        (title_sidebar.clone(), settings),
        (left.clone(), settings_panel.clone()),
    ];
    let left = tr(&left);
    let right = new_node(
        vm,
        "Selected Save Panel",
        &parent,
        [0.64, 0.965],
        110.0,
        418.0,
    );
    image(vm, &right, section_frame.as_ref(), false);
    themed.push((right.clone(), settings_panel));
    let right = tr(&right);
    let headings = [(&left, "Save Slots"), (&right, "Selected Save")].map(|(p, name)| {
        let heading = new_node(vm, name, p, [0.05, 0.95], 17.0, 34.0);
        text(vm, &heading, section_style.as_ref(), Some(23), 4)
    });
    let summary = new_node(
        vm,
        "Selected Save Details",
        &right,
        [0.08, 0.92],
        61.0,
        133.0,
    );
    let selection = text(vm, &summary, body_style.as_ref(), Some(16), 4);
    let name_panel = new_node(vm, "Character Name", &right, [0.08, 0.92], 155.0, 88.0);
    let name_parent = tr(&name_panel);
    let name_label = new_node(vm, "Name Label", &name_parent, [0.0, 1.0], 0.0, 29.0);
    label(&text(vm, &name_label, body_style.as_ref(), Some(16), 3), "Character Name".into());
    let name_field = new_node(vm, "Character Name Input", &name_parent, [0.0, 1.0], 34.0, 37.0);
    let graphic = image(vm, &name_field, button_frame.as_ref(), true);
    themed.push((name_field.clone(), settings_back.clone()));
    let name_text = new_node(vm, "Text", &tr(&name_field), [0.06, 0.94], 0.0, 37.0);
    let name_placeholder = new_node(vm, "Placeholder", &tr(&name_field), [0.06, 0.94], 0.0, 37.0);
    let placeholder = text(vm, &name_placeholder, body_style.as_ref(), Some(16), 3);
    with_comp(&placeholder, |c| {
        if let CK::Text(text) = &mut c.k { text.color = [0.34, 0.34, 0.34, 1.0]; }
    });
    label(&placeholder, DEFAULT_NAME.into());
    let mut field = default_sel(SelKind::InputField);
    field.target_graphic = graphic;
    field.text_component = text(vm, &name_text, body_style.as_ref(), Some(16), 3);
    field.placeholder = placeholder;
    field.char_limit = NAME_LIMIT as i32;
    let name_input = attach(vm, &name_field, "UnityEngine.UI.InputField", CK::Sel(Box::new(field)));
    let mut actions = Vec::new();
    let mut button = |vm: &mut Vm, p: &Value, action, x, y, h, size| {
        let go = new_node(vm, "Save Screen Button", p, x, y, h);
        let graphic = image(vm, &go, button_frame.as_ref(), true);
        let mut sel = default_sel(SelKind::Button);
        if let Some((colors, transition)) = button_tint {
            sel.colors = colors;
            sel.transition = transition;
        }
        sel.target_graphic = graphic;
        attach(vm, &go, "UnityEngine.UI.Button", CK::Sel(Box::new(sel)));
        themed.push((go.clone(), settings_back.clone()));
        let caption = new_node(vm, "Text", &tr(&go), [0.025, 0.975], 0.0, h);
        let label = text(vm, &caption, body_style.as_ref(), Some(size), 4);
        actions.push((go.clone(), action));
        (go, label)
    };
    let mut rows = Vec::with_capacity(ROW_COUNT);
    let mut row_info = Vec::with_capacity(ROW_COUNT);
    let mut delete_buttons = Vec::with_capacity(SLOT_COUNT);
    let trash = vm.eng.keys.get("sharedassets0.assets:3370").cloned().unwrap_or_default();
    let checkmark = vm.eng.keys.get("sharedassets0.assets:2493").cloned().unwrap_or_default();
    for i in 0..ROW_COUNT {
        let y = 65.0 + i as f32 * 49.0;
        rows.push(button(
            vm,
            &left,
            Action::Row(i),
            [0.066, 0.32],
            y,
            37.0,
            16,
        ));
        let info = new_node(vm, "Slot Details", &left, [0.36, 0.83], y, 37.0);
        row_info.push(text(vm, &info, body_style.as_ref(), Some(16), 3));
        if i > 0 {
            let trash_button = button(vm, &left, Action::Delete(i), [0.86, 0.95], y, 37.0, 12);
            button_icon(vm, &trash_button, &trash);
            delete_buttons.push(trash_button);
        }
    }
    let delete_row = new_node(vm, "Delete Slot Confirmation", &left, [0.066, 0.95], 65.0, 37.0);
    let delete_parent = tr(&delete_row);
    let delete_message = new_node(vm, "Delete Prompt", &delete_parent, [0.0, 0.75], 0.0, 37.0);
    let delete_prompt = text(vm, &delete_message, body_style.as_ref(), Some(13), 3);
    let delete_cancel = button(vm, &delete_parent, Action::CancelDelete, [0.76, 0.87], 0.0, 37.0, 20);
    let delete_confirm = button(vm, &delete_parent, Action::ConfirmDelete, [0.89, 1.0], 0.0, 37.0, 18);
    button_icon(vm, &delete_confirm, &checkmark);
    let message = new_node(
        vm,
        "Save Screen Status",
        &parent,
        [0.035, 0.965],
        540.0,
        39.0,
    );
    let status = text(vm, &message, body_style.as_ref(), Some(16), 4);
    let primary = button(vm, &right, Action::Primary, [0.08, 0.92], 213.0, 41.0, 16);
    let secondary = button(vm, &right, Action::Secondary, [0.08, 0.92], 266.0, 41.0, 16);
    let browser_import = button(vm, &right, Action::BrowserImport, [0.08, 0.92], 319.0, 41.0, 16);
    let main_menu = button(vm, &parent, Action::MainMenu, [0.035, 0.168], 18.0, 37.0, 16);
    let back = button(vm, &parent, Action::Back, [0.035, 0.168], 18.0, 37.0, 16);
    let previous = button(vm, &left, Action::Previous, [0.066, 0.35], 366.0, 34.0, 15);
    let next = button(vm, &left, Action::Next, [0.65, 0.934], 366.0, 34.0, 15);
    let grid = new_node(vm, "Achievement Grid", &left, [0.0, 1.0], 53.0, 306.0);
    let tiles = (0..ACHIEVEMENTS_PER_PAGE).map(|i| {
        let x = 0.09 + 0.82 * (i % ACHIEVEMENT_COLUMNS) as f32 / (ACHIEVEMENT_COLUMNS - 1) as f32;
        let y = 18.0 + 67.0 * (i / ACHIEVEMENT_COLUMNS) as f32;
        let tile = button(vm, &tr(&grid), Action::Achievement(i), [x, x], y, 43.0, 9);
        with_tr(&tr(&tile.0), |t| {
            t.apos[0] = -21.5;
            t.size[0] = 43.0;
        });
        for comp in with_go(&tile.0, |g| g.comps.clone()).unwrap_or_default() {
            with_comp(&comp, |c| {
                if let CK::Sel(s) = &mut c.k { s.transition = 0; }
            });
        }
        let icon = icon_in_frame(vm, &tile.0, 43.0);
        (tile, icon)
    }).collect();
    let details = new_node(vm, "Achievement Details", &right, [0.0, 1.0], 56.0, 340.0);
    let details_parent = tr(&details);
    let icon_frame = new_node(vm, "Selected Achievement Frame", &details_parent, [0.5, 0.5], 14.0, 102.0);
    with_tr(&tr(&icon_frame), |t| {
        t.apos[0] = -51.0;
        t.size[0] = 102.0;
    });
    image(vm, &icon_frame, button_frame.as_ref(), false);
    themed.push((icon_frame.clone(), settings_back));
    let icon = icon_in_frame(vm, &icon_frame, 102.0);
    let title_node = new_node(vm, "Selected Achievement Name", &details_parent, [0.09, 0.91], 125.0, 57.0);
    let name = text(vm, &title_node, section_style.as_ref(), Some(20), 4);
    let description_node = new_node(vm, "Condition", &details_parent, [0.09, 0.91], 190.0, 98.0);
    let description = text(vm, &description_node, body_style.as_ref(), Some(16), 4);
    let state_node = new_node(vm, "Unlock State", &details_parent, [0.09, 0.91], 301.0, 30.0);
    let state = text(vm, &state_node, subtitle_style.as_ref(), Some(15), 4);
    let canvas = named_go(vm, "Canvas").unwrap_or_default();
    let frame_junctions = [["Middle Bolt", "Middle Bolt (3)"], ["Middle Bolt (1)", "Middle Bolt (2)"]]
        .map(|names| names.map(|name| tr(&child(&canvas, name))));
    Screen {
        root,
        panel,
        panel_art,
        sections: [left, right],
        title_sidebar,
        locked_sidebar,
        title,
        detail,
        status,
        headings,
        rows,
        row_info,
        delete_buttons,
        delete_row,
        delete_prompt,
        delete_cancel,
        delete_confirm,
        selection,
        name_panel,
        name_input,
        primary,
        secondary,
        browser_import,
        back,
        previous,
        next,
        main_menu,
        achievements: AchievementGrid {
            root: grid, tiles, details, icon_frame, icon, name, description, state,
            sprites: RefCell::default(),
        },
        frame_junctions,
        actions,
        themed,
    }
}

pub(super) fn refresh_theme(ui: &Screen) {
    for (target, source) in &ui.themed {
        copy_theme_sprite(target, source);
    }
}

pub(super) fn copy_theme_sprite(target: &Value, source: &Value) {
    let Some(sprite) = style(source, |k| match k {
        CK::Image(image) => Some(image.sprite.clone()),
        _ => None,
    }) else { return };
    let changed = with_go(target, |g| {
        let mut changed = false;
        for comp in &g.comps {
            with_comp(comp, |c| {
                if let CK::Image(image) = &mut c.k {
                    if !same(&image.sprite, &sprite) {
                        image.sprite = sprite.clone();
                        changed = true;
                    }
                }
            });
        }
        changed
    });
    if changed == Some(true) {
        mark_dirty(target);
    }
}

pub(crate) fn label(comp: &Value, value: String) {
    with_comp(comp, |c| {
        if let CK::Text(t) = &mut c.k {
            t.text = value;
            t.layout = None;
        }
    });
    mark_dirty(comp);
}

pub(crate) fn prepare_title(vm: &mut Vm) {
    if !vm.eng.desktop_panels || vm.eng.save_dir.is_empty() {
        return;
    }
    let Some(title) = named_go(vm, "Main Menu Screen") else {
        return;
    };
    let button = child(&title, "Load File Button");
    let caption = child(&button, "Button Text");
    for comp in with_go(&caption, |g| g.comps.clone()).unwrap_or_default() {
        if with_comp(&comp, |c| matches!(c.k, CK::Text(_))) == Some(true) {
            label(&comp, "Load Game".into());
        }
    }
    if let Some(panel) = named_go(vm, "Player Panel") {
        for name in ["Online Save", "Online Load"] {
            let original = child(&panel, name);
            with_go(&original, |g| g.port_hidden = true);
            mark_dirty(&original);
        }
    }
    if vm.eng.save_slots.steam_tab.is_some() { return; }
    let Some(menu) = named_go(vm, "Achievements Menu") else { return };
    let page_two = child(&menu, "Page 2");
    let bp_grid = child(&menu, "Slots");
    let Some((x, y, width, height, pivot)) = with_tr(&tr(&page_two), |t| {
        (t.apos[0], t.apos[1], t.size[0], t.size[1], t.pivot)
    }) else { return };
    if bp_grid.is_null() { return; }
    let background = style(&page_two, |k| match k {
        CK::Image(i) => Some(i.as_ref().clone()),
        _ => None,
    });
    let typeface = style(&child(&page_two, "Text"), |k| match k {
        CK::Text(t) => Some(t.as_ref().clone()),
        _ => None,
    });
    let button = new_node(vm, "Steam Achievements Tab", &tr(&menu), [0.5, 0.5], 0.0, height);
    with_tr(&tr(&button), |t| {
        t.amin[1] = 0.5;
        t.amax[1] = 0.5;
        t.apos = [x + (1.0 - pivot[0]) * width + 3.0, y + (1.0 - pivot[1]) * height];
        t.size[0] = 174.0;
    });
    let graphic = image(vm, &button, background.as_ref(), true);
    let mut selectable = default_sel(SelKind::Button);
    selectable.target_graphic = graphic;
    attach(vm, &button, "UnityEngine.UI.Button", CK::Sel(Box::new(selectable)));
    let title = new_node(vm, "Text", &tr(&button), [0.025, 0.975], 0.0, height);
    let title = text(vm, &title, typeface.as_ref(), Some(13), 4);
    label(&title, "Steam Achievements".into());
    vm.eng.save_slots.steam_tab = Some(SteamTab { button: (button, title), page_two, menu, bp_grid });
}

pub(crate) fn refresh_achievements(vm: &mut Vm) {
    if vm.eng.save_slots.mode == Some(Mode::Achievements) {
        let state = std::mem::take(&mut vm.eng.save_slots);
        draw_state(vm, &state);
        vm.eng.save_slots = state;
    }
}

fn achievement_sprite(vm: &mut Vm, grid: &AchievementGrid, id: i32, selected: Option<bool>) -> Value {
    let mut sprites = grid.sprites.borrow_mut();
    if let Some(sprite) = sprites.get(&(id, selected)) {
        return sprite.clone();
    }
    let ty = vm.type_id("SteamManager/Achievement");
    let sprite = crate::bcl_core::enum_name(vm, ty, id)
        .map(|name| format!("steam-achievements/{name}"))
        .filter(|key| crate::assets::read(&format!("tex/{key}.png")).is_some())
        .map(|mut key| {
            if let Some(earned) = selected {
                key.push_str(if earned { "#selected" } else { "#selected-locked" });
            }
            let texture = new_comp(vm, "UnityEngine.Texture2D", Value::Null, CK::Texture(Box::new(TexD {
                name: key.clone(), key: key.clone(), w: 64, h: 64,
            })));
            new_comp(vm, "UnityEngine.Sprite", Value::Null, CK::Sprite(Box::new(SpriteD {
                name: key, tex: texture, rect: [0.0, 0.0, 64.0, 64.0],
                border: [0.0; 4], pivot: [0.5, 0.5], ppu: 100.0,
            })))
        }).unwrap_or_default();
    sprites.insert((id, selected), sprite.clone());
    sprite
}

fn paint_achievement(comp: &Value, sprite: Value, earned: bool) {
    let brightness = if earned { 1.0 } else { 0.45 };
    let alpha = if sprite.is_null() { 0.0 } else { 1.0 };
    with_comp(comp, |c| {
        if let CK::Image(image) = &mut c.k {
            image.sprite = sprite;
            image.color = [brightness, brightness, brightness, alpha];
        }
    });
    mark_dirty(comp);
}

fn draw_achievements(vm: &mut Vm, state: &SaveSlots, ui: &Screen) {
    let grid = &ui.achievements;
    let start = state.page * ACHIEVEMENTS_PER_PAGE;
    let count = vm.eng.achievements.entries.len();
    let earned = vm.eng.achievements.entries.iter().filter(|e| e.earned).count();
    let pages = count.div_ceil(ACHIEVEMENTS_PER_PAGE).max(1);
    label(&ui.headings[0], format!("{earned} of {count} unlocked"));
    label(&ui.status, if vm.eng.achievements.write_failed {
        "Could not save achievements. The game will try again.".into()
    } else { format!("Page {} of {pages}", state.page + 1) });
    for (row, info) in ui.rows.iter().zip(&ui.row_info) {
        vm.set_active(&row.0, false);
        vm.set_active(&go_of(info), false);
    }
    for (button, _) in &ui.delete_buttons { vm.set_active(button, false); }
    vm.set_active(&ui.delete_row, false);
    for (i, (tile, icon)) in grid.tiles.iter().enumerate() {
        let entry = vm.eng.achievements.entries.get(start + i).map(|e| (e.id, e.name.clone(), e.earned));
        vm.set_active(&tile.0, entry.is_some());
        if let Some((id, name, earned)) = entry {
            let selected = state.selected == start + i;
            let sprite = achievement_sprite(vm, grid, id, selected.then_some(earned));
            let has_icon = !sprite.is_null();
            caption(tile, &name, !has_icon && selected);
            // Retain an input label for focus and scripted navigation.
            with_comp(&tile.1, |comp| {
                if let CK::Text(text) = &mut comp.k {
                    text.color[3] = if has_icon { 0.0 } else { 1.0 };
                }
            });
            paint_achievement(icon, sprite, earned || selected);
        }
    }
    let selected = vm.eng.achievements.entries.get(state.selected)
        .map(|e| (e.id, e.name.clone(), e.description.clone(), e.earned));
    if let Some((id, name, description, earned)) = selected {
        let icon = achievement_sprite(vm, grid, id, None);
        vm.set_active(&grid.icon_frame, !icon.is_null());
        paint_achievement(&grid.icon, icon, earned);
        label(&grid.name, name);
        label(&grid.description, description);
        label(&grid.state, if earned { "Unlocked" } else { "Locked" }.into());
    } else {
        vm.set_active(&grid.icon_frame, false);
        label(&grid.name, "Choose an achievement".into());
        label(&grid.description, String::new());
        label(&grid.state, String::new());
    }
    for button in [&ui.primary, &ui.secondary, &ui.browser_import, &ui.main_menu] {
        vm.set_active(&button.0, false);
    }
    vm.set_active(&ui.previous.0, state.page > 0);
    vm.set_active(&ui.next.0, state.page + 1 < pages);
    caption(&ui.previous, "Previous", false);
    caption(&ui.next, "Next", false);
}

pub(super) fn caption(button: &(Value, Value), value: &str, selected: bool) {
    label(&button.1, value.into());
    for c in with_go(&button.0, |g| g.comps.clone()).unwrap_or_default() {
        with_comp(&c, |c| {
            if let CK::Image(image) = &mut c.k {
                image.color = if selected {
                    SELECTION_COLOR
                } else {
                    [1.0; 4]
                };
            }
        });
    }
    mark_dirty(&button.0);
}

pub(super) fn draw_state(vm: &mut Vm, state: &SaveSlots) {
    let (Some(ui), Some(mode)) = (&state.ui, state.mode) else {
        return;
    };
    let choosing = state.from_title && state.home.is_some();
    let creating = mode == Mode::Create;
    let achievements = mode == Mode::Achievements;
    vm.set_active(&ui.locked_sidebar, choosing);
    let importing = state.import_files.is_some();
    let cancelling = state.pending.is_some() || importing;
    vm.set_active(&ui.back.0, !achievements && (state.from_title || cancelling));
    vm.set_active(&ui.name_panel, creating && state.selected < ROW_COUNT);
    vm.set_active(&ui.achievements.root, achievements);
    vm.set_active(&ui.achievements.details, achievements);
    vm.set_active(&go_of(&ui.selection), !achievements);
    if achievements {
        vm.set_active(&ui.title_sidebar, false);
        draw_achievements(vm, state, ui);
        return;
    }
    vm.set_active(&ui.title_sidebar, state.from_title && !choosing);
    label(
        &ui.title,
        if importing {
            "IMPORT A SAVE"
        } else if creating {
            "CREATE NEW GAME"
        } else if mode == Mode::Save {
            "SAVE GAME"
        } else {
            "LOAD GAME"
        }
        .into(),
    );
    label(
        &ui.detail,
        if importing {
            "Place .txt saves in save/import, then choose a file below."
        } else if creating {
            "Select an empty slot for your new game."
        } else if mode == Mode::Save {
            "Choose a slot for a snapshot of your current game."
        } else if state.from_title {
            "Choose a saved game to continue."
        } else if state.browser.is_some() {
            "Choose a saved game to continue. Save any current progress first."
        } else {
            "Choose a saved game to continue. Your current game is backed up before loading."
        }
        .into(),
    );
    label(&ui.status, state.status.clone());
    let row_y = |i: usize| 65.0 + i as f32 * 49.0 - if choosing { 24.5 } else { 0.0 };
    vm.set_active(&ui.delete_row, state.deleting.is_some());
    if let Some(delete) = &state.deleting {
        with_tr(&tr(&ui.delete_row), |t| t.apos[1] = -row_y(delete.index));
        mark_dirty(&ui.delete_row);
        label(&ui.delete_prompt, if delete.final_confirmation {
            format!("Delete Slot {} for good? The trash can has no undo.", delete.index)
        } else {
            format!("Delete the save in Slot {}?", delete.index)
        });
        let (left, right) = if delete.final_confirmation {
            (&ui.delete_confirm, &ui.delete_cancel)
        } else {
            (&ui.delete_cancel, &ui.delete_confirm)
        };
        for (button, anchors) in [(left, [0.76, 0.87]), (right, [0.89, 1.0])] {
            with_tr(&tr(&button.0), |t| { t.amin[0] = anchors[0]; t.amax[0] = anchors[1]; });
            mark_dirty(&button.0);
        }
        caption(&ui.delete_cancel, "×", false);
        caption(&ui.delete_confirm, "✓", false);
    }
    for (i, row) in ui.rows.iter().enumerate() {
        let confirming = state.deleting.as_ref().is_some_and(|delete| delete.index == i);
        let visible = (!choosing || i > 0) && !confirming;
        let browser_load = state.browser.is_some() && mode == Mode::Load;
        let available = (!choosing || browser_load || slot_available(mode, state.entries.get(i))) &&
            (importing || !state.blocked(i));
        set_interactable(&row.0, available);
        vm.set_active(&go_of(&ui.row_info[i]), visible);
        if let Some(button) = i.checked_sub(1).and_then(|i| ui.delete_buttons.get(i)) {
            let occupied = matches!(state.entries.get(i), Some(Entry::Save(..) | Entry::Invalid));
            vm.set_active(&button.0, creating && occupied && !confirming && !state.blocked(i));
            with_tr(&tr(&button.0), |t| t.apos[1] = -row_y(i));
            caption(button, &format!("Delete Slot {i}"), false);
        }
        if let Some(files) = &state.import_files {
            let file = files.get(state.page * ROW_COUNT + i);
            vm.set_active(&row.0, file.is_some());
            caption(
                row,
                &format!("File {}", state.page * ROW_COUNT + i + 1),
                false,
            );
            label(
                &ui.row_info[i],
                file.map_or(String::new(), |f| {
                    clean_label(&f.file_name().unwrap_or_default().to_string_lossy(), 40)
                }),
            );
            continue;
        }
        for node in [&row.0, &go_of(&ui.row_info[i])] {
            with_tr(&tr(node), |t| {
                t.apos[1] = -row_y(i);
            });
            mark_dirty(node);
        }
        vm.set_active(&row.0, visible);
        let name = if i == 0 {
            "Autosave".into()
        } else {
            format!("Slot {i}")
        };
        let contents = if state.browser.as_ref().is_some_and(|browser| browser.locked & (1 << i) != 0) {
            "In use on another device".into()
        } else { match state.entries.get(i).unwrap_or(&Entry::Empty) {
            Entry::Empty => "Empty".into(),
            Entry::Invalid => "Unreadable save".into(),
            Entry::Save(info, _) => {
                let time = info
                    .seconds
                    .map(play_time)
                    .unwrap_or_else(|| "Unknown play time".into());
                if info.player.is_empty() {
                    time
                } else {
                    format!("{}   |   {time}", clean_label(&info.player, 20))
                }
            }
        }};
        caption(row, &name, state.selected == i);
        label(&ui.row_info[i], contents);
    }
    let slot_name = if state.selected == 0 {
        "Autosave".into()
    } else {
        format!("Slot {}", state.selected)
    };
    let selection = match state.entries.get(state.selected) {
        Some(Entry::Save(info, modified)) => {
            let player = if info.player.is_empty() {
                "Unknown character"
            } else {
                &info.player
            };
            let played = info
                .seconds
                .map(play_time)
                .unwrap_or_else(|| "Unknown".into());
            let saved = modified.map(date).unwrap_or_else(|| "Unknown".into());
            format!("{slot_name}\n\n{player}\nPlayed: {played}\n\nLast saved\n{saved}")
        }
        Some(Entry::Invalid) => format!("{slot_name}\n\nUnreadable save"),
        None if choosing => "No slot selected".into(),
        None => "Choose a save".into(),
        _ if creating => format!("{slot_name} — Empty"),
        _ => format!("{slot_name}\n\nEmpty"),
    };
    label(&ui.selection, selection);
    vm.set_active(&ui.primary.0, (!importing || state.pending.is_some()) && state.deleting.is_none());
    with_tr(&tr(&ui.primary.0), |t| t.apos[1] = if creating { -251.0 } else { -213.0 });
    let waiting = state.browser.as_ref().is_some_and(|browser| browser.requested.is_some());
    let server_action = !matches!(state.pending, Some(Pending::MainMenu));
    set_interactable(&ui.primary.0, !waiting && (!server_action || !state.blocked(state.selected)) &&
        (!choosing || slot_available(mode, state.entries.get(state.selected))));
    let web = state.browser.is_some();
    vm.set_active(&ui.secondary.0, !importing && if web { !creating && state.pending.is_none() } else { !choosing });
    set_interactable(&ui.secondary.0, if web && mode == Mode::Save { state.active_slot.is_some() }
        else if web { !state.blocked(state.selected) && matches!(state.entries.get(state.selected), Some(Entry::Save(..))) }
        else { !state.blocked(state.selected) });
    vm.set_active(&ui.browser_import.0, web && mode == Mode::Load && state.pending.is_none());
    set_interactable(&ui.browser_import.0, state.browser_import_target().is_some());
    caption(&ui.browser_import, "Import Save File", false);
    vm.set_active(&ui.main_menu.0, !state.from_title && !cancelling);
    caption(&ui.main_menu, "Main Menu", false);
    let pages = state
        .import_files
        .as_ref()
        .map_or(0, |files| files.len().div_ceil(ROW_COUNT));
    vm.set_active(
        &ui.previous.0,
        importing && state.pending.is_none() && state.page > 0,
    );
    vm.set_active(
        &ui.next.0,
        importing && state.pending.is_none() && state.page + 1 < pages,
    );
    caption(&ui.previous, "Previous", false);
    caption(&ui.next, "Next", false);
    caption(
        &ui.primary,
        if state.pending.is_some() {
            "Confirm"
        } else if creating {
            "Create New Game"
        } else if mode == Mode::Save {
            "Save Current Game"
        } else {
            "Load Selected Game"
        },
        false,
    );
    caption(
        &ui.secondary,
        if web && mode == Mode::Save {
            "Export Current Game"
        } else if mode == Mode::Save {
            "Import Save"
        } else {
            "Export Save"
        },
        false,
    );
    caption(
        &ui.back,
        if cancelling {
            "Cancel"
        } else {
            "Main Menu"
        },
        false,
    );
}
