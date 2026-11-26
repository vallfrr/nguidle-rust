//! Puts the downloaded program in the application folder and adds a desktop shortcut.
//! The file the player downloads is only the installer: the same program, copied.

use std::path::{Path, PathBuf};

#[cfg(not(windows))]
const CAT: &[u8] = include_bytes!("../../../assets/ui/cat.png");

/// What to install.
pub struct Spec<'a> {
    /// Application folder that receives the program.
    pub root: &'a Path,
    /// File name of the installed program, without extension.
    pub stem: &'a str,
    /// Name shown under the desktop shortcut.
    pub label: &'a str,
    /// A file whose icon the shortcut uses (the game's own program), if there is one.
    pub icon: Option<PathBuf>,
}

/// Where the installed program lives.
pub fn installed_path(root: &Path, stem: &str) -> PathBuf {
    root.join(if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_owned()
    })
}

/// Whether the running program is the installed one.
pub fn is_installed(root: &Path, stem: &str) -> bool {
    let (Ok(me), Ok(installed)) = (
        std::env::current_exe().and_then(|p| p.canonicalize()),
        installed_path(root, stem).canonicalize(),
    ) else {
        return false;
    };
    me == installed
}

/// Copies the running program into the application folder (replacing an older copy) and makes
/// the shortcut. Returns the program to start: the installed one, or the running one when
/// installing is not possible (a developer folder, or a copy that is in use).
pub fn install(spec: &Spec) -> PathBuf {
    let me = std::env::current_exe().unwrap_or_default();
    // A folder chosen by hand is a developer's test setup: leave the desktop alone.
    if std::env::var_os("NGU_APP_DIR").is_some() || is_installed(spec.root, spec.stem) {
        return me;
    }
    let target = installed_path(spec.root, spec.stem);
    let staged = target.with_extension("new");
    let copied = std::fs::create_dir_all(spec.root)
        .and_then(|_| std::fs::copy(&me, &staged))
        .and_then(|_| std::fs::rename(&staged, &target));
    if copied.is_err() {
        let _ = std::fs::remove_file(&staged);
        // An installed copy that is running cannot be replaced; it is still good to use.
        return if target.is_file() { target } else { me };
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755));
    }
    shortcut(spec, &target);
    target
}

#[cfg(windows)]
fn shortcut(spec: &Spec, target: &Path) {
    use std::os::windows::process::CommandExt;
    const SCRIPT: &str = "$d=[Environment]::GetFolderPath('Desktop'); \
        $s=(New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $d ($env:NGU_LABEL+'.lnk'))); \
        $s.TargetPath=$env:NGU_TARGET; $s.WorkingDirectory=Split-Path $env:NGU_TARGET; \
        $s.IconLocation=$env:NGU_ICON+',0'; $s.Description=$env:NGU_LABEL; $s.Save()";
    let icon = spec
        .icon
        .as_deref()
        .filter(|p| p.is_file())
        .unwrap_or(target);
    let _ = std::process::Command::new("powershell")
        .creation_flags(0x0800_0000) // no console window flashing
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("NGU_LABEL", spec.label)
        .env("NGU_TARGET", target)
        .env("NGU_ICON", icon)
        .status();
}

#[cfg(all(unix, not(target_os = "macos")))]
fn shortcut(spec: &Spec, target: &Path) {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let icon = spec.root.join("icon.png");
    let _ = std::fs::write(&icon, CAT);
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec=\"{}\"\nPath={}\nIcon={}\nTerminal=false\nCategories=Game;\n",
        spec.label,
        target.display(),
        spec.root.display(),
        icon.display()
    );
    let file = format!("{}.desktop", spec.stem);
    let menu = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"))
        .join("applications");
    for dir in [menu, home.join("Desktop")] {
        if dir.is_dir() || dir.ends_with("applications") {
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join(&file);
            if std::fs::write(&path, &entry).is_ok() {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
            }
        }
    }
}

// macOS: the .app bundle is already the thing the player opens; only the copy is made.
#[cfg(target_os = "macos")]
fn shortcut(_spec: &Spec, _target: &Path) {
    let _ = CAT;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_developer_folder_is_never_installed_into() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("NGU_APP_DIR", dir.path());
        let spec = Spec {
            root: dir.path(),
            stem: "demo",
            label: "Demo",
            icon: None,
        };
        let started = install(&spec);
        std::env::remove_var("NGU_APP_DIR");
        assert_eq!(started, std::env::current_exe().unwrap());
        assert!(!installed_path(dir.path(), "demo").exists());
    }

    #[test]
    fn the_installed_program_is_named_for_the_system() {
        let path = installed_path(Path::new("root"), "demo");
        assert_eq!(path.extension().is_some(), cfg!(windows));
    }

    #[test]
    fn the_running_test_program_is_not_the_installed_one() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_installed(dir.path(), "demo"));
    }
}
