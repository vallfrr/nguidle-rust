
use clr::*;

use clr::code::Op;
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: clr <dll> [scan|run Type::Method]");
    let mode = args.get(2).map(|s| s.as_str()).unwrap_or("info");
    if mode == "autoplay" {
        // loads its own VM through the console API
        if let Err(e) = autoplay::run_pc(&args[3..]) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let mut vm = Vm::new(path);
    match mode {
        "scan" => scan(&mut vm),
        "stats" => stats(&mut vm),
        "run" => run(&mut vm, &args[3..]),
        "load" => load(&mut vm, &args[3..]),
        "nrbf" => nrbf_dump(&args[3..]),
        "shot" => shot(&mut vm, &args[3..]),
        "click" => click(&mut vm, &args[3..]),
        "play" => play(&mut vm, &args[3..]),
        "fullgame" => fullgame(&mut vm, &args[3..]),
        _ => println!("types={} methodes={}", vm.n_user, vm.prog.methods.len()),
    }
}

/// Lowers all methods and counts missing intrinsics / unhandled operations.
fn scan(vm: &mut Vm) {
    let n = vm.prog.methods.len();
    let mut missing: HashMap<String, usize> = HashMap::new();
    let mut unsupported: HashMap<String, usize> = HashMap::new();
    let mut with_body = 0;
    for mid in 0..n as u32 {
        let has = {
            let m = &vm.prog.methods[mid as usize];
            m.mi.map(|x| vm.prog.res[x].body.is_some()).unwrap_or(false)
        };
        if !has {
            continue;
        }
        with_body += 1;
        let code = vm.lower(mid);
        for op in &code.ops {
            match op {
                Op::CallMissing { key, .. } | Op::NewObjMissing { key, .. } => *missing.entry(key.to_string()).or_default() += 1,
                Op::Unsupported(s) => *unsupported.entry(s.to_string()).or_default() += 1,
                _ => {}
            }
        }
    }
    println!("methods with body: {with_body}");
    let mut v: Vec<_> = missing.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("== missing intrinsics: {} ==", v.len());
    for (k, c) in &v {
        println!("{c:5} {k}");
    }
    let mut u: Vec<_> = unsupported.into_iter().collect();
    u.sort_by(|a, b| b.1.cmp(&a.1));
    println!("== unhandled ops: {} ==", u.len());
    for (k, c) in u.iter().take(30) {
        println!("{c:5} {k}");
    }
}

/// clr <dll> run Type::Method arg...   (args: integer, float with '.', or string)
fn run(vm: &mut Vm, a: &[String]) {
    let (t, m) = a[0].split_once("::").expect("Type::Methode");
    let tid = *vm.type_ids.get(t).unwrap_or_else(|| panic!("unknown type {t}"));
    let args: Vec<value::Value> = a[1..]
        .iter()
        .map(|s| {
            if let Ok(i) = s.parse::<i32>() {
                value::Value::I32(i)
            } else if let Ok(f) = s.parse::<f64>() {
                value::Value::F64(f)
            } else {
                value::Value::str(s)
            }
        })
        .collect();
    let mid = vm.find_method(tid, m, args.len()).unwrap_or_else(|| panic!("method {m}/{} not found", args.len()));
    if let Err(e) = vm.ensure_cctor(tid) {
        println!("exception in .cctor: {}", exc_text(vm, &e));
        return;
    }
    match vm.invoke(mid, args) {
        Ok(v) => println!("=> {}", bcl_core::to_string(vm, &v).unwrap_or_default()),
        Err(e) => println!("EXCEPTION: {}", exc_text(vm, &e)),
    }
}

fn exc_text(vm: &mut Vm, e: &value::Value) -> String {
    let n = vm.type_name_of(e);
    let msg = if let value::Value::Obj(o) = e { o.f.borrow().first().cloned() } else { None };
    format!("{} : {}", n, msg.map(|m| bcl_core::to_string(vm, &m).unwrap_or_default()).unwrap_or_default())
}

