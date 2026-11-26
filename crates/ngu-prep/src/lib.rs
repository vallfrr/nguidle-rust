//! Preparation shared by the PC application and the Vita preparation
//! application: find the game, verify it against the SHA-256 manifest and
//! copy it. The original game folder is only ever read.

pub mod dirs;
pub mod locate;
pub mod unity;

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const MANIFEST: &str = include_str!("../../../tools/game-data.sha256.json");

/// Relative file name to expected SHA-256 for a supported game build.
pub fn manifest() -> BTreeMap<String, String> {
    let json: serde_json::Value = serde_json::from_str(MANIFEST).expect("embedded manifest");
    serde_json::from_value(json["files"].clone()).expect("embedded manifest files")
}

/// The `NGUIdle_Data` folder for a game folder, or for the data folder itself.
pub fn data_dir(selected: &Path) -> Result<PathBuf> {
    let nested = selected.join("NGUIdle_Data");
    let data = if nested.is_dir() {
        nested
    } else {
        selected.to_path_buf()
    };
    if !data.join("Managed/Assembly-CSharp.dll").is_file() {
        return Err("Select the NGU Idle game folder.".into());
    }
    Ok(data.canonicalize()?)
}

/// Problems found in `data` compared with `expected`; empty means intact.
pub fn verify(data: &Path, expected: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, hash) in expected {
        let path = data.join(name);
        match File::open(&path).and_then(|file| sha256(file)) {
            Ok(actual) if actual == *hash => {}
            Ok(_) => problems.push(format!("Wrong SHA-256: {name}")),
            Err(_) => problems.push(format!("Missing: {name}")),
        }
    }
    problems
}

fn sha256(mut file: impl Read) -> io::Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 128 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            return Ok(format!("{:x}", digest.finalize()));
        }
        digest.update(&buffer[..count]);
    }
}

/// Copies the whole game folder to `destination`, which must not exist and
/// must not be inside the source. The copy appears at `destination` only
/// when complete, so an interrupted copy leaves nothing behind.
pub fn copy_game(source: &Path, destination: &Path, progress: &mut dyn FnMut(usize)) -> Result<()> {
    let source = source.canonicalize()?;
    if destination.exists() {
        return Err("The destination already exists.".into());
    }
    let parent = destination
        .parent()
        .ok_or("The destination has no parent folder.")?;
    fs::create_dir_all(parent)?;
    let parent = parent.canonicalize()?;
    if parent.starts_with(&source) {
        return Err("Choose a destination outside the game folder.".into());
    }
    let name = destination
        .file_name()
        .filter(|name| {
            Path::new(name)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        })
        .ok_or("Invalid destination name.")?;
    let staging = parent.join(format!(".{}.partial", name.to_string_lossy()));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    let mut count = 0;
    let done = copy_tree(&source, &staging, &mut count, progress);
    if let Err(error) = done {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    fs::rename(&staging, parent.join(name))?;
    Ok(())
}

fn copy_tree(
    from: &Path,
    to: &Path,
    count: &mut usize,
    progress: &mut dyn FnMut(usize),
) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target, count, progress)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target)?;
            *count += 1;
            progress(*count);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(dir: &Path) -> PathBuf {
        let root = dir.join("NGU IDLE");
        fs::create_dir_all(root.join("NGUIdle_Data/Managed")).unwrap();
        fs::write(
            root.join("NGUIdle_Data/Managed/Assembly-CSharp.dll"),
            b"abc",
        )
        .unwrap();
        fs::write(root.join("NGU IDLE.exe"), b"exe").unwrap();
        root
    }

    #[test]
    fn embedded_manifest_lists_the_assembly() {
        assert!(manifest().contains_key("Managed/Assembly-CSharp.dll"));
    }

    #[test]
    fn verify_reports_wrong_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let data = data_dir(&game(dir.path())).unwrap();
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let mut expected = BTreeMap::new();
        expected.insert("Managed/Assembly-CSharp.dll".to_string(), abc.to_string());
        assert!(verify(&data, &expected).is_empty());
        expected.insert("Managed/Assembly-CSharp.dll".to_string(), "00".to_string());
        expected.insert("gone.dll".to_string(), abc.to_string());
        assert_eq!(verify(&data, &expected).len(), 2);
    }

    #[test]
    fn copy_is_complete_and_refuses_unsafe_targets() {
        let dir = tempfile::tempdir().unwrap();
        let source = game(dir.path());
        let target = dir.path().join("app/game");
        let mut seen = 0;
        copy_game(&source, &target, &mut |n| seen = n).unwrap();
        assert_eq!(seen, 2);
        assert!(target
            .join("NGUIdle_Data/Managed/Assembly-CSharp.dll")
            .is_file());
        assert!(copy_game(&source, &target, &mut |_| {}).is_err());
        assert!(copy_game(&source, &source.join("inside"), &mut |_| {}).is_err());
        assert!(!source.join("inside").exists());
    }

    #[test]
    fn data_dir_rejects_other_folders() {
        let dir = tempfile::tempdir().unwrap();
        assert!(data_dir(dir.path()).is_err());
    }
}
