//! Folder browsing for the setup window, without a system dialog.

use std::path::{Path, PathBuf};

/// A folder being looked at, with its sub-folders.
pub struct Browser {
    pub dir: Option<PathBuf>,
    pub entries: Vec<PathBuf>,
}

/// The places to start from: the home folder first, then drives on Windows or `/` elsewhere.
pub fn roots(home: Option<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = home.into_iter().collect();
    if cfg!(windows) {
        out.extend(
            ('A'..='Z')
                .map(|c| PathBuf::from(format!("{c}:\\")))
                .filter(|p| p.is_dir()),
        );
    } else {
        out.push(PathBuf::from("/"));
    }
    out
}

impl Browser {
    /// The list of roots, no folder open.
    pub fn at_roots(home: Option<PathBuf>) -> Browser {
        Browser {
            dir: None,
            entries: roots(home),
        }
    }

    /// Open `dir` and list its readable sub-folders, hidden ones last.
    pub fn open(dir: &Path) -> Browser {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.path())
            .collect();
        let hidden = |p: &PathBuf| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        };
        entries.sort_by_key(|p| {
            (
                hidden(p),
                p.file_name().map(|n| n.to_string_lossy().to_lowercase()),
            )
        });
        Browser {
            dir: Some(dir.to_path_buf()),
            entries,
        }
    }

    /// The parent folder, or the list of roots when at the top.
    pub fn up(&self, home: Option<PathBuf>) -> Browser {
        match self.dir.as_deref().and_then(Path::parent) {
            Some(parent) => Browser::open(parent),
            None => Browser::at_roots(home),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_folders_only_with_hidden_last() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["beta", "Alpha", ".hidden"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        std::fs::write(dir.path().join("file.txt"), b"x").unwrap();
        let names: Vec<_> = Browser::open(dir.path())
            .entries
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Alpha", "beta", ".hidden"]);
    }

    #[test]
    fn going_up_from_a_folder_opens_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        let up = Browser::open(&inner).up(None);
        assert_eq!(up.dir.as_deref(), Some(dir.path()));
        assert!(Browser::at_roots(None).entries.len() >= 1);
    }
}
