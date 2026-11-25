//! Host tuning: how the engine adapts to slow or limited storage and processors, chosen by the host.
use crate::save_slots::{atomic_write, record_autosave_slot};
use crate::Vm;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{fs, path::Path};

/// Behaviors a host with little memory, slow storage and a slow processor can switch on. All are off by
/// default, and none changes what the player sees or what a save contains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostTuning {
    /// Write autosaves on a worker thread instead of inside the frame.
    pub background_autosave: bool,
    /// The save folder has no hard links and only this process writes to it.
    pub single_writer_storage: bool,
    /// Parse the startup fonts on a worker thread while the scene loads.
    pub startup_fonts_worker: bool,
    /// Measure the layout time of each frame, for the host's trace (`ngu_layout_us`).
    pub time_layout: bool,
}

impl HostTuning {
    /// The bit values used by the C ABI function `ngu_set_host_tuning`.
    pub const BACKGROUND_AUTOSAVE: u32 = 1;
    /// See [`HostTuning::single_writer_storage`].
    pub const SINGLE_WRITER_STORAGE: u32 = 2;
    /// See [`HostTuning::startup_fonts_worker`].
    pub const STARTUP_FONTS_WORKER: u32 = 4;
    /// See [`HostTuning::time_layout`].
    pub const TIME_LAYOUT: u32 = 8;

    /// Build from the C ABI bit set.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            background_autosave: bits & Self::BACKGROUND_AUTOSAVE != 0,
            single_writer_storage: bits & Self::SINGLE_WRITER_STORAGE != 0,
            startup_fonts_worker: bits & Self::STARTUP_FONTS_WORKER != 0,
            time_layout: bits & Self::TIME_LAYOUT != 0,
        }
    }

    fn bits(self) -> u32 {
        [
            (self.background_autosave, Self::BACKGROUND_AUTOSAVE),
            (self.single_writer_storage, Self::SINGLE_WRITER_STORAGE),
            (self.startup_fonts_worker, Self::STARTUP_FONTS_WORKER),
            (self.time_layout, Self::TIME_LAYOUT),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(0, |bits, (_, bit)| bits | bit)
    }
}

static TUNING: AtomicU32 = AtomicU32::new(0);

/// Choose the tuning before opening a session. It applies to the whole process.
pub fn set_host_tuning(tuning: HostTuning) {
    TUNING.store(tuning.bits(), Ordering::Relaxed);
}

/// The tuning in force.
pub fn host_tuning() -> HostTuning {
    HostTuning::from_bits(TUNING.load(Ordering::Relaxed))
}

pub(crate) fn save_process_id() -> u32 {
    #[cfg(not(target_family = "wasm"))]
    if !host_tuning().single_writer_storage {
        return std::process::id();
    }
    0
}

pub(crate) fn publish_new_save(temp: &Path, path: &Path) -> std::io::Result<()> {
    if host_tuning().single_writer_storage {
        // No hard links on this storage and the game has one writer.
        if path.try_exists()? {
            return Err(std::io::ErrorKind::AlreadyExists.into());
        }
        return fs::rename(temp, path);
    }
    // Publishing with a link fails atomically if another process filled the slot.
    fs::hard_link(temp, path)
}

enum AutosaveWork {
    Write(PathBuf, String, Option<usize>),
    Flush(std::sync::mpsc::SyncSender<()>),
}

pub(crate) struct BackgroundAutosave(std::sync::mpsc::SyncSender<AutosaveWork>);

impl BackgroundAutosave {
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
                                    crate::elog!("Could not write the autosave: {error}");
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

pub(crate) fn flush_pending_autosave(vm: &Vm) {
    if let Some(writer) = &vm.eng.autosave_writer {
        writer.flush();
    }
}

pub(crate) fn before_managed_read(vm: &Vm, path: &Path) {
    if is_autosave_path(vm, path) {
        flush_pending_autosave(vm);
    }
}

fn is_autosave_path(vm: &Vm, path: &Path) -> bool {
    path.parent() == Some(Path::new(&vm.eng.save_dir))
        && matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("NGUSaveSteam.txt" | "NGUSave.txt")
        )
}

pub(crate) fn defer_autosave(vm: &mut Vm, filename: &str, data: String) -> Option<String> {
    if !host_tuning().background_autosave {
        return Some(data);
    }
    let data = if vm.eng.save_slots.has_home()
        && !data.is_empty()
        && is_autosave_path(vm, Path::new(filename))
    {
        if vm.eng.autosave_writer.is_none() {
            let profile = std::env::var_os("NGU_SAVE_DIR").is_some();
            vm.eng.autosave_writer = BackgroundAutosave::new(profile).ok();
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
