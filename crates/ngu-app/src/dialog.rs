//! The system's own folder chooser (the Windows Explorer dialog on Windows).

use std::path::PathBuf;

/// Asks the player for a folder, starting at `start` when it exists. `None` means they cancelled.
pub fn pick_folder(start: Option<PathBuf>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title("Choose the NGU Idle folder");
    if let Some(start) = start.filter(|path| path.is_dir()) {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_folder()
}
