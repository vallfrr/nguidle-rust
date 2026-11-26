//! Where the application keeps its files, per operating system.

use std::path::{Path, PathBuf};

/// Folder name used under every operating system (decision D9).
pub const APP_NAME: &str = "NGUIdleRustEngine";

/// Operating systems the applications support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    MacOs,
}

impl Os {
    /// The system this program was built for.
    pub fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

/// Root of the application data. `env` reads an environment variable.
pub fn data_root(os: Os, home: &Path, env: impl Fn(&str) -> Option<String>) -> PathBuf {
    let non_empty = |name: &str| env(name).filter(|value| !value.is_empty());
    match os {
        Os::Windows => non_empty("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"))
            .join(APP_NAME),
        Os::Linux => non_empty("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"))
            .join(APP_NAME),
        Os::MacOs => home.join("Library/Application Support").join(APP_NAME),
    }
}

/// The copy of the original game.
pub fn game_dir(root: &Path) -> PathBuf {
    root.join("game")
}

/// Prepared files, regenerable from the game copy.
pub fn prepared_dir(root: &Path) -> PathBuf {
    root.join("data")
}

/// Player saves. Never deleted by a new preparation.
pub fn saves_dir(root: &Path) -> PathBuf {
    root.join("saves")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_each_system_convention() {
        let home = Path::new("/home/p");
        let none = |_: &str| None;
        assert_eq!(
            data_root(Os::Linux, home, none),
            Path::new("/home/p/.local/share/NGUIdleRustEngine")
        );
        let xdg = |name: &str| (name == "XDG_DATA_HOME").then(|| "/x".to_string());
        assert_eq!(
            data_root(Os::Linux, home, xdg),
            Path::new("/x/NGUIdleRustEngine")
        );
        assert_eq!(
            data_root(Os::MacOs, home, none),
            Path::new("/home/p/Library/Application Support/NGUIdleRustEngine")
        );
        let local = |name: &str| (name == "LOCALAPPDATA").then(|| "/L".to_string());
        assert_eq!(
            data_root(Os::Windows, home, local),
            Path::new("/L/NGUIdleRustEngine")
        );
    }

    #[test]
    fn ignores_a_relative_data_home() {
        let relative = |name: &str| (name == "XDG_DATA_HOME").then(|| "rel".to_string());
        assert_eq!(
            data_root(Os::Linux, Path::new("/h"), relative),
            Path::new("/h/.local/share/NGUIdleRustEngine")
        );
    }
}
