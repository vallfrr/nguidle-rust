//! Scripted input scenarios (clicks, drags, waits) shared by the oracle and native mode.
use crate::Vm;

/// Steps: `x,y[,wait]` click (virtual 960x600 px, top-left origin; wait in frames, default 15),
/// `w,N` waits N frames, `h,x,y` holds a press, `d,x1,y1,x2,y2` drags from point 1 to point 2.
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
            } else if let Some(f) = crate::focus::navigate(vm, p[1].chars().next().unwrap_or('d')) {
                println!("[focus] {} -> {:<8} ({:>3.0},{:>3.0})-({:>3.0},{:>3.0}) {}", p[1], f.kind, f.rect[0], f.rect[1], f.rect[2], f.rect[3], f.label.chars().take(40).collect::<String>());
            } else {
                println!("[focus] {} -> no element", p[1]);
            }
            vm.frame(1.0 / 30.0);
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
                // wheel,x,y,notches : mouse wheel at a point (positive = up)
                vm.eng.mouse = (p[1].parse().unwrap(), 600.0 - p[2].parse::<f32>().unwrap());
                vm.scroll_wheel(p[3].parse().unwrap());
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
