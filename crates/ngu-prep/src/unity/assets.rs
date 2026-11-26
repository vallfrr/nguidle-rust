//! Exports textures, fonts, text assets and the resource tables.

use super::image::{decode, png_bytes};
use super::typetree::{builtin, Reader};
use super::SerializedFile;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

type Result<T> = std::result::Result<T, String>;

const TEXTURE2D: i32 = 28;
const TEXT_ASSET: i32 = 49;
const FONT: i32 = 128;
const AUDIO_CLIP: i32 = 83;

fn file_key(key: &str) -> String {
    key.replace(':', "_")
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes textures, fonts and text assets under `out` and returns their records.
/// Textures are skipped when `textures` is false.
pub fn export(
    files: &[SerializedFile],
    data_dir: &Path,
    out: &Path,
    textures: bool,
) -> Result<(Map<String, Value>, Vec<String>)> {
    for folder in ["tex", "fonts", "text", "audio"] {
        fs::create_dir_all(out.join(folder)).map_err(|e| e.to_string())?;
    }
    let mut records = Map::new();
    let mut errors = Vec::new();
    for file in files {
        for object in &file.objects {
            if ![TEXTURE2D, TEXT_ASSET, FONT, AUDIO_CLIP].contains(&object.class_id) {
                continue;
            }
            let key = format!("{}:{}", file.name, object.path_id);
            let layout = builtin(object.class_id).ok_or("Missing engine layout")?;
            let mut reader = Reader::new(file.bytes(object), file);
            let value = match reader.value(layout, true) {
                Ok(value) => value,
                Err(e) => {
                    errors.push(format!("{key}: {e}"));
                    continue;
                }
            };
            let name = value["m_Name"].clone();
            let record = match object.class_id {
                TEXTURE2D => {
                    let (w, h) = (
                        value["m_Width"].as_i64().unwrap_or(0),
                        value["m_Height"].as_i64().unwrap_or(0),
                    );
                    let format = value["m_TextureFormat"].as_i64().unwrap_or(0) as i32;
                    if textures && w > 0 && h > 0 {
                        let path = out.join("tex").join(format!("{}.png", file_key(&key)));
                        if !path.exists() {
                            let result = texture_bytes(&reader, &value, data_dir)
                                .and_then(|bytes| decode(format, w as usize, h as usize, &bytes))
                                .and_then(|picture| png_bytes(&picture))
                                .and_then(|png| write(&path, &png));
                            if let Err(e) = result {
                                errors.push(format!("{key}: texture: {e}"));
                            }
                        }
                    }
                    json!({"t": "Texture2D", "name": name, "w": w, "h": h, "fmt": format})
                }
                FONT => {
                    let mut record = json!({"t": "Font", "name": name});
                    if let Some(data) = reader.blob("m_FontData").filter(|d| !d.is_empty()) {
                        let file_name = format!("{}.ttf", file_key(&key));
                        write(&out.join("fonts").join(&file_name), data)?;
                        record["file"] = file_name.into();
                    }
                    record["d"] = json!({});
                    record
                }
                AUDIO_CLIP => json!({"t": "AudioClip", "name": name}),
                _ => {
                    write(
                        &out.join("text").join(format!("{}.bin", file_key(&key))),
                        reader.blob("m_Script").unwrap_or(&[]),
                    )?;
                    json!({"t": "TextAsset", "name": name})
                }
            };
            records.insert(key, record);
        }
    }
    Ok((records, errors))
}

/// Pixel data of a texture, inline or in a streamed resource file.
fn texture_bytes(reader: &Reader, value: &Value, data_dir: &Path) -> Result<Vec<u8>> {
    if let Some(inline) = reader.blob("image data").filter(|d| !d.is_empty()) {
        return Ok(inline.to_vec());
    }
    let stream = &value["m_StreamData"];
    let (offset, size) = (
        stream["offset"].as_u64().unwrap_or(0) as usize,
        stream["size"].as_u64().unwrap_or(0) as usize,
    );
    let path = stream["path"].as_str().unwrap_or("");
    if size == 0 || path.is_empty() {
        return Err("Texture has no image data.".into());
    }
    use std::io::{Read, Seek, SeekFrom};
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let mut file = fs::File::open(data_dir.join(name)).map_err(|e| format!("{path}: {e}"))?;
    file.seek(SeekFrom::Start(offset as u64))
        .map_err(|e| format!("{path}: {e}"))?;
    let mut bytes = vec![0; size];
    file.read_exact(&mut bytes)
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(bytes)
}
