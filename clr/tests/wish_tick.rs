use clr::value::{Ext, Obj, Value};
use clr::Vm;
use std::rc::Rc;

fn object(value: &Value) -> Rc<Obj> {
    let Value::Obj(object) = value else { panic!("Expected an object") };
    Rc::clone(object)
}

fn field(vm: &Vm, object: &Obj, name: &str) -> usize {
    vm.types[object.ty as usize].fields.iter().position(|field| field == name).unwrap()
}

fn member(vm: &Vm, owner: &Obj, name: &str) -> Rc<Obj> {
    object(&owner.f.borrow()[field(vm, owner, name)])
}

fn tick(vm: &mut Vm, method: u32, controller: &Rc<Obj>) {
    if let Err(error) = vm.invoke(method, vec![Value::Obj(Rc::clone(controller))]) {
        panic!("Wish update raised {}", vm.describe(&error));
    }
}

fn late_game(dll: &str, assets: &str, save: &str) -> Vm {
    let mut vm = Vm::new(dll);
    vm.eng.screen = (960, 600);
    vm.load_scene(assets).expect("load prepared game assets");
    vm.awake_all();
    vm.start_pending();
    clr::script::run_steps(&mut vm, &[format!("l,{save}")]);
    vm
}

fn run(dll: &str, assets: &str, save: &str, guarded: bool) -> (Vec<(i32, u32)>, u64, u64) {
    let mut vm = late_game(dll, assets, save);
    let controller = vm.eng.user_comps.iter().find_map(|component| {
        let Value::Obj(component) = component else { return None };
        (vm.types[component.ty as usize].name == "WishesController").then(|| Rc::clone(component))
    }).expect("wish controller");
    let update = vm.find_method(controller.ty, "updateAllWishes", 1).unwrap();
    let progress = vm.find_method(controller.ty, "progressPerTick", 2).unwrap();
    if !guarded {
        let methods = &mut Rc::get_mut(&mut vm.prog).unwrap().methods;
        methods[update as usize].guard = 0;
        methods[progress as usize].guard = 0;
    }
    let character = member(&vm, &controller, "character");
    let wishes = member(&vm, &character, "wishes");
    let entries = member(&vm, &wishes, "wishes");
    let entries = match &*entries.x.borrow() {
        Ext::List(items) => items.iter().map(object).collect::<Vec<_>>(),
        _ => panic!("Expected wish list"),
    };
    wishes.f.borrow_mut()[field(&vm, &wishes, "wishesOn")] = Value::I32(1);
    let resources = ["energy", "magic", "res3"].map(|name| field(&vm, &entries[0], name));
    let level = field(&vm, &entries[0], "level");
    let fraction = field(&vm, &entries[0], "progress");
    for entry in &entries {
        let mut fields = entry.f.borrow_mut();
        for slot in resources { fields[slot] = Value::I64(0); }
    }
    let start = vm.steps;
    tick(&mut vm, update, &controller);
    let idle_steps = vm.steps - start;

    character.f.borrow_mut()[field(&vm, &character, "energyPower")] = Value::F32(1_000_000.0);
    let magic = member(&vm, &character, "magic");
    magic.f.borrow_mut()[field(&vm, &magic, "magicPower")] = Value::F32(1_000_000.0);
    let res3 = member(&vm, &character, "res3");
    res3.f.borrow_mut()[field(&vm, &res3, "res3Power")] = Value::F32(1_000_000.0);
    for (i, entry) in entries.iter().take(3).enumerate() {
        let mut fields = entry.f.borrow_mut();
        for slot in resources { fields[slot] = Value::I64(1_000_000_000); }
        fields[level] = Value::I32(0);
        fields[fraction] = Value::F32(if i == 0 { 0.999_999_94 } else { 0.0 });
    }
    entries[2].f.borrow_mut()[resources[1]] = Value::I64(0);
    let start = vm.steps;
    tick(&mut vm, update, &controller);
    let active_steps = vm.steps - start;
    let result = entries.iter().map(|wish| {
        let fields = wish.f.borrow();
        (fields[level].as_i32(), fields[fraction].as_f32().to_bits())
    }).collect::<Vec<_>>();
    assert_eq!(result[0], (1, 0), "wish gained one level and reset its bar");
    assert!(f32::from_bits(result[1].1) > 0.0, "allocated wish gained progress");
    assert_eq!(result[2], (0, 0), "partly allocated wish stayed idle");
    (result, idle_steps, active_steps)
}

