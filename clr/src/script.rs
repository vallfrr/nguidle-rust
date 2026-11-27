//! Scripted clicks, drags and waits for game verification.
use crate::Vm;

/// Steps: `x,y[,wait]` click (virtual 960x600 px, top-left origin; wait in frames, default 15),
/// `w,N` waits N frames, `wr,N` runs N frames paced at 30 Hz on desktop,
/// `h,x,y` holds a press, `d,x1,y1,x2,y2` drags from point 1 to point 2.
pub fn run_steps(vm: &mut Vm, steps: &[String]) {
    for step in steps {
        let p: Vec<&str> = step.split(',').collect();
        if p[0] == "f" {
            // focus navigation: f,u|d|l|r; "f,list" lists the elements
            if p[1] == "list" {
                let l = crate::focus::collect(vm);
                println!("[focus] {} visible interactive elements", l.len());
                for f in l.iter().take(60) {
                    println!("[focus]   {:<8} ({:>3.0},{:>3.0})-({:>3.0},{:>3.0}) {}", f.kind, f.rect[0], f.rect[1], f.rect[2], f.rect[3], f.label.chars().take(40).collect::<String>());
                }
            } else if vm.dropdown_navigate(p[1].chars().next().unwrap_or('d')) {
                println!("[focus] {} -> dropdown", p[1]);
            } else if let Some(f) = crate::focus::navigate(vm, p[1].chars().next().unwrap_or('d')) {
                println!("[focus] {} -> {:<8} ({:>3.0},{:>3.0})-({:>3.0},{:>3.0}) {}", p[1], f.kind, f.rect[0], f.rect[1], f.rect[2], f.rect[3], f.label.chars().take(40).collect::<String>());
            } else {
                println!("[focus] {} -> no element", p[1]);
            }
            vm.frame(1.0 / 30.0);
            continue;
        }
        if p[0] == "consolemode" {
            // consolemode,on|off: the Settings switch (desktop UI) and the console presentation
            vm.eng.desktop_panels = true;
            crate::console_mode::prepare(vm);
            crate::console_mode::set_enabled(vm, p.get(1) == Some(&"on"));
            crate::console_mode::sync(vm);
            println!("[console] {}", crate::console_mode::enabled(vm));
            continue;
        }
        if p[0] == "consolescale" {
            // consolescale,100|110|125: the console mode's scale setting
            crate::console_mode::set_scale(vm, p[1].parse().unwrap_or(100));
            println!("[console] scale {}", crate::console_mode::scale(vm));
            continue;
        }
        if p[0] == "sel" {
            // sel,<label>: puts the controller selection on the control with that label (no click)
            match crate::dev::find_click(vm, &p[1..].join(",")) {
                Some((x, y, label)) => {
                    let ok = crate::focus::select_at(vm, x, vm.eng.screen.1 as f32 - y);
                    println!("[sel] {label} {}", if ok { "selected" } else { "not reachable" });
                }
                None => println!("[sel] no control '{}'", p[1..].join(",")),
            }
            continue;
        }
        if p[0] == "inputs" {
            // inputs: every text / number field of the scene with its content type (0 standard, 2 integer, 3 decimal ...)
            let gos = vm.eng.gos.clone();
            for go in gos {
                let comps = crate::unity::with_go(&go, |g| g.comps.clone()).unwrap_or_default();
                for c in comps {
                    let info = crate::unity::with_comp(&c, |k| match &k.k {
                        crate::unity::CK::Sel(s) if s.kind == crate::unity::SelKind::InputField => Some((s.content_type, s.char_limit, s.text.clone())),
                        _ => None,
                    }).flatten();
                    if let Some((ct, limit, txt)) = info {
                        let name = crate::unity::with_go(&go, |g| g.name.clone()).unwrap_or_default();
                        let mut parent = crate::unity::with_go(&go, |g| g.transform.clone()).and_then(|t| crate::unity::with_tr(&t, |x| x.parent.clone())).unwrap_or_default();
                        let mut path = String::new();
                        for _ in 0..3 {
                            if parent.is_null() { break; }
                            path = format!("{}/{}", crate::unity::with_go(&crate::unity::go_of(&parent), |g| g.name.clone()).unwrap_or_default(), path);
                            parent = crate::unity::with_tr(&parent, |x| x.parent.clone()).unwrap_or_default();
                        }
                        println!("[input] ct={ct} limit={limit} {path}{name} = {txt:?}");
                    }
                }
            }
            continue;
        }
        if p[0] == "consolestate" {
            // regions that scroll right now (after a capture or a frame laid the screen out)
            for r in &vm.eng.desktop_layout.regions {
                println!("[console] region {} view ({:.0},{:.0})-({:.0},{:.0}) extra {:.0} native {}", r.key, r.view[0], r.view[1], r.view[2], r.view[3], r.extra, !r.native.is_null());
            }
            println!("[console] rows {:.0} pans {:?}", vm.eng.desktop_layout.console_view_h, vm.eng.desktop_layout.pans.values().collect::<Vec<_>>());
            continue;
        }
        if p[0] == "pad" {
            // pad,u|d|l|r|a|s|b|prev|next|holdstart|holdend|scroll[,dy]: console controller input
            use crate::console_mode::Pad;
            let pad = match p[1] {
                "u" | "d" | "l" | "r" => Pad::Dir(p[1].chars().next().unwrap()),
                "a" => Pad::Activate,
                "s" => Pad::Secondary,
                "b" => Pad::Back,
                "prev" => Pad::Prev,
                "next" => Pad::Next,
                "holdstart" => Pad::HoldStart,
                "holdend" => Pad::HoldEnd,
                "info" => Pad::Info,
                "start" => Pad::Settings,
                "select" => Pad::InfoStuff,
                "short" => Pad::Shortcut(p.get(2).and_then(|v| v.chars().next()).unwrap_or('x')),
                _ => Pad::Scroll(0.0, p.get(2).and_then(|v| v.parse().ok()).unwrap_or(100.0)),
            };
            let Some(mut c) = vm.eng.console.take() else { println!("[pad] console mode is off"); continue };
            let click = c.input(vm, pad);
            vm.eng.console = Some(c);
            crate::console_mode::sync(vm);
            println!("[pad] {} -> focus {:?} click {:?}", p[1], crate::focus::focused_label(vm), click);
            if let Some(k) = click {
                use crate::console_mode::Phase;
                vm.eng.mouse = (k.x, k.y);
                vm.eng.mouse_button = k.button;
                if k.phase != Phase::Release {
                    vm.eng.mouse_down = true;
                    vm.eng.mouse_down_edge = true;
                    vm.frame(1.0 / 30.0);
                }
                if k.phase != Phase::Press {
                    vm.eng.mouse_down = false;
                    vm.eng.mouse_up_edge = true;
                    vm.frame(1.0 / 30.0);
                    vm.eng.mouse_button = 0;
                }
            }
            for _ in 0..6 {
                crate::console_mode::sync(vm);
                vm.frame(1.0 / 30.0);
            }
            println!("[pad] ... focus {:?}", crate::focus::focused_label(vm));
            continue;
        }
        if p[0] == "focus_at" {
            let found = crate::focus::select_at(vm, p[1].parse().unwrap(), p[2].parse().unwrap());
            println!("[focus] point selected: {found}");
            continue;
        }
        if p[0] == "focus_scroll" {
            crate::focus::scroll(vm, p[1].parse().unwrap(), p[2].parse().unwrap());
            continue;
        }
        if p[0] == "m" {
            // previous / next menu: m,-1 / m,1 (focus on the menu button and click)
            let d: i32 = p.get(1).and_then(|x| x.parse().ok()).unwrap_or(1);
            if let Some((x, y)) = crate::focus::menu_step(vm, d) {
                println!("[menu] {:+} -> bouton ({:.0},{:.0})", d, x, vm.eng.screen.1 as f32 - y);
                vm.eng.mouse = (x, y);
                vm.eng.mouse_down = true;
                vm.eng.mouse_down_edge = true;
                vm.frame(1.0 / 30.0);
                vm.eng.mouse_down = false;
                vm.eng.mouse_up_edge = true;
                vm.frame(1.0 / 30.0);
                for _ in 0..6 {
                    vm.frame(1.0 / 30.0);
                }
            }
            continue;
        }
        if p[0] == "a" {
            if vm.dropdown_navigate('a') {
                continue;
            }
            // activation of the focused element: virtual click at the centre
            if let Some((x, y)) = crate::focus::activation_point(vm) {
                vm.eng.mouse = (x, y);
                vm.eng.mouse_down = true;
                vm.eng.mouse_down_edge = true;
                vm.frame(1.0 / 30.0);
                vm.eng.mouse_down = false;
                vm.eng.mouse_up_edge = true;
                vm.frame(1.0 / 30.0);
                for _ in 0..6 {
                    vm.frame(1.0 / 30.0);
                }
            }
            continue;
        }
        if p[0] == "l" {
            // load: ImportExport.loadBase64ToData(content of file p[1])
            let txt = std::fs::read_to_string(p[1]).unwrap_or_default();
            let comps = vm.eng.user_comps.clone();
            for c in comps {
                if let crate::value::Value::Obj(o) = &c {
                    if vm.types[o.ty as usize].name == "ImportExport" {
                        if let Some(m) = vm.find_method(o.ty, "loadBase64ToData", 2) {
                            let _ = vm.invoke(m, vec![c.clone(), crate::value::Value::str(txt.trim())]);
                        }
                    }
                }
            }
            continue;
        }
        if p[0] == "open" {
            // open,<file>: opens a save the way the game does (loadintoGame: offline progress, version updates)
            let txt = std::fs::read_to_string(p[1]).unwrap_or_default();
            match crate::save_slots::load_into_game(vm, &txt) {
                Ok(()) => println!("[open] {} opened", p[1]),
                Err(e) => println!("[open] {} failed: {e}", p[1]),
            }
            continue;
        }
        if p[0] == "clock" {
            // clock,<seconds since 1970>: the time the game reads (DateTime.UtcNow); clock,real: the system clock again
            if p[1] == "real" {
                std::env::remove_var("NGU_FIXED_NOW");
            } else {
                std::env::set_var("NGU_FIXED_NOW", p[1]);
            }
            vm.forget_wall_clock();
            continue;
        }
        if p[0] == "skip" {
            // skip,<seconds>: the host was away for that long (a hidden tab, a sleeping computer): the next step applies
            // the game's offline progress for it
            vm.lose_time(p[1].parse().unwrap_or(0.0));
            continue;
        }
        if p[0] == "s" {
            // save probe: ImportExport.getBase64Data() written to file p[1]
            let comps = vm.eng.user_comps.clone();
            for c in comps {
                if let crate::value::Value::Obj(o) = &c {
                    if vm.types[o.ty as usize].name == "ImportExport" {
                        if let Some(m) = vm.find_method(o.ty, "getBase64Data", 1) {
                            match vm.invoke(m, vec![c.clone()]) {
                                Ok(v) => {
                                    let txt = crate::bcl_core::to_string(vm, &v).unwrap_or_default();
                                    std::fs::write(p[1], txt).ok();
                                }
                                Err(e) => {
                                    vm.report_exc("save probe", &crate::value::Value::Null, &e);
                                    let msg = String::new();
                                    println!("[save] exception: {msg}");
                                    std::fs::write(p[1], "EXCEPTION").ok();
                                }
                            }
                        }
                    }
                }
            }
            continue;
        }
        if p[0] == "menu" || p[0] == "menus" {
            // dev: `menus` lists MenuSwapper.allMenus; `menu,N` shows menu N (MenuSwapper.swapMenu), whatever the state
            let comps = vm.eng.user_comps.clone();
            for c in comps {
                let crate::value::Value::Obj(o) = &c else { continue };
                if vm.types[o.ty as usize].name != "MenuSwapper" {
                    continue;
                }
                if p[0] == "menus" {
                    let slot = vm.types[o.ty as usize].fields.iter().position(|n| n == "allMenus");
                    let arr = slot.map(|s| o.f.borrow()[s].clone());
                    if let Some(crate::value::Value::Obj(a)) = arr {
                        if let crate::value::Ext::Arr(v) = &*a.x.borrow() {
                            for (i, m) in v.iter().enumerate() {
                                println!("[menus] {i} {}", crate::unity::with_go(m, |g| g.name.clone()).unwrap_or_else(|| vm.describe(m)));
                            }
                        }
                    }
                } else if let Some(m) = vm.find_method(o.ty, "swapMenu", 2) {
                    let n: i32 = p[1].parse().unwrap_or(0);
                    if crate::dev::menu_roots(vm).get(n as usize).is_some_and(|go| {
                        crate::unity::with_go(go, |g| g.port_hidden).unwrap_or(false)
                    }) {
                        break;
                    }
                    if let Err(e) = vm.invoke(m, vec![c.clone(), crate::value::Value::I32(n)]) {
                        println!("[menu] {n} : exception");
                        vm.report_exc("menu", &c, &e);
                    }
                }
                break;
            }
            for _ in 0..10 {
                vm.frame(1.0 / 30.0);
            }
            continue;
        }
        // ---- dev tools (see dev.rs, docs/DEV.md)
        match p[0] {
            "r" | "mc" => {
                // r,x,y[,wait] right click; mc,x,y[,wait] middle click (button numbers as Unity: 1 right, 2 middle)
                let (x, y): (f32, f32) = (p[1].parse().unwrap(), p[2].parse().unwrap());
                let wait: usize = p.get(3).and_then(|s| s.parse().ok()).unwrap_or(15);
                vm.eng.mouse = (x, 600.0 - y);
                vm.eng.mouse_button = if p[0] == "r" { 1 } else { 2 };
                vm.eng.mouse_down = true;
                vm.eng.mouse_down_edge = true;
                vm.frame(1.0 / 30.0);
                vm.eng.mouse_down = false;
                vm.eng.mouse_up_edge = true;
                vm.frame(1.0 / 30.0);
                vm.eng.mouse_button = 0;
                for _ in 0..wait {
                    vm.frame(1.0 / 30.0);
                }
                continue;
            }
            "k" | "kd" | "ku" => {
                // k,<key>[,frames] tap; kd,<key> press and hold; ku,<key> release. <key> = Unity KeyCode number or a name (a, left shift, space...)
                let code = p[1].parse::<i32>().unwrap_or_else(|_| crate::input::key_by_name(p[1]));
                match p[0] {
                    "kd" => {
                        vm.eng.keys_down.insert(code);
                        vm.eng.keys_edge.insert(code);
                    }
                    "ku" => {
                        vm.eng.keys_down.remove(&code);
                    }
                    _ => {
                        let frames: usize = p.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
                        vm.eng.keys_down.insert(code);
                        vm.eng.keys_edge.insert(code);
                        for _ in 0..frames {
                            vm.frame(1.0 / 30.0);
                        }
                        vm.eng.keys_down.remove(&code);
                        vm.frame(1.0 / 30.0);
                    }
                }
                continue;
            }
            "find" => {
                // find,<name part> : GameObjects with their tag, active flag and world rectangle
                print!("{}", crate::dev::find(vm, &p[1..].join(",")));
                continue;
            }
            "tree" => {
                // tree,<depth> : active hierarchy with rectangles (960x600, top-left), canvases marked
                print!("{}", crate::dev::tree(vm, p.get(1).and_then(|s| s.parse().ok()).unwrap_or(2)));
                continue;
            }
            "uimap" => {
                // uimap,<json file>,<markdown file> : UI map of the 58 menus (task R1)
                let (j, m) = crate::dev::uimap(vm);
                std::fs::write(p[1], j).ok();
                std::fs::write(p[2], m).ok();
                continue;
            }
            "save_picker" => {
                vm.start_desktop_save_picker().expect("save picker");
                continue;
            }
            "type" => {
                let request = vm.eng.text_req.take().expect("no text field selected");
                vm.input_commit(&request.comp, step.split_once(',').map_or("", |(_, text)| text));
                continue;
            }
            "visual_motion" => {
                vm.enable_visual_motion();
                continue;
            }
            "pointer" => {
                vm.eng.mouse = (p[1].parse().unwrap(), 600.0 - p[2].parse::<f32>().unwrap());
                continue;
            }
            "release_pointer" => {
                vm.eng.mouse_down = false;
                vm.eng.mouse_up_edge = true;
                vm.frame(1.0 / 30.0);
                continue;
            }
            "sounds" => {
                // sounds requested by the game since the last `sounds` step (AudioSource.Play)
                println!("[sounds] {:?}", std::mem::take(&mut vm.eng.sound_queue));
                continue;
            }
            "ultimate" => {
                // dev: set every unlock-like boolean reachable from the scene (see dev::ultimate)
                println!("[ultimate] {}", crate::dev::ultimate(vm));
                continue;
            }
            "wheel" => {
                // wheel,x,y,notches : mouse wheel at a point (positive = up); console mode's squeezed parts first
                vm.eng.mouse = (p[1].parse().unwrap(), 600.0 - p[2].parse::<f32>().unwrap());
                let notches: f32 = p[3].parse().unwrap();
                let taken = match vm.eng.console.take() {
                    Some(mut c) => {
                        let (mx, my) = vm.eng.mouse;
                        let t = c.wheel(vm, mx, my, notches);
                        vm.eng.console = Some(c);
                        t
                    }
                    None => false,
                };
                if !taken {
                    vm.scroll_wheel(notches);
                }
                vm.frame(1.0 / 30.0);
                continue;
            }
            "get" => {
                println!("[get] {} = {}", p[1], crate::dev::get(vm, p[1]));
                continue;
            }
            "set" => {
                println!("[set] {}", crate::dev::set(vm, p[1], &p[2..].join(",")));
                continue;
            }
            "call" => {
                println!("[call] {}", crate::dev::call(vm, p[1], &p[2..]));
                for _ in 0..2 {
                    vm.frame(1.0 / 30.0);
                }
                continue;
            }
            "inspect" => {
                // inspect,x,y: nodes under the point (960x600, top-left) with their components
                print!("{}", crate::dev::inspect(vm, p[1].parse().unwrap_or(0.0), p[2].parse().unwrap_or(0.0)));
                continue;
            }
            "ui" => {
                let s = crate::dev::ui(vm);
                match p.get(1) {
                    Some(f) => drop(std::fs::write(f, s)),
                    None => print!("{s}"),
                }
                continue;
            }
            "do" => {
                // step file: one per line, `#` = comment
                let txt = std::fs::read_to_string(p[1]).unwrap_or_else(|_| panic!("do: {} not found", p[1]));
                let v: Vec<String> = txt.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
                run_steps(vm, &v);
                continue;
            }
            "hover" => {
                let x: f32 = p[1].parse().unwrap();
                let y: f32 = p[2].parse().unwrap();
                let frames = p.get(3).map_or(1, |n| n.parse().unwrap());
                vm.eng.hover_always = true;
                vm.eng.mouse = (x, vm.eng.screen.1 as f32 - y);
                for _ in 0..frames {
                    vm.frame(1.0 / 30.0);
                }
                continue;
            }
            "click" => {
                // click on the visible element whose text matches (exact, else contains)
                let label = p[1..].join(",");
                match crate::dev::find_click(vm, &label) {
                    Some((x, y, l)) => {
                        println!("[click] {label} -> \"{}\" ({x:.0},{:.0})", l.replace('\n', " | "), 600.0 - y);
                        vm.eng.mouse = (x, y);
                        vm.eng.mouse_down = true;
                        vm.eng.mouse_down_edge = true;
                        vm.frame(1.0 / 30.0);
                        vm.eng.mouse_down = false;
                        vm.eng.mouse_up_edge = true;
                        for _ in 0..16 {
                            vm.frame(1.0 / 30.0);
                        }
                    }
                    None => println!("[click] {label}: not found"),
                }
                continue;
            }
            _ => {}
        }
        if p[0] == "pngl" {
            // dev: pngl,<file>,<profile>,<w>,<h>[,hold][,pointer y] : capture through a screen profile (layouts/README.md)
            let (w, h): (f32, f32) = (p[3].parse().unwrap(), p[4].parse().unwrap());
            if p[2] == "desktop" {
                let console = vm.eng.console.is_some();
                let (scale, rows) = if console {
                    let (k, rows, _) = crate::console_mode::fit_screen(vm, w, h);
                    crate::desktop_layout::rebuild(vm);
                    (k, rows)
                } else {
                    ((w / 960.0).min(h / 600.0), 600.0)
                };
                let old_width = vm.eng.desktop_layout.width();
                vm.eng.desktop_layout.set_width(w / scale);
                let mut r = crate::render::Renderer::new(w as usize, h as usize);
                if let Some(ms) = p.get(5).and_then(|v| v.parse::<f32>().ok()) {
                    r.set_subframe_time(ms / 1000.0);
                }
                r.render(vm, scale, 0.0, ((h - rows * scale) / 2.0).max(0.0));
                r.save_png(p[1]);
                vm.eng.desktop_layout.set_width(old_width);
                crate::desktop_layout::rebuild(vm);
                crate::save_slots::reflow(vm);
                continue;
            }
            let hold = p.get(5) == Some(&"hold");
            let py = p.iter().skip(5).find_map(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
            match crate::profile::load(p[2]) {
                Ok(prof) => {
                    let mut r = crate::render::Renderer::new(w as usize, h as usize);
                    r.regions = prof.place(w, h, hold, py);
                    r.render(vm, 1.0, 0.0, 0.0);
                    r.save_png(p[1]);
                }
                Err(e) => crate::elog!("[pngl] {e}"),
            }
            continue;
        }
        if p[0] == "png" {
            // dev: 960x600 capture of the current state
            let mut r = crate::render::Renderer::new(960, 600);
            r.render(vm, 1.0, 0.0, 0.0);
            r.save_png(p[1]);
            continue;
        }
        if p[0] == "w" {
            for _ in 0..p[1].parse::<usize>().unwrap() {
                vm.frame(1.0 / 30.0);
            }
            continue;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if p[0] == "wr" {
            let frames: usize = p[1].parse().unwrap();
            let start = std::time::Instant::now();
            for n in 1..=frames {
                vm.frame(1.0 / 30.0);
                let deadline = start + std::time::Duration::from_secs_f64(n as f64 / 30.0);
                if let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) {
                    std::thread::sleep(remaining);
                }
            }
            println!("[realtime] {} frames in {:.6}s", frames, start.elapsed().as_secs_f64());
            continue;
        }
        if p[0] == "h" {
            // held press (hover / tooltip)
            vm.eng.mouse = (p[1].parse().unwrap(), 600.0 - p[2].parse::<f32>().unwrap());
            vm.eng.mouse_down = true;
            vm.eng.mouse_down_edge = true;
            for _ in 0..12 {
                vm.frame(1.0 / 30.0);
            }
            continue;
        }
        if p[0] == "d" {
            let v: Vec<f32> = p[1..5].iter().map(|s| s.parse().unwrap()).collect();
            vm.eng.mouse = (v[0], 600.0 - v[1]);
            vm.eng.mouse_down = true;
            vm.eng.mouse_down_edge = true;
            vm.frame(1.0 / 30.0);
            for i in 1..=8 {
                let t = i as f32 / 8.0;
                vm.eng.mouse = (v[0] + (v[2] - v[0]) * t, 600.0 - (v[1] + (v[3] - v[1]) * t));
                vm.frame(1.0 / 30.0);
            }
            vm.eng.mouse_down = false;
            vm.eng.mouse_up_edge = true;
            vm.frame(1.0 / 30.0);
            for _ in 0..10 {
                vm.frame(1.0 / 30.0);
            }
            continue;
        }
        let x: f32 = p[0].parse().unwrap();
        let y: f32 = p[1].parse().unwrap();
        let wait: usize = p.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);
        vm.eng.mouse = (x, 600.0 - y);
        vm.eng.mouse_down = true;
        vm.eng.mouse_down_edge = true;
        vm.frame(1.0 / 30.0);
        vm.eng.mouse_down = false;
        vm.eng.mouse_up_edge = true;
        vm.frame(1.0 / 30.0);
        for _ in 0..wait {
            vm.frame(1.0 / 30.0);
        }
    }
}