/// clr <dll> load <folder out/ngu> [frames]
fn load(vm: &mut Vm, a: &[String]) {
    let dir = a[0].clone();
    let frames: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = format!("{}/save", dir);
    vm.eng.desktop_panels = true;
    unity_api::load_prefs(vm);
    let t =std::time::Instant::now();
    if let Err(e) = vm.load_scene(&dir) {
        println!("ERREUR load_scene: {e}");
        return;
    }
    println!("scene loaded in {:?}: {} GameObjects", t.elapsed(), vm.eng.gos.len());
    let t = std::time::Instant::now();
    vm.awake_all();
    println!("Awake/OnEnable en {:?}", t.elapsed());
    let tb = std::time::Instant::now();
    let sb = vm.steps;
    for i in 0..frames {
        let t = std::time::Instant::now();
        vm.frame(1.0 / 30.0);
        if frames <= 12 {
            println!("frame {i} en {:?} (steps={})", t.elapsed(), vm.steps);
        }
    }
    let el = tb.elapsed();
    if let Ok(st) = std::env::var("NGU_STEPS") {
        let v: Vec<String> = st.split(';').map(|x| x.to_string()).collect();
        clr::script::run_steps(vm, &v);
    }
    if let Ok(p) = std::env::var("NGU_DUMP") {
        std::fs::write(p, unity::dump_state(vm)).ok();
    }
    println!("BENCH {} images en {:?} : {} instr, {:.2} M instr/s", frames, el, vm.steps - sb, (vm.steps - sb) as f64 / el.as_secs_f64() / 1e6);
    if let Some(p) = vm.prof.take() {
        let mut v: Vec<_> = p.into_iter().collect();
        v.sort_by(|a, b| b.1 .2.cmp(&a.1 .2));
        println!("== profile (exclusive instructions, inclusive, calls) ==");
        for (m, (calls, incl, excl)) in v.iter().take(30) {
            let mi = &vm.prog.methods[*m as usize];
            println!("{:>10} {:>10} {:>8}  {}::{}", excl, incl, calls, vm.types[mi.owner as usize].name, mi.name);
        }
    }
    let mut miss: Vec<_> = vm.missing.iter().cloned().collect();
    miss.sort();
    println!("missing intrinsics encountered: {:?}", miss);
}

/// clr <dll> shot <folder out/ngu> <frames> <output.png> [scale]
fn shot(vm: &mut Vm, a: &[String]) {
    let dir = a[0].clone();
    let frames: usize = a[1].parse().unwrap_or(3);
    let out = a[2].clone();
    let sc: f32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = format!("{}/save", dir);
    unity_api::load_prefs(vm);
    vm.load_scene(&dir).expect("scene");
    vm.awake_all();
    for _ in 0..frames {
        vm.frame(1.0 / 30.0);
    }
    let mut r = render::Renderer::new((960.0 * sc) as usize, (600.0 * sc) as usize);
    let t = std::time::Instant::now();
    r.render(vm, sc, 0.0, 0.0);
    println!("rendu en {:?}, {} dessins", t.elapsed(), r.draws);
    r.save_png(&out);
    println!("written {}", out);
}