#[test]
#[ignore = "requires assets and a late-game save from the verified installation"]
fn wish_tick_preserves_levelups_and_idle_wishes() {
    let dll = std::env::var("NGU_TEST_DLL").unwrap();
    let assets = std::env::var("NGU_TEST_ASSETS").unwrap();
    let save = std::env::var("NGU_TEST_SAVE").unwrap();
    let (optimized, idle_steps, active_steps) = run(&dll, &assets, &save, true);
    let (oracle, oracle_idle, oracle_active) = run(&dll, &assets, &save, false);
    assert_eq!(optimized, oracle);
    assert!(idle_steps < oracle_idle && active_steps < oracle_active);
}

#[test]
#[ignore = "requires assets and a late-game save from the verified installation"]
fn titan_checks_preserve_state_with_autokill_on_and_off() {
    let dll = std::env::var("NGU_TEST_DLL").unwrap();
    let assets = std::env::var("NGU_TEST_ASSETS").unwrap();
    let save = std::env::var("NGU_TEST_SAVE").unwrap();
    for enabled in ["false", "true"] {
        let run = |guarded: bool| {
            let mut vm = late_game(&dll, &assets, &save);
            if !guarded {
                for method in &mut Rc::get_mut(&mut vm.prog).unwrap().methods {
                    if (4..=7).contains(&method.guard) || method.guard == 9 { method.guard = 0; }
                }
            }
            let steps = vm.steps;
            clr::script::run_steps(&mut vm, &[
                format!("set,Character.settings.autoKillTitans,{enabled}"),
                "w,90".into(),
            ]);
            (clr::unity::dump_state(&vm), vm.steps - steps)
        };
        let (optimized, steps) = run(true);
        let (oracle, oracle_steps) = run(false);
        assert!(optimized == oracle, "visible state with auto-kill {enabled}");
        assert!(steps < oracle_steps, "instruction count with auto-kill {enabled}");
    }
}

#[test]
#[ignore = "requires assets and a late-game save from the verified installation"]
fn cooking_uses_the_exact_score_after_recipe_changes() {
    let dll = std::env::var("NGU_TEST_DLL").unwrap();
    let assets = std::env::var("NGU_TEST_ASSETS").unwrap();
    let save = std::env::var("NGU_TEST_SAVE").unwrap();
    let mut vm = late_game(&dll, &assets, &save);
    let controller = vm.eng.user_comps.iter().filter_map(|component| match component {
        Value::Obj(component) if vm.types[component.ty as usize].name == "CookingController" => Some(component.clone()),
        _ => None,
    }).next().unwrap();
    let character = member(&vm, &controller, "character");
    let cooking = member(&vm, &character, "cooking");
    let target = field(&vm, &cooking, "pair1Target");
    let method = vm.find_method(controller.ty, "getOptimalScore", 1).unwrap();
    let score = |vm: &mut Vm| {
        let start = vm.steps;
        let score = vm.invoke(method, vec![Value::Obj(controller.clone())]).unwrap_or_else(|err| {
            panic!("Cooking score calculation raised {}", vm.describe(&err))
        });
        (score.as_f32().to_bits(), vm.steps - start)
    };
    let (first, initial_steps) = score(&mut vm);
    let (cached, cached_steps) = score(&mut vm);
    assert_eq!(cached, first);
    assert!(cached_steps < initial_steps);

    let old = cooking.f.borrow()[target].as_i32();
    cooking.f.borrow_mut()[target] = Value::I32(old + 10);
    let (changed, changed_steps) = score(&mut vm);
    assert!(changed_steps > cached_steps);
    Rc::get_mut(&mut vm.prog).unwrap().methods[method as usize].guard = 0;
    assert_eq!(changed, score(&mut vm).0);
}

