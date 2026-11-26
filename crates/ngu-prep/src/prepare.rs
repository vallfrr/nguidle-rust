//! The two preparation jobs: the PC game data and the Vita `nguidle` folder.
//! The original game folder is only read.

use crate::{copy_game, data_dir, dirs, manifest, pack, unity, verify};
use std::fs;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// What a finished job tells the player.
#[derive(Debug, Default)]
pub struct Report {
    /// Things that did not work out but did not stop the job.
    pub warnings: Vec<String>,
    /// Files in the packed archive (Vita) or objects in the scene (PC).
    pub count: usize,
}

/// Version of the prepared data layout. A new version asks for a new preparation.
pub const PREPARED_FORMAT: u32 = 1;

/// Files the PC application needs in `data/`.
const PREPARED_FILES: [&str; 5] = [
    "scene.json",
    "tags.json",
    "Assembly-CSharp.dll",
    "audio/Fart.wav",
    "audio/LongFart.wav",
];

/// Tells whether the application folder is ready to play. The error says, in
/// words for the player, what is missing. Saves are never looked at.
pub fn check_prepared(root: &Path) -> std::result::Result<(), String> {
    let game = dirs::game_dir(root).join("NGUIdle_Data");
    if let Some(missing) = manifest().keys().find(|name| !game.join(name).is_file()) {
        return Err(format!("A game file is missing: {missing}."));
    }
    let data = dirs::prepared_dir(root);
    if let Some(missing) = PREPARED_FILES
        .iter()
        .find(|name| !data.join(name).is_file())
    {
        return Err(format!("A prepared file is missing: {missing}."));
    }
    match fs::read_to_string(data.join("format")) {
        Ok(text) if text.trim() == PREPARED_FORMAT.to_string() => Ok(()),
        _ => Err("The prepared files come from another version.".into()),
    }
}

/// Checks the folder holds the supported game build and returns its data folder.
pub fn verified_game(source: &Path) -> Result<PathBuf> {
    let data = data_dir(source)?;
    let problems = verify(&data, &manifest());
    if !problems.is_empty() {
        return Err(format!(
            "This is not the supported NGU Idle build (1.260). {}",
            problems
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        )
        .into());
    }
    Ok(data)
}

fn outside(game: &Path, output: &Path) -> Result<()> {
    let game = game.canonicalize()?;
    let parent = output
        .ancestors()
        .find(|p| p.is_dir())
        .ok_or("The output folder has no existing parent.")?;
    if parent.canonicalize()?.starts_with(&game) {
        return Err("Choose an output folder outside the game folder.".into());
    }
    Ok(())
}

fn replace(staged: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        let old = target.with_extension("old");
        if old.exists() {
            fs::remove_dir_all(&old)?;
        }
        fs::rename(target, &old)?;
        fs::rename(staged, target)?;
        fs::remove_dir_all(old)?;
    } else {
        fs::rename(staged, target)?;
    }
    Ok(())
}

fn fresh(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(path.to_path_buf())
}

/// Prepares the PC data in `root`: a verified copy of the game in `game/`,
/// and the prepared files in `data/`, made from that copy. Saves in `saves/`
/// are never touched.
pub fn prepare_pc(source: &Path, root: &Path, progress: &mut dyn FnMut(&str)) -> Result<Report> {
    let source = source.canonicalize()?;
    progress("Checking the game files...");
    verified_game(&source)?;
    outside(&source, root)?;
    fs::create_dir_all(root)?;
    let game = dirs::game_dir(root);
    // A copy that is incomplete or damaged is replaced, never reused.
    if game.exists() && verified_game(&game).is_err() && game.canonicalize()? != source {
        fs::remove_dir_all(&game)?;
    }
    if !game.exists() {
        progress("Copying the game...");
        copy_game(&source, &game, &mut |_| {})?;
    }
    progress("Preparing the game data...");
    let staged = fresh(&root.join(".data.partial"))?;
    let summary = unity::export::export(&data_dir(&game)?, &staged)?;
    if !summary.errors.is_empty() {
        return Err(format!("Preparation failed: {}", summary.errors.join("; ")).into());
    }
    fs::copy(
        data_dir(&game)?.join("Managed/Assembly-CSharp.dll"),
        staged.join("Assembly-CSharp.dll"),
    )?;
    fs::write(staged.join("format"), PREPARED_FORMAT.to_string())?;
    replace(&staged, &dirs::prepared_dir(root))?;
    Ok(Report {
        warnings: vec![],
        count: summary.objects,
    })
}

