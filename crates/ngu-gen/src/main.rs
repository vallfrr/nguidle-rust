//! Bench / host of the generated game.
//!   ngu-gen <args...>                 : runs the generated entry method (unit tests vs oracle)
//!   ngu-gen game <dll> <out/ngu> [n]  : loads the scene with the engine, runs the native logic, n frames
#![allow(non_camel_case_types, non_snake_case, dead_code, unused_imports, unused_variables, unused_mut, unreachable_code, unused_parens, unused_assignments, static_mut_refs)]
mod ext {
    pub use ngu_rt::ext::*;
    include!("ext_stubs.rs");
}
mod gen;
mod entry;

use ngu_host::clr::{unity_api, Vm};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.first().map(|s| s.as_str()) == Some("game") {
        return game(&a[1..]);
    }
    if a.first().map(|s| s.as_str()) == Some("window") {
        return window(&a[1..]);
    }
    if a.first().map(|s| s.as_str()) == Some("fuzz") {
        return fuzz(&a[1..]);
    }
    // engine native calls (object concatenation...) require a VM: NGU_DLL loads it (unit bench)
    if let Ok(d) = std::env::var("NGU_DLL") {
        let vm: &'static mut Vm = Box::leak(Box::new(Vm::new(&d)));
        vm.native_mode = true;
        ngu_host::set_vm(vm as *mut Vm);
    }
    println!("=> {}", entry::entry(&a));
}

fn game(a: &[String]) {
    let mut vm = Vm::new(&a[0]);
    let dir = a[1].clone();
    let frames: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = format!("{}/save", dir);
    ngu_host::install_panic_hook();
    vm.native_mode = true;
    vm.native_inv = Some(gen::hook);
    ngu_host::set_vm(&mut vm as *mut Vm);
    unity_api::load_prefs(&mut vm);
    let t = std::time::Instant::now();
    vm.load_scene(&dir).expect("scene");
    println!("scene loaded in {:?}: {} GameObjects", t.elapsed(), vm.eng.gos.len());
    if std::env::var("NGU_DBG_ITEMLIST").is_ok() {
        use ngu_host::clr::value::{Ext, Value};
        for c in vm.eng.user_comps.clone() {
            if let Value::Obj(o) = &c {
                if vm.types[o.ty as usize].name == "AllItemListController" {
                    let slot = vm.types[o.ty as usize].fields.iter().position(|n| n == "itemList").unwrap_or(0);
                    let f0 = o.f.borrow()[slot].clone();
                    let desc = match &f0 {
                        Value::Obj(a) => match &*a.x.borrow() {
                            Ext::Arr(v) | Ext::List(v) => format!("{} elements of which {} non-null", v.len(), v.iter().filter(|x| !x.is_null()).count()),
                            Ext::None => "Ext::None".to_string(),
                            Ext::Dict(_) => "Ext::Dict".to_string(),
                            Ext::Native(_) => "Ext::Native".to_string(),
                            Ext::Boxed(_) => "Ext::Boxed".to_string(),
                            _ => "autre".to_string(),
                        },
                        _ => format!("{}", vm.describe(&f0)),
                    };
                    eprintln!("[dbg] AllItemListController.itemList (shell): {desc}");
                    eprintln!("[dbg] fields: {:?}", vm.types[o.ty as usize].fields.iter().take(6).collect::<Vec<_>>());
                }
            }
        }
    }
    let t = std::time::Instant::now();
    vm.awake_all();
    println!("Awake/OnEnable in {:?}", t.elapsed());
    for i in 0..frames {
        let t = std::time::Instant::now();
        vm.frame(1.0 / 30.0);
        println!("frame {i} in {:?} (native calls: {})", t.elapsed(), ngu_host::host().native_calls);
    }
    if let Ok(st) = std::env::var("NGU_STEPS") {
        let v: Vec<String> = st.split(';').map(|x| x.to_string()).collect();
        ngu_host::clr::script::run_steps(&mut vm, &v);
    }
    if let Ok(p) = std::env::var("NGU_DUMP") {
        std::fs::write(p, ngu_host::clr::unity::dump_state(&vm)).ok();
    }
    println!("missing intrinsics: {:?}", vm.missing);
}

