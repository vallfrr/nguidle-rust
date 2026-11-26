//! Writes the single-file archive (`ngu.pak`) the Vita reads.
//!
//! Layout: `NGUPAK1\0`, file count (u32), then per file a u16 name length, the
//! name, the data offset and the size (u64 each), then all file data in order.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn collect(root: &Path, folder: &Path, files: &mut BTreeMap<String, PathBuf>) -> io::Result<()> {
    if !folder.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, files)?;
        } else if path.is_file() {
            let name: Vec<_> = path
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.insert(name.join("/"), path);
        }
    }
    Ok(())
}

/// Packs the prepared data in `assets` and the game's `Assembly-CSharp.dll`
/// into `output`, and returns the number of files. The archive appears at
/// `output` only when complete.
pub fn pack(assets: &Path, dll: &Path, output: &Path) -> Result<usize> {
    let mut files = BTreeMap::new();
    files.insert("Assembly-CSharp.dll".to_owned(), dll.to_path_buf());
    files.insert("scene.json".to_owned(), assets.join("scene.json"));
    files.insert("tags.json".to_owned(), assets.join("tags.json"));
    for folder in ["audio", "fonts", "tex", "text"] {
        collect(assets, &assets.join(folder), &mut files)?;
    }
    let mut index = Vec::new();
    for (name, path) in &files {
        index.push((
            name.as_bytes(),
            path,
            fs::metadata(path)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .len(),
        ));
    }
    let mut offset = 12
        + index
            .iter()
            .map(|(name, _, _)| 18 + name.len() as u64)
            .sum::<u64>();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let staging = output.with_extension("pending");
    let mut out = BufWriter::new(File::create(&staging)?);
    out.write_all(b"NGUPAK1\0")?;
    out.write_all(&(index.len() as u32).to_le_bytes())?;
    for (name, _, size) in &index {
        out.write_all(&(name.len() as u16).to_le_bytes())?;
        out.write_all(name)?;
        out.write_all(&offset.to_le_bytes())?;
        out.write_all(&size.to_le_bytes())?;
        offset += size;
    }
    for (_, path, _) in &index {
        io::copy(&mut File::open(path)?, &mut out)?;
    }
    out.flush()?;
    drop(out);
    fs::rename(&staging, output)?;
    Ok(index.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_lists_files_in_name_order_with_matching_offsets() {
        let dir = tempfile::tempdir().unwrap();
        let assets = dir.path().join("assets");
        fs::create_dir_all(assets.join("tex")).unwrap();
        fs::write(assets.join("scene.json"), b"{}").unwrap();
        fs::write(assets.join("tags.json"), b"[]").unwrap();
        fs::write(assets.join("tex/b.png"), b"BB").unwrap();
        fs::write(dir.path().join("a.dll"), b"DLL").unwrap();
        let pak = dir.path().join("out/ngu.pak");
        assert_eq!(pack(&assets, &dir.path().join("a.dll"), &pak).unwrap(), 4);
        let bytes = fs::read(&pak).unwrap();
        assert_eq!(&bytes[..8], b"NGUPAK1\0");
        let mut at = 12;
        let mut names = Vec::new();
        for _ in 0..4 {
            let length = u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
            let name = String::from_utf8(bytes[at + 2..at + 2 + length].to_vec()).unwrap();
            let rest = at + 2 + length;
            let offset = u64::from_le_bytes(bytes[rest..rest + 8].try_into().unwrap()) as usize;
            let size = u64::from_le_bytes(bytes[rest + 8..rest + 16].try_into().unwrap()) as usize;
            names.push((name, bytes[offset..offset + size].to_vec()));
            at = rest + 16;
        }
        let order: Vec<_> = names.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            order,
            [
                "Assembly-CSharp.dll",
                "scene.json",
                "tags.json",
                "tex/b.png"
            ]
        );
        assert_eq!(names[3].1, b"BB");
        assert!(!pak.with_extension("pending").exists());
    }
}