#[test]
#[ignore = "requires assets and a save from the verified installation"]
fn achievement_scan_preserves_thresholds_and_bonus_points() {
    let dll = std::env::var("NGU_TEST_DLL").unwrap();
    let assets = std::env::var("NGU_TEST_ASSETS").unwrap();
    let save = std::env::var("NGU_TEST_SAVE").unwrap();
    let run = |guarded: bool| {
        let mut vm = late_game(&dll, &assets, &save);
        let controller = vm.eng.user_comps.iter().find_map(|component| {
            let Value::Obj(component) = component else { return None };
            (vm.types[component.ty as usize].name == "AllAchievementsController")
                .then(|| Rc::clone(component))
        }).unwrap();
        let character = member(&vm, &controller, "character");
        let achievements = member(&vm, &character, "achievements");
        let completed = member(&vm, &achievements, "achievementComplete");
        {
            let Ext::List(entries) = &mut *completed.x.borrow_mut() else { panic!("Expected achievement list") };
            entries.fill(Value::bool(false));
        }
        let method = vm.find_method(controller.ty, "checkAchievements", 1).unwrap();
        if !guarded {
            Rc::get_mut(&mut vm.prog).unwrap().methods[method as usize].guard = 0;
        }
        let start = vm.steps;
        vm.invoke(method, vec![Value::Obj(controller.clone())]).unwrap_or_else(|error| {
            panic!("Achievement scan raised {}", vm.describe(&error))
        });
        let steps = vm.steps - start;
        let Ext::List(entries) = &*completed.x.borrow() else { panic!("Expected achievement list") };
        let flags = entries.iter().map(Value::as_bool).collect::<Vec<_>>();
        let totals = ["totalBP", "totalAchievementCount"].map(|name| {
            controller.f.borrow()[field(&vm, &controller, name)].as_i64()
        });
        ((totals, flags), steps)
    };
    let (optimized, steps) = run(true);
    let (reference, reference_steps) = run(false);
    assert_eq!(optimized, reference);
    assert!(optimized.0[1] > 0);
    assert!(steps < reference_steps);
}

#[test]
#[ignore = "requires assets and a save from the verified installation"]
fn boss_thresholds_and_loadout_shortcuts_match_the_interpreter() {
    let dll = std::env::var("NGU_TEST_DLL").unwrap();
    let assets = std::env::var("NGU_TEST_ASSETS").unwrap();
    let save = std::env::var("NGU_TEST_SAVE").unwrap();
    let run = |guarded: bool| {
        let mut vm = late_game(&dll, &assets, &save);
        let component = |name: &str| vm.eng.user_comps.iter().find_map(|value| {
            value.as_obj().filter(|obj| vm.types[obj.ty as usize].name == name).cloned()
        }).unwrap();
        let character = component("Character");
        let settings = member(&vm, &character, "settings");
        let loadout = vm.eng.user_comps.iter().find_map(|value| {
            value.as_obj().filter(|obj| {
                vm.types[obj.ty as usize].name == "LoadoutController"
                    && obj.f.borrow()[field(&vm, obj, "id")].as_i32() == -1
            }).cloned()
        }).expect("equipped helmet slot");
        let boss = vm.find_method(character.ty, "effectiveBossID", 1).unwrap();
        let shortcut = vm.find_method(loadout.ty, "Update", 1).unwrap();
        if !guarded {
            let methods = &mut Rc::get_mut(&mut vm.prog).unwrap().methods;
            methods[boss as usize].guard = 0;
            methods[shortcut as usize].guard = 0;
        }
        let difficulty_field = field(&vm, &settings, "rebirthDifficulty");
        let boss_field = field(&vm, &character, "bossID");
        let start = vm.steps;
        let thresholds = [-1, 0, 1, 2, 3].map(|difficulty| {
            settings.f.borrow_mut()[difficulty_field] = Value::I32(difficulty);
            [-1, 301, i32::MAX].map(|value| {
                character.f.borrow_mut()[boss_field] = Value::I32(value);
                vm.invoke(boss, [Value::Obj(character.clone())])
                    .unwrap_or_else(|error| panic!("Boss threshold raised {}", vm.describe(&error)))
                    .as_i32()
            })
        });
        let boss_steps = vm.steps - start;
        loadout.f.borrow_mut()[field(&vm, &loadout, "hovered")] = Value::bool(true);
        settings.f.borrow_mut()[field(&vm, &settings, "simpleInvShortcuts")] = Value::bool(true);
        let start = vm.steps;
        tick(&mut vm, shortcut, &loadout);
        let idle_steps = vm.steps - start;
        let shortcut_steps = [97, 100].map(|key| {
            vm.eng.keys_edge.insert(key);
            let start = vm.steps;
            tick(&mut vm, shortcut, &loadout);
            vm.eng.keys_edge.remove(&key);
            vm.steps - start
        });
        (thresholds, boss_steps, idle_steps, shortcut_steps)
    };
    let (thresholds, boss_steps, idle, shortcuts) = run(true);
    let (oracle_thresholds, oracle_steps, oracle_idle, oracle_shortcuts) = run(false);
    assert_eq!(thresholds, oracle_thresholds);
    assert!(boss_steps < oracle_steps && idle < oracle_idle);
    assert_eq!(shortcuts, oracle_shortcuts);
    assert!(shortcuts.iter().all(|&steps| steps > 0));
}