/// Prepares the Vita data: writes `output/nguidle/ngu.pak`.
pub fn prepare_vita(
    source: &Path,
    output: &Path,
    progress: &mut dyn FnMut(&str),
) -> Result<Report> {
    let source = source.canonicalize()?;
    progress("Checking the game files...");
    let data = verified_game(&source)?;
    outside(&source, output)?;
    let target = output.join("nguidle");
    if target.exists() {
        return Err(
            "The output folder already contains a folder named nguidle. Remove or rename it first."
                .into(),
        );
    }
    fs::create_dir_all(output)?;
    progress("Preparing the game data...");
    let work = fresh(&output.join(".nguidle-work"))?;
    let result = (|| -> Result<Report> {
        let summary = unity::export::export(&data, &work)?;
        if !summary.errors.is_empty() {
            return Err(format!("Preparation failed: {}", summary.errors.join("; ")).into());
        }
        progress("Building the archive...");
        let staged = fresh(&output.join(".nguidle.partial"))?;
        let count = pack::pack(
            &work,
            &data.join("Managed/Assembly-CSharp.dll"),
            &staged.join("ngu.pak"),
        )?;
        fs::rename(&staged, &target)?;
        Ok(Report {
            warnings: vec![],
            count,
        })
    })();
    let _ = fs::remove_dir_all(&work);
    let _ = fs::remove_dir_all(output.join(".nguidle.partial"));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_folder_that_is_not_the_game() {
        let dir = tempfile::tempdir().unwrap();
        assert!(prepare_vita(dir.path(), &dir.path().join("out"), &mut |_| {}).is_err());
    }

    #[test]
    fn rejects_a_game_that_does_not_match_the_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let managed = dir.path().join("NGUIdle_Data/Managed");
        fs::create_dir_all(&managed).unwrap();
        fs::write(managed.join("Assembly-CSharp.dll"), b"modified").unwrap();
        let error = verified_game(dir.path()).unwrap_err().to_string();
        assert!(error.contains("supported NGU Idle build"), "{error}");
    }

    #[test]
    fn a_complete_folder_is_ready_until_something_goes_missing() {
        let dir = tempfile::tempdir().unwrap();
        let touch = |path: PathBuf| {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"x").unwrap();
        };
        for name in manifest().keys() {
            touch(dirs::game_dir(dir.path()).join("NGUIdle_Data").join(name));
        }
        for name in PREPARED_FILES {
            touch(dirs::prepared_dir(dir.path()).join(name));
        }
        touch(dirs::saves_dir(dir.path()).join("slot.txt"));
        assert!(check_prepared(dir.path())
            .unwrap_err()
            .contains("another version"));
        fs::write(
            dirs::prepared_dir(dir.path()).join("format"),
            PREPARED_FORMAT.to_string(),
        )
        .unwrap();
        assert_eq!(check_prepared(dir.path()), Ok(()));
        fs::remove_file(dirs::prepared_dir(dir.path()).join("audio/Fart.wav")).unwrap();
        assert!(check_prepared(dir.path()).unwrap_err().contains("Fart.wav"));
    }

    #[test]
    fn an_empty_folder_is_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        let reason = check_prepared(dir.path()).unwrap_err();
        assert!(reason.contains("missing"), "{reason}");
    }

    #[test]
    fn refuses_an_output_inside_the_game() {
        let dir = tempfile::tempdir().unwrap();
        assert!(outside(dir.path(), &dir.path().join("sub/out")).is_err());
    }
}