/// clr <dll> click <folder> <frames_before> <x> <y> <frames_after> <output.png>  (x,y in virtual 960x600 px, top-left origin)
fn click(vm: &mut Vm, a: &[String]) {
    let dir = a[0].clone();
    let before: usize = a[1].parse().unwrap();
    let (x, y): (f32, f32) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let after: usize = a[4].parse().unwrap();
    let out = a[5].clone();
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = format!("{}/save", dir);
    unity_api::load_prefs(vm);
    vm.load_scene(&dir).expect("scene");
    vm.awake_all();
    for _ in 0..before {
        vm.frame(1.0 / 30.0);
    }
    vm.eng.mouse = (x, 600.0 - y);
    vm.eng.mouse_down = true;
    vm.eng.mouse_down_edge = true;
    vm.frame(1.0 / 30.0);
    vm.eng.mouse_down = false;
    vm.eng.mouse_up_edge = true;
    vm.frame(1.0 / 30.0);
    for _ in 0..after {
        vm.frame(1.0 / 30.0);
    }
    let mut r = render::Renderer::new(960, 600);
    let t = std::time::Instant::now();
    r.emit(vm, 1.0, 0.0, 0.0);
    println!("emit #1 : {:?} ({} quads)", t.elapsed(), r.quads.len());
    let t = std::time::Instant::now();
    r.emit(vm, 1.0, 0.0, 0.0);
    println!("emit #2 : {:?}", t.elapsed());
    if std::env::var("CLR_DIRTYLOG").is_ok() {
        unity::DIRTYLOG.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    for k in 0..4 {
        for _ in 0..3 {
            vm.frame(1.0 / 30.0);
        }
        let t = std::time::Instant::now();
        r.emit(vm, 1.0, 0.0, 0.0);
        println!("emit after 3 frames ({}): {:?} ({} quads) visits={} hits={}", k, t.elapsed(), r.quads.len(), r.visits, r.hits);
    }
    r.raster();
    r.save_png(&out);
    println!("written {}", out);
}

/// clr <dll> play <folder> <output.png> x,y[,wait] ...   (virtual 960x600 px, top-left origin; wait in frames, default 15)
/// A "w,N" step waits N frames; "d,x1,y1,x2,y2" drags from point 1 to point 2.
fn play(vm: &mut Vm, a: &[String]) {
    let dir = a[0].clone();
    let out = a[1].clone();
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = format!("{}/save", dir);
    let _ = std::fs::create_dir_all(&vm.eng.save_dir);
    // NGU_DESKTOP=1: the desktop screens and save picker of the applications (the default is the bare game screens).
    if std::env::var_os("NGU_DESKTOP").is_some() {
        vm.eng.desktop_panels = true;
    }
    if std::env::var_os("NGU_OPEN_FOLDER_LOG").is_some() {
        // Developer check of the "Open saves folder" button: desktop screens, and the click is written to this file.
        vm.eng.desktop_panels = true;
        vm.eng.open_folder = Some(|dir| {
            if let Some(log) = std::env::var_os("NGU_OPEN_FOLDER_LOG") { let _ = std::fs::write(log, dir); }
        });
    }
    unity_api::load_prefs(vm);
    vm.load_scene(&dir).expect("scene");
    vm.awake_all();
    if vm.eng.desktop_panels { vm.start_desktop_save_picker().expect("save picker"); }
    for _ in 0..40 {
        vm.frame(1.0 / 30.0);
    }
    clr::script::run_steps(vm, &a[2..]);
    if std::env::var_os("NGU_GUARD_STATS").is_some() {
        for (id, hits) in clr::vm::GUARD_HITS.iter().enumerate().skip(1) {
            println!("[guard] {id} {}", hits.load(std::sync::atomic::Ordering::Relaxed));
        }
        println!("[steps] {}", vm.steps);
    }
    let mut r = render::Renderer::new(960, 600);
    r.render(vm, 1.0, 0.0, 0.0);
    r.save_png(&out);
    println!("written {}", out);
}

/// clr <dll> fullgame <prepared folder> [report file] [game hours]: plays the whole game and prints OK / KO / SKIP per step.
fn fullgame(vm: &mut Vm, a: &[String]) {
    let dir = a.first().cloned().unwrap_or_else(|| {
        eprintln!("usage: clr <dll> fullgame <prepared folder> [report file] [game hours]");
        std::process::exit(2)
    });
    let hours: f32 = a.get(2).and_then(|h| h.parse().ok()).unwrap_or(1.0);
    // NGU_FULLGAME_WORK: the folder for the temporary saves (WebAssembly has no temporary folder of its own).
    let work = std::env::var_os("NGU_FULLGAME_WORK").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::env::temp_dir().join(format!("ngu-fullgame-{}", std::process::id()))
    });
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = work.join("save").to_string_lossy().into_owned();
    let _ = std::fs::create_dir_all(&vm.eng.save_dir);
    // Like the real applications: desktop panels, and the save screens in place of the original file dialogs.
    vm.eng.desktop_panels = true;
    unity_api::load_prefs(vm);
    vm.load_scene(&dir).expect("scene");
    vm.awake_all();
    vm.start_desktop_save_picker().expect("save picker");
    for _ in 0..40 {
        vm.frame(1.0 / 30.0);
    }
    let checks = clr::fullgame::run(vm, &clr::fullgame::Options { hours, workdir: work });
    let (text, ok) = clr::fullgame::report(&checks);
    print!("{text}");
    if let Some(file) = a.get(1) {
        let _ = std::fs::write(file, &text);
    }
    std::process::exit(if ok { 0 } else { 1 });
}