/// Differential fuzz: for each static method with a primitive signature, same arguments in the interpreter (oracle)
/// and in the generated Rust; any difference in result (or exception) is reported.
fn fuzz(a: &[String]) {
    use ngu_host::clr::value::Value;
    let mut vm = Vm::new(&a[0]);
    let n: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let filter = a.get(2).cloned().unwrap_or_default();
    ngu_host::install_panic_hook();
    vm.native_mode = true;
    ngu_host::set_vm(&mut vm as *mut Vm);
    let mut seed = 0x9E3779B97F4A7C15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let ints: [i32; 12] = [0, 1, -1, 2, 3, 7, 10, 100, 255, 1000, 42, 12];
    let dbls: [f64; 16] = [0.0, 1.0, -1.0, 0.5, 2.0, 10.0, 99.9, 1000.0, 123456.789, 1e6, 1e9, 1.5e12, 1e18, 1e30, 1e300, -2.5];
    let strs = ["", "a", "123", "1.5", "Hello World", "-7", "abc,def", "10K"];
    let (mut total, mut diffs, mut skipped) = (0usize, 0usize, 0usize);
    let (mut exc_both, mut exc_one) = (0usize, 0usize);
    for &(mid, pk, rk) in gen::FUZZ {
        let name = {
            let mi = &vm.prog.methods[mid as usize];
            format!("{}::{}", vm.types[mi.owner as usize].name, mi.name)
        };
        if !filter.is_empty() && !name.contains(&filter) {
            continue;
        }
        // non-deterministic (clock, randomness) or platform-specific: out of scope
        if ["Random", "DateTime", "Epoch", "Crypto", "Steam", "Kartridge", "File", "Import", "Export", "Now"].iter().any(|k| name.contains(k)) {
            continue;
        }
        if std::env::var("NGU_FUZZ_VERBOSE").is_ok() {
            eprintln!("[fuzz] {name}");
        }
        for _ in 0..n {
            let args: Vec<Value> = pk
                .chars()
                .map(|c| match c {
                    'i' => Value::I32(ints[(rnd() % ints.len() as u64) as usize]),
                    'l' => Value::I64(ints[(rnd() % ints.len() as u64) as usize] as i64 * (rnd() % 1000) as i64),
                    'f' => Value::F32(dbls[(rnd() % dbls.len() as u64) as usize] as f32),
                    'd' => Value::F64(dbls[(rnd() % dbls.len() as u64) as usize]),
                    'b' => Value::I32((rnd() % 2) as i32),
                    _ => Value::str(strs[(rnd() % strs.len() as u64) as usize]),
                })
                .collect();
            total += 1;
            // natif
            vm.native_inv = Some(gen::hook);
            let rn = vm.invoke(mid, args.clone());
            // oracle
            vm.native_inv = None;
            let ro = vm.invoke(mid, args.clone());
            let show = |vm: &mut Vm, r: &Result<Value, Value>| -> String {
                match r {
                    Ok(v) => match v {
                        Value::F32(x) => format!("f32:{:08x}", x.to_bits()),
                        Value::F64(x) => format!("f64:{:016x}", x.to_bits()),
                        Value::Obj(o) if vm.types[o.ty as usize].name == "System.String" => format!("str:{:?}", ngu_host::clr::bcl_core::to_string(vm, v).unwrap_or_default()),
                        Value::Str(x) => format!("str:{:?}", &**x),
                        o => vm.describe(o),
                    },
                    Err(_) => "EXC".to_string(),
                }
            };
            let (sn, so) = (show(&mut vm, &rn), show(&mut vm, &ro));
            // NaN has several representations: compare as NaN
            let nan = |x: &str| (x.starts_with("f64:7ff8") || x.starts_with("f64:fff8") || x.starts_with("f32:7fc") || x.starts_with("f32:ffc"));
            let same = sn == so || (nan(&sn) && nan(&so));
            if sn == "EXC" && so == "EXC" {
                exc_both += 1;
            } else if (sn == "EXC") != (so == "EXC") {
                exc_one += 1;
            }
            if !same && !(sn == "EXC" && so == "EXC") {
                diffs += 1;
                if diffs <= 40 {
                    let d: Vec<String> = args.iter().map(|v| vm.describe(v)).collect();
                    println!("DIFF {name}({}) oracle={so} native={sn}", d.join(", "));
                }
            }
        }
        if pk.is_empty() && rk == "v" {
            skipped += 1;
        }
    }
    println!("fuzz: {total} calls, {diffs} differences ({exc_both} exceptions on both sides, {exc_one} on one side only), {skipped} without argument or return");
}

/// Desktop window: `ngu-gen window <dll> <out/ngu> [fps] [max_frames] [capture.png]`
fn window(a: &[String]) {
    let mut vm = Vm::new(&a[0]);
    let dir = a[1].clone();
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = format!("{}/save", dir);
    ngu_host::install_panic_hook();
    vm.native_mode = true;
    vm.native_inv = Some(gen::hook);
    ngu_host::set_vm(&mut vm as *mut Vm);
    unity_api::load_prefs(&mut vm);
    vm.load_scene(&dir).expect("scene");
    vm.awake_all();
    let o = ngu_desktop::Options {
        fps: a.get(2).and_then(|s| s.parse().ok()).unwrap_or(60),
        max_frames: a.get(3).and_then(|s| s.parse().ok()),
        screenshot: a.get(4).cloned(),
        ..Default::default()
    };
    if let Err(e) = ngu_desktop::run(&mut vm, o) {
        eprintln!("window: {e}");
    }
}
