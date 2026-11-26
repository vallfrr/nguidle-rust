//! Builds the exported scene description from the game's serialized files.

use super::typetree::{builtin, Reader};
use super::SerializedFile;
use serde_json::{Map, Value};
use std::path::Path;

type Result<T> = std::result::Result<T, String>;

/// Engine classes exported with their fields (name, class id).
const PLAIN: [(&str, i32); 8] = [
    ("GameObject", 1),
    ("Transform", 4),
    ("Camera", 20),
    ("Material", 21),
    ("Sprite", 213),
    ("CanvasRenderer", 222),
    ("Canvas", 223),
    ("RectTransform", 224),
];

/// Serialized files of the game in a stable order.
pub fn load_files(data_dir: &Path) -> Result<Vec<SerializedFile>> {
    let mut names: Vec<String> = std::fs::read_dir(data_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|n| {
            n == "globalgamemanagers"
                || n == "resources.assets"
                || n.starts_with("level") && n[5..].bytes().all(|b| b.is_ascii_digit())
                || n.starts_with("sharedassets") && n.ends_with(".assets")
                || n == "globalgamemanagers.assets"
        })
        .collect();
    names.sort();
    let mut files = names
        .iter()
        .map(|n| SerializedFile::open(&data_dir.join(n)))
        .collect::<Result<Vec<_>>>()?;
    for name in ["unity default resources", "unity_builtin_extra"] {
        let path = data_dir.join("Resources").join(name);
        if path.is_file() {
            files.push(SerializedFile::open(&path)?);
        }
    }
    Ok(files)
}

/// `{"t": class, "d": fields}` for every plain engine object, keyed `file:id`.
pub fn plain_objects(files: &[SerializedFile]) -> Result<Map<String, Value>> {
    let mut out = Map::new();
    for file in files {
        for object in &file.objects {
            let Some((name, _)) = PLAIN.iter().find(|(_, id)| *id == object.class_id) else {
                continue;
            };
            let layout = builtin(object.class_id).ok_or("Missing engine layout")?;
            let mut reader = Reader::new(file.bytes(object), file);
            let fields = reader
                .value(layout, true)
                .map_err(|e| format!("{}:{} ({name}): {e}", file.name, object.path_id))?;
            let mut record = Map::new();
            record.insert("t".into(), Value::String((*name).into()));
            record.insert("d".into(), fields);
            out.insert(
                format!("{}:{}", file.name, object.path_id),
                Value::Object(record),
            );
        }
    }
    Ok(out)
}

fn field<'a>(objects: &'a Map<String, Value>, reference: &Value) -> Option<&'a Map<String, Value>> {
    objects.get(reference.as_str()?)?.get("d")?.as_object()
}

fn name_of<'a>(objects: &'a Map<String, Value>, transform: &Map<String, Value>) -> Option<&'a str> {
    field(objects, transform.get("m_GameObject")?)?
        .get("m_Name")?
        .as_str()
}

/// The card drag preview starts over the sidebar; the game parks it offscreen
/// until a drag begins.
pub fn park_card_preview(objects: &mut Map<String, Value>) {
    let mut shifts = Vec::new();
    for (key, record) in objects.iter() {
        if record["t"] != "RectTransform" {
            continue;
        }
        let rect = record["d"].as_object().unwrap();
        if name_of(objects, rect) != Some("Ghost") {
            continue;
        }
        let parent = rect.get("m_Father").and_then(|p| field(objects, p));
        if parent.and_then(|p| name_of(objects, p)) == Some("Cards Menu") {
            let y = rect["m_AnchoredPosition"]["y"].as_f64().unwrap_or(0.0);
            shifts.push((key.clone(), -6000.0 - y));
        }
    }
    for (key, shift) in shifts {
        let rect = objects[&key]["d"].clone();
        let mut rect = rect;
        for field in ["m_AnchoredPosition", "m_LocalPosition"] {
            let y = rect[field]["y"].as_f64().unwrap_or(0.0) + shift;
            rect[field]["y"] = serde_json::json!(y);
        }
        objects.get_mut(&key).unwrap()["d"] = rect;
    }
}

/// Marks interface elements for online services the port does not offer.
pub fn hide_unavailable_online_ui(objects: &mut Map<String, Value>) {
    let mut hidden = Vec::new();
    for (_, record) in objects.iter() {
        if record["t"] != "RectTransform" {
            continue;
        }
        let rect = record["d"].as_object().unwrap();
        let Some(object) = rect.get("m_GameObject").and_then(|g| field(objects, g)) else {
            continue;
        };
        let name = object.get("m_Name").and_then(Value::as_str).unwrap_or("");
        let parent = rect.get("m_Father").and_then(|p| field(objects, p));
        let parent_name = parent.and_then(|p| name_of(objects, p));
        let hide = match parent_name {
            Some("Canvas") => matches!(
                name,
                "Sellout Kreds Canvas" | "Sellout Kreds 2 Canvas" | "Itopod Name List Canvas"
            ),
            Some("Title Panel") => {
                name.starts_with("Buy AP Wth Kred")
                    || matches!(name, "Special Packs" | "Special News" | "Special News 2")
            }
            Some("Info Menu + Controller") => {
                matches!(
                    name,
                    "Discord" | "Kart Promo Button" | "SteamMtx" | "Itopod Names List"
                )
            }
            Some("Settings Panel 2") => name == "Check For Updates",
            Some("Augment Settings 1") => name == "Submit Highscores",
            Some("Main Menu Screen") => {
                name.starts_with("Discord Button")
                    || matches!(
                        name,
                        "Kartridge Button" | "Kong Button" | "Cloud Save Border" | "Text"
                    )
            }
            Some("Splash Screen") => {
                name.starts_with("Discord Button")
                    || matches!(
                        name,
                        "Kartridge Button" | "Kong Button" | "Cloud Save Border"
                    )
            }
            _ => false,
        };
        if hide {
            hidden.push(rect["m_GameObject"].clone());
        }
    }
    for reference in hidden {
        if let Some(Value::Object(go)) = objects
            .get_mut(reference.as_str().unwrap())
            .and_then(|record| record.get_mut("d"))
        {
            go.insert("m_PortHidden".into(), Value::Bool(true));
        }
    }
}