/// IL feature statistics: sizes the C# -> Rust generator.
fn stats(vm: &mut Vm) {
    use clr::code::{EhKind, Op};
    let n = vm.prog.methods.len();
    let mut tot = 0;
    let mut c: std::collections::BTreeMap<&str, usize> = Default::default();
    let mut ops_total = 0usize;
    let mut with_eh = 0;
    let mut simple = 0; // no EH, no virtual, no delegate, no ext newobj, no ldelema, no Ptr
    let mut ops_hist: HashMap<String, usize> = HashMap::new();
    for mid in 0..n as u32 {
        let has = {
            let m = &vm.prog.methods[mid as usize];
            m.mi.map(|x| vm.prog.res[x].body.is_some()).unwrap_or(false)
        };
        if !has {
            continue;
        }
        tot += 1;
        let code = vm.lower(mid);
        ops_total += code.ops.len();
        let mut f = [false; 8];
        if !code.eh.is_empty() {
            f[0] = true;
            with_eh += 1;
        }
        for e in &code.eh {
            if matches!(e.kind, EhKind::Filter(_)) {
                *c.entry("filtres").or_default() += 1;
            }
        }
        for op in &code.ops {
            let name = match op {
                Op::CallVirtUser(_) => { f[1] = true; "callvirt_user" }
                Op::CallVirtExt { .. } => { f[2] = true; "callvirt_ext" }
                Op::DelegateInvoke { .. } | Op::NewDelegate | Op::Ldftn(_) | Op::Ldvirtftn(_) => { f[3] = true; "delegue" }
                Op::Ldelema | Op::Ldflda(_) | Op::Ldsflda(..) | Op::Ldloca(_) | Op::Ldarga(_) | Op::Ldind | Op::Stind(_) | Op::Ldobj | Op::Stobj | Op::Initobj(_) => { f[4] = true; "adresses" }
                Op::Throw | Op::Rethrow => { f[5] = true; "throw" }
                Op::Box(_) | Op::Unbox(_) => { f[6] = true; "box" }
                Op::CallMissing { .. } | Op::NewObjMissing { .. } | Op::Unsupported(_) => { f[7] = true; "missing" }
                Op::Call(_) => "call",
                Op::CallNative { .. } => "call_native",
                Op::NewObj { .. } => "newobj",
                Op::NewObjExt { .. } => "newobj_ext",
                Op::Castclass(_) | Op::Isinst(_) => "cast",
                Op::Ldfld(_) | Op::Stfld(_) => "champ",
                Op::Ldsfld(..) | Op::Stsfld(..) | Op::LdsfldExt(_) => "static",
                Op::Newarr(_) | Op::Ldelem | Op::Stelem | Op::Ldlen => "tableau",
                Op::Switch(_) => "switch",
                _ => "autre",
            };
            *ops_hist.entry(name.to_string()).or_default() += 1;
        }
        let names = ["with_EH", "callvirt_user", "callvirt_ext", "delegate", "addresses", "throw", "box", "missing"];
        for (i, b) in f.iter().enumerate() {
            if *b {
                *c.entry(names[i]).or_default() += 1;
            }
        }
        if !f[0] && !f[1] && !f[3] && !f[4] && !f[7] {
            simple += 1;
        }
    }
    println!("methods with body: {tot}, instructions: {ops_total}, with EH: {with_eh}, \"simple\" (no EH/user virtual/delegate/addresses/missing): {simple}");
    println!("methods containing: {:?}", c);
    let mut h: Vec<_> = ops_hist.into_iter().collect();
    h.sort_by(|a, b| b.1.cmp(&a.1));
    println!("operation histogram (families): {:?}", h);
    // iterator classes (coroutines) and classes inheriting from an external type
    let mut iter = 0;
    for t in vm.types.iter().take(vm.n_user as usize) {
        if t.methods.contains_key("MoveNext") {
            iter += 1;
        }
    }
    println!("classes with MoveNext (iterators/coroutines): {iter}");
}

/// clr <dll> nrbf <save file> : summary of the BinaryFormatter streams of a real save (outer SaveData, inner PlayerData).
fn nrbf_dump(a: &[String]) {
    let _ = a;
    let txt = std::fs::read_to_string(&a[0]).expect("save file");
    let outer = decode_b64(txt.trim());
    let s = clr::nrbf::parse(&outer).expect("outer stream");
    if std::env::var("NRBF_FLAT").is_ok() {
        // inner PlayerData stream flattened, one field per line
        if let clr::nrbf::N::Obj(o) = &s.root {
            if let clr::nrbf::N::Str(_, inner) = &o.vals[0] {
                let s2 = clr::nrbf::parse(&decode_b64(inner)).expect("inner stream");
                print!("{}", clr::nrbf::flatten(&s2));
            }
        }
        return;
    }
    println!("== outer ==
{}", clr::nrbf::summary(&s));
    if let clr::nrbf::N::Obj(o) = &s.root {
        if let clr::nrbf::N::Str(_, inner) = &o.vals[0] {
            let bytes = decode_b64(inner);
            match clr::nrbf::parse(&bytes) {
                Ok(s2) => println!("== inner ({} bytes) ==
{}
== root shape ==
{}", bytes.len(), clr::nrbf::summary(&s2), clr::nrbf::root_shape(&s2)),
                Err(e) => println!("inner parse error: {e}"),
            }
        }
    }
}

fn decode_b64(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => 255,
    };
    let (mut out, mut acc, mut bits) = (vec![], 0u32, 0);
    for c in s.bytes().filter(|c| *c != b'=') {
        acc = (acc << 6) | val(c) as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}
