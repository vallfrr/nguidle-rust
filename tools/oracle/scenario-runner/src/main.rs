use clr::{autoplay::{self, Act}, value::Value, Vm};
use std::{env, fs};

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 6, "dll assets save scenario output");
    let mut vm = Vm::new(&args[1]);
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = format!("{}/save", &args[2]);
    vm.eng.desktop_panels = true;
    clr::unity_api::load_prefs(&mut vm);
    vm.load_scene(&args[2]).expect("scene");
    vm.awake_all();
    for _ in 0..90 { vm.frame(1.0 / 30.0); }
    clr::script::run_steps(&mut vm, &["call,MainMenuController.finishMainMenu".into(), format!("l,{}", args[3])]);
    clr::unity_api::lookup("UnityEngine.Random::InitState").unwrap()(&mut vm, &mut vec![Value::I32(12345)]).ok().expect("seed");
    let energy = vm.eng.user_comps.iter().find_map(|v| v.as_obj().filter(|o| vm.types[o.ty as usize].name == "Energy").cloned()).unwrap();
    let update = vm.find_method(energy.ty, "updateBar", 1).unwrap();
    let calls = |vm: &Vm| vm.prof.as_ref().and_then(|p| p.get(&update)).map_or(0, |n| n.0);
    let initial_calls = calls(&vm);
    let scenario = fs::read_to_string(&args[4]).unwrap();
    let events = autoplay::parse(&scenario).unwrap();
    let mut trace = String::from("scheduled_tick,action,menu,energy,idle,cap,gained,bar\n");
    let mut tick = 0;
    for (time, acts) in events {
        while tick < time { vm.frame(1.0 / 30.0); tick += 1; }
        for action in acts {
            let (label, argument) = match action {
                Act::Menu(id) => { autoplay::swap_menu(&mut vm, id); ("menu", None) },
                Act::Mark(_) => ("mark", None),
                Act::Check(name) => ("check", Some(name)),
                Act::End => ("end", None),
                _ => panic!("unsupported action"),
            };
            if let Some(name) = argument {
                assert!(!name.contains('/') && !name.contains('\\') && !name.contains(".."));
                fs::write(format!("{}.{}.sav", args[5], name), autoplay::save_text(&mut vm).unwrap()).unwrap();
            }
            let character = vm.eng.user_comps.iter().find_map(|v| v.as_obj().filter(|o| vm.types[o.ty as usize].name == "Character").cloned()).unwrap();
            eprintln!("tick {time}: energy updates {}", calls(&vm) - initial_calls);
            let fields = ["menuID", "curEnergy", "idleEnergy", "capEnergy", "energyGained", "energyBarProgress"];
            trace.push_str(&format!("{time},{label}"));
            for name in fields {
                let slot = vm.types[character.ty as usize].fields.iter().position(|f| f == name).unwrap();
                let value = &character.f.borrow()[slot];
                if name == "energyBarProgress" { trace.push_str(&format!(",{}", value.as_f32())); }
                else { trace.push_str(&format!(",{}", value.as_i64())); }
            }
            trace.push('\n');
        }
    }
    fs::write(&args[5], trace).unwrap();
}
