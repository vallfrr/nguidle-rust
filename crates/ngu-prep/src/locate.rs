//! Finds an installed NGU Idle without writing anything.

use crate::dirs::Os;
use std::fs;
use std::path::{Path, PathBuf};

const GAME_FOLDER: &str = "NGU IDLE";

/// Library folders listed in a Steam `libraryfolders.vdf`.
pub fn vdf_libraries(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let mut quoted = line.split('"').skip(1).step_by(2);
            (quoted.next()? == "path").then(|| quoted.next())?
        })
        .map(|path| PathBuf::from(path.replace("\\\\", "\\")))
        .collect()
}

/// Steam installation roots worth checking on `os`.
pub fn steam_roots(os: Os, home: &Path, env: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    match os {
        Os::Windows => ["ProgramFiles(x86)", "ProgramFiles"]
            .iter()
            .filter_map(|name| env(name))
            .map(|base| PathBuf::from(base).join("Steam"))
            .collect(),
        Os::Linux => vec![
            home.join(".steam/steam"),
            home.join(".local/share/Steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            home.join("snap/steam/common/.local/share/Steam"),
        ],
        Os::MacOs => {
            let mut roots = vec![home.join("Library/Application Support/Steam")];
            let bottles = home.join("Library/Application Support/CrossOver/Bottles");
            for bottle in fs::read_dir(bottles).into_iter().flatten().flatten() {
                for steam in ["Program Files (x86)/Steam", "Program Files/Steam"] {
                    roots.push(bottle.path().join("drive_c").join(steam));
                }
            }
            roots
        }
    }
}

/// Game folders (the ones that contain `NGUIdle_Data`) found under `roots`,
/// without duplicates. More than one result means the player must choose.
pub fn find_games(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut libraries = Vec::new();
    for root in roots {
        libraries.push(root.clone());
        if let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            libraries.extend(vdf_libraries(&text));
        }
    }
    let mut found: Vec<PathBuf> = Vec::new();
    for library in libraries {
        let game = library.join("steamapps/common").join(GAME_FOLDER);
        if !game
            .join("NGUIdle_Data/Managed/Assembly-CSharp.dll")
            .is_file()
        {
            continue;
        }
        let game = game.canonicalize().unwrap_or(game);
        if !found.contains(&game) {
            found.push(game);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_library_paths() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"Lib\\\\Games\\\\Steam\"\n\t\t\"label\"\t\t\"\"\n\t}\n}";
        assert_eq!(vdf_libraries(vdf), vec![PathBuf::from("Lib\\Games\\Steam")]);
    }

    #[test]
    fn finds_a_game_in_a_second_library_once() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("steam");
        let other = dir.path().join("other");
        fs::create_dir_all(steam.join("steamapps")).unwrap();
        let game = other.join("steamapps/common/NGU IDLE/NGUIdle_Data/Managed");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("Assembly-CSharp.dll"), b"x").unwrap();
        let vdf = format!("\"path\" \"{}\"\n", other.display());
        fs::write(steam.join("steamapps/libraryfolders.vdf"), vdf).unwrap();
        let found = find_games(&[steam.clone(), steam]);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("NGU IDLE"));
    }

    #[test]
    fn reports_nothing_without_the_game() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_games(&[dir.path().to_path_buf()]).is_empty());
    }
}