/// Script identity of a MonoBehaviour class, as stored in its MonoScript.
#[derive(Debug, Clone)]
pub struct ScriptInfo {
    pub namespace: String,
    pub class: String,
    pub assembly: String,
}

impl ScriptInfo {
    /// `Assembly.dll|Namespace.Class`, the key for layouts.
    pub fn key(&self) -> String {
        match self.namespace.is_empty() {
            true => format!("{}|{}", self.assembly, self.class),
            false => format!("{}|{}.{}", self.assembly, self.namespace, self.class),
        }
    }

    fn json(&self) -> Value {
        serde_json::json!({ "ns": self.namespace, "cls": self.class, "asm": self.assembly })
    }
}

/// Every MonoScript of the game, keyed `file:pathid`.
pub fn scripts(files: &[SerializedFile]) -> Result<Vec<(String, ScriptInfo)>> {
    const MONO_SCRIPT: i32 = 115;
    let layout = builtin(MONO_SCRIPT).ok_or("Missing MonoScript layout")?;
    let mut out = Vec::new();
    for file in files {
        for object in file.objects.iter().filter(|o| o.class_id == MONO_SCRIPT) {
            let value = Reader::new(file.bytes(object), file).value(layout, true)?;
            let text = |name: &str| value[name].as_str().unwrap_or("").to_owned();
            out.push((
                format!("{}:{}", file.name, object.path_id),
                ScriptInfo {
                    namespace: text("m_Namespace"),
                    class: text("m_ClassName"),
                    assembly: text("m_AssemblyName"),
                },
            ));
        }
    }
    Ok(out)
}

/// MonoBehaviour records. `layout` returns the field layout of a script class,
/// or `None` when it cannot be built (the record then keeps no fields).
pub fn behaviours(
    files: &[SerializedFile],
    scripts: &[(String, ScriptInfo)],
    layout: &dyn Fn(&ScriptInfo) -> Option<std::rc::Rc<super::typetree::Node>>,
) -> Result<Map<String, Value>> {
    let mut out = Map::new();
    for file in files {
        for object in file.objects.iter().filter(|o| o.class_id == 114) {
            let bytes = file.bytes(object);
            let reference = bytes.get(16..28).ok_or("Short MonoBehaviour")?;
            let file_id = i32::from_le_bytes(reference[..4].try_into().unwrap());
            let path_id = i64::from_le_bytes(reference[4..].try_into().unwrap());
            let key = format!("{}:{path_id}", file.file_name(file_id));
            let info = scripts
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, info)| info);
            let mut fields = match info.and_then(layout) {
                Some(node) => Reader::new(bytes, file)
                    .value(&node, true)
                    .map_err(|e| format!("{}:{} ({}): {e}", file.name, object.path_id, key))?,
                None => Value::Object(Map::new()),
            };
            if let Value::Object(map) = &mut fields {
                map.shift_remove("m_Script");
            }
            let mut record = Map::new();
            record.insert("t".into(), "MonoBehaviour".into());
            record.insert("script".into(), info.map_or(Value::Null, ScriptInfo::json));
            record.insert("d".into(), fields);
            out.insert(
                format!("{}:{}", file.name, object.path_id),
                Value::Object(record),
            );
        }
    }
    Ok(out)
}

/// Two score labels are drawn smaller than the original so they fit the
/// port's font.
pub fn fit_card_labels(objects: &mut Map<String, Value>) {
    let mut targets = Vec::new();
    for (key, record) in objects.iter() {
        if record["script"]["cls"] != "Text" {
            continue;
        }
        let Some(text) = record["d"].as_object() else {
            continue;
        };
        let name = text
            .get("m_GameObject")
            .and_then(|g| field(objects, g))
            .and_then(|g| g.get("m_Name"))
            .and_then(Value::as_str);
        if matches!(name, Some("Total Mana Text" | "Mayo Gen Text"))
            && text.contains_key("m_FontData")
        {
            targets.push(key.clone());
        }
    }
    for key in targets {
        let font = &mut objects.get_mut(&key).unwrap()["d"]["m_FontData"];
        let size = font["m_FontSize"].clone();
        let small = size.as_i64().unwrap_or(10).min(10);
        font["m_BestFit"] = Value::Bool(true);
        font["m_MinSize"] = small.into();
        font["m_MaxSize"] = size;
    }
}
