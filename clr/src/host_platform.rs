//! Host-specific filesystem behavior and background save scheduling.
#[cfg(target_os = "vita")]
use crate::save_slots::{atomic_write, record_autosave_slot};
use crate::Vm;
#[cfg(target_os = "vita")]
use std::path::PathBuf;
use std::{fs, path::Path};

#[cfg(not(any(target_family = "wasm", target_os = "vita")))]
pub(crate) fn save_process_id() -> u32 {
    std::process::id()
}

#[cfg(any(target_family = "wasm", target_os = "vita"))]
pub(crate) fn save_process_id() -> u32 {
    0
}

#[cfg(not(target_os = "vita"))]
pub(crate) fn publish_new_save(temp: &Path, path: &Path) -> std::io::Result<()> {
    // Publishing with a link fails atomically if another process filled the slot.
    fs::hard_link(temp, path)
}

#[cfg(target_os = "vita")]
pub(crate) fn publish_new_save(temp: &Path, path: &Path) -> std::io::Result<()> {
    // The Vita's local volume has no hard links and the game has one writer.
    if path.try_exists()? {
        return Err(std::io::ErrorKind::AlreadyExists.into());
    }
    fs::rename(temp, path)
}

#[cfg(target_os = "vita")]
enum AutosaveWork {
    Write(PathBuf, String, Option<usize>),
    Flush(std::sync::mpsc::SyncSender<()>),
}

#[cfg(target_os = "vita")]
pub(crate) struct VitaAutosave(std::sync::mpsc::SyncSender<AutosaveWork>);

#[cfg(target_os = "vita")]
impl VitaAutosave {
    fn new(profile: bool) -> std::io::Result<Self> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(2);
        std::thread::Builder::new()
            .name("ngu-autosave".into())
            .stack_size(128 * 1024)
            .spawn(move || {
                while let Ok(work) = receiver.recv() {
                    match work {
                        AutosaveWork::Write(path, data, slot) => {
                            let start = profile.then(std::time::Instant::now);
                            match atomic_write(&path, data.as_bytes()) {
                                Ok(()) => {
                                    if let (Some(dir), Some(slot)) = (path.parent(), slot) {
                                        record_autosave_slot(dir, slot, data.as_bytes());
                                    }
                                    if let Some(start) = start {
                                        crate::elog!(
                                            "[autosave] {} bytes written in {:?}",
                                            data.len(),
                                            start.elapsed()
                                        );
                                    }
                                }
                                Err(error) => {
                                    crate::elog!("Could not write the Vita autosave: {error}");
                                }
                            }
                        }
                        AutosaveWork::Flush(reply) => {
                            let _ = reply.send(());
                        }
                    }
                }
            })?;
        Ok(Self(sender))
    }

    fn flush(&self) {
        let (reply, complete) = std::sync::mpsc::sync_channel(0);
        if self.0.send(AutosaveWork::Flush(reply)).is_ok() {
            let _ = complete.recv();
        }
    }
}

pub(crate) fn flush_pending_autosave(_vm: &Vm) {
    #[cfg(target_os = "vita")]
    if let Some(writer) = &_vm.eng.autosave_writer {
        writer.flush();
    }
}

pub(crate) fn before_managed_read(vm: &Vm, path: &Path) {
    #[cfg(target_os = "vita")]
    if is_autosave_path(vm, path) {
        flush_pending_autosave(vm);
    }
    let _ = (vm, path);
}

#[cfg(target_os = "vita")]
fn is_autosave_path(vm: &Vm, path: &Path) -> bool {
    path.parent() == Some(Path::new(&vm.eng.save_dir))
        && matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("NGUSaveSteam.txt" | "NGUSave.txt")
        )
}

#[cfg(target_os = "vita")]
pub(crate) fn defer_autosave(vm: &mut Vm, filename: &str, data: String) -> Option<String> {
    let data = if vm.eng.save_slots.has_home()
        && !data.is_empty()
        && is_autosave_path(vm, Path::new(filename))
    {
        if vm.eng.autosave_writer.is_none() {
            let profile = std::env::var_os("NGU_VITA_SAVE_DIR").is_some();
            vm.eng.autosave_writer = VitaAutosave::new(profile).ok();
        }
        if let Some(writer) = &vm.eng.autosave_writer {
            let job = AutosaveWork::Write(
                PathBuf::from(filename),
                data,
                vm.eng.save_slots.active_slot(),
            );
            match writer.0.send(job) {
                Ok(()) => return None,
                Err(std::sync::mpsc::SendError(AutosaveWork::Write(_, data, _))) => data,
                Err(_) => unreachable!(),
            }
        } else {
            data
        }
    } else {
        data
    };
    Some(data)
}

#[cfg(not(target_os = "vita"))]
pub(crate) fn defer_autosave(_vm: &mut Vm, _filename: &str, data: String) -> Option<String> {
    Some(data)
}
