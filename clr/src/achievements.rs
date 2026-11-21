//! Local progression for the installation's platform achievements.
use crate::save_slots::{atomic_write, game_call, game_component};
use crate::value::{Ext, Value};
use crate::Vm;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(PartialEq)]
pub(crate) struct Entry {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub earned: bool,
}

#[derive(Default)]
pub(crate) struct Book {
    next_check: f64,
    saved: Option<BTreeSet<i32>>,
    earned: BTreeSet<i32>,
    pub entries: Vec<Entry>,
    pub write_failed: bool,
}

fn fields(vm: &Vm, object: &Value) -> Vec<Value> {
    let Value::Obj(o) = object else { return Vec::new() };
    vm.eng.read_live_fields.and_then(|read| read(object)).unwrap_or_else(|| o.f.borrow().clone())
}

fn field(vm: &Vm, object: &Value, values: &[Value], name: &str) -> Value {
    let Value::Obj(o) = object else { return Value::Null };
    vm.types[o.ty as usize].fields.iter().position(|f| f == name)
        .and_then(|i| values.get(i)).cloned().unwrap_or_default()
}

pub(crate) fn tick(vm: &mut Vm) {
    if !vm.eng.desktop_panels || vm.eng.save_dir.is_empty() || vm.eng.time < vm.eng.achievements.next_check {
        return;
    }
    vm.eng.achievements.next_check = vm.eng.time + 1.0;
    let Some(controller) = game_component(vm, "SteamManager") else { return };
    let started = game_component(vm, "MainMenuController").is_some_and(|title| {
        field(vm, &title, &fields(vm, &title), "doneInitialLoad").as_bool()
    });
    if started {
        let _ = game_call(vm, &controller, "checkAllAchievements", vec![]);
    }
    let Value::Obj(array) = field(vm, &controller, &fields(vm, &controller), "m_Achievements") else {
        return;
    };
    let entries = match &*array.x.borrow() {
        Ext::Arr(a) => a.clone(),
        _ => return,
    };
    let mut catalog = Vec::with_capacity(entries.len());
    for object in entries {
        let f = fields(vm, &object);
        let read = |name| field(vm, &object, &f, name);
        let name = read("m_strName");
        let description = read("m_strDescription");
        let (Value::I32(id), Some(name), Some(description)) =
            (read("m_eAchievementID"), name.as_str(), description.as_str()) else { continue };
        catalog.push(Entry {
            id, name: name.into(), description: description.into(), earned: read("m_bAchieved").as_bool(),
        });
    }
    let file = Path::new(&vm.eng.save_dir).join("achievements.json");
    let shared_profile = vm.browser_saves_enabled();
    let book = &mut vm.eng.achievements;
    let last_failure = book.write_failed;
    if book.saved.is_none() || shared_profile {
        let stored: BTreeSet<i32> = std::fs::read(&file).ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        book.earned.extend(stored.iter().copied());
        book.saved = Some(stored);
    }
    for entry in &mut catalog {
        if entry.earned {
            book.earned.insert(entry.id);
        }
        entry.earned = book.earned.contains(&entry.id);
    }
    let changed = book.entries != catalog;
    book.entries = catalog;
    if book.saved.as_ref() != Some(&book.earned) {
        let bytes = serde_json::to_vec(&book.earned).expect("integer achievement IDs serialize");
        book.write_failed = atomic_write(&file, &bytes).is_err();
        if !book.write_failed {
            book.saved = Some(book.earned.clone());
        }
    }
    if changed || last_failure != book.write_failed {
        crate::save_slots::refresh_achievements(vm);
    }
}
