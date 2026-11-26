//! Exports the game's Unity data: the scene file, textures, fonts and texts.

use super::assets;
use super::layout::Layouts;
use super::scene::{self, ScriptInfo};
use super::typetree::{builtin, Reader};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;
use std::rc::Rc;

type Result<T> = std::result::Result<T, String>;

const TAG_MANAGER: i32 = 78;
const RESOURCE_MANAGER: i32 = 147;

/// Counts reported after an export.
#[derive(Debug)]
pub struct Summary {
    pub objects: usize,
    pub resources: usize,
    /// Objects that could not be read; the export fails if any are listed.
    pub errors: Vec<String>,
}

/// Writes `scene.json`, `tags.json`, `tex/`, `fonts/` and `text/` into `out`.
/// `data_dir` is the game's `NGUIdle_Data` folder.
pub fn export(data_dir: &Path, out: &Path) -> Result<Summary> {
    let files = scene::load_files(data_dir)?;
    let infos = scene::scripts(&files)?;
    let layouts = Layouts::new(data_dir.join("Managed"));
    let layout = |info: &ScriptInfo| {
        let class = info.key().split_once('|')?.1.to_owned();
        layouts.script(&info.assembly, &class).ok().map(Rc::new)
    };
    let mut objects = scene::plain_objects(&files)?;
    objects.extend(scene::behaviours(&files, &infos, &layout)?);
    let (records, errors) = assets::export(&files, data_dir, out, true)?;
    objects.extend(records);
    scene::park_card_preview(&mut objects);
    scene::fit_card_labels(&mut objects);
    scene::hide_unavailable_online_ui(&mut objects);

    let mut resources = Vec::new();
    let mut tags = None;
    for file in &files {
        for object in &file.objects {
            match object.class_id {
                RESOURCE_MANAGER if resources.is_empty() => {
                    let layout = builtin(object.class_id).ok_or("Missing engine layout")?;
                    let value = Reader::new(file.bytes(object), file).value(layout, true)?;
                    for pair in value["m_Container"].as_array().into_iter().flatten() {
                        let path_id = pair[1]["m_PathID"].as_i64().unwrap_or(0);
                        if path_id != 0 {
                            let file_id = pair[1]["m_FileID"].as_i64().unwrap_or(0) as i32;
                            resources.push(json!([
                                pair[0],
                                format!("{}:{path_id}", file.file_name(file_id))
                            ]));
                        }
                    }
                }
                TAG_MANAGER if tags.is_none() => {
                    let layout = builtin(object.class_id).ok_or("Missing engine layout")?;
                    tags = Some(
                        Reader::new(file.bytes(object), file).value(layout, true)?["tags"].clone(),
                    );
                }
                _ => {}
            }
        }
    }
    let scripts: Map<String, Value> = infos
        .iter()
        .map(|(key, info)| {
            (
                key.clone(),
                json!({"ns": info.namespace, "cls": info.class, "asm": info.assembly}),
            )
        })
        .collect();
    let count = objects.len();
    let resource_count = resources.len();
    let scene = json!({"objects": objects, "scripts": scripts, "resources": resources});
    write_atomic(
        &out.join("scene.json"),
        &serde_json::to_vec(&scene).map_err(|e| e.to_string())?,
    )?;
    let tags = json!({"tags": tags.ok_or("The game has no tag list.")?});
    write_atomic(
        &out.join("tags.json"),
        &serde_json::to_vec(&tags).map_err(|e| e.to_string())?,
    )?;
    Ok(Summary {
        objects: count,
        resources: resource_count,
        errors,
    })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|e| format!("{}: {e}", temp.display()))?;
    fs::rename(&temp, path).map_err(|e| format!("{}: {e}", path.display()))
}
