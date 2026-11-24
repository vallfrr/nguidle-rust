use super::*;

fn member<'a>(node: &'a N, field: &str) -> Option<&'a N> {
    let N::Obj(obj) = node else { return None };
    obj.meta
        .names
        .iter()
        .position(|n| n == field)
        .and_then(|i| obj.vals.get(i))
}

pub(super) fn meta(save: &str) -> Result<Metadata, String> {
    let outer = nrbf::parse(&b64_dec(save)).map_err(|_| "Unsupported save format.")?;
    let (Some(N::Str(_, inner)), Some(N::Str(_, checksum))) = (
        member(&outer.root, "playerData"),
        member(&outer.root, "checksum"),
    ) else {
        return Err("Missing save data or checksum.".into());
    };
    let bytes = b64_dec(inner);
    if b64_enc(&Md5::digest(&bytes)) != checksum.as_ref() {
        return Err("Save checksum does not match.".into());
    }
    let game = nrbf::parse(&bytes).map_err(|_| "Could not read the saved character.")?;
    if !matches!(&game.root, N::Obj(obj) if obj.meta.name == "PlayerData") {
        return Err("This file does not contain a saved character.".into());
    }
    let resolve = |node: &N| match node {
        N::Ref(i) => game.index.get(i).cloned(),
        other => Some(other.clone()),
    };
    let player = match member(&game.root, "playerName").and_then(resolve) {
        Some(N::Str(_, n)) => clean_label(&n, 34),
        _ => "".into(),
    };
    let time = member(&game.root, "totalPlaytime").and_then(resolve);
    let seconds = match time.as_ref().and_then(|t| member(t, "totalseconds")) {
        Some(N::P(Pv::F64(seconds))) if seconds.is_finite() && *seconds >= 0.0 => Some(*seconds),
        _ => None,
    };
    Ok(Metadata { player, seconds })
}

pub(super) fn clean_label(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

pub(super) fn read_save(path: &Path) -> Result<(String, Metadata), String> {
    let mut data = String::new();
    File::open(path)
        .and_then(|f| f.take(MAX_SAVE_BYTES + 1).read_to_string(&mut data))
        .map_err(|e| format!("Could not read the file: {e}"))?;
    if data.len() as u64 > MAX_SAVE_BYTES {
        return Err("The save file is too large.".into());
    }
    let metadata = meta(&data)?;
    Ok((data, metadata))
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_save_file(path, bytes, true)
}

pub(super) fn write_save_file(path: &Path, bytes: &[u8], replace: bool) -> std::io::Result<()> {
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let process = crate::host_platform::save_process_id();
    let temp = dir.join(format!(".save-{process}-{stamp}.tmp"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let written = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    let result = written.and_then(|_| {
        if replace {
            fs::rename(&temp, path)
        } else {
            crate::host_platform::publish_new_save(&temp, path)
        }
    });
    if !replace || result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub(super) fn slot_path(dir: &Path, slot: usize) -> PathBuf {
    if slot == 0 {
        ["NGUSaveSteam.txt", "NGUSave.txt"]
            .into_iter()
            .map(|n| dir.join(n))
            .filter(|p| p.is_file())
            .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
            .unwrap_or_else(|| dir.join("NGUSaveSteam.txt"))
    } else {
        dir.join("slots").join(format!("slot-{slot}.txt"))
    }
}

pub(crate) fn record_autosave_slot(dir: &Path, slot: usize, data: &[u8]) {
    let metadata = format!("{slot}\n{:x}", Md5::digest(data));
    if let Err(error) = atomic_write(&dir.join("autosave-slot.txt"), metadata.as_bytes()) {
        crate::elog!("Could not remember the autosave slot: {error}");
    }
}

pub(crate) fn autosave_written(vm: &Vm, path: &Path, data: &[u8]) {
    let state = &vm.eng.save_slots;
    if let Some(slot) = state.active_slot.filter(|_| state.home.is_some()) {
        let dir = Path::new(&vm.eng.save_dir);
        if path.parent() == Some(dir)
            && matches!(path.file_name().and_then(|name| name.to_str()), Some("NGUSaveSteam.txt" | "NGUSave.txt"))
            && !data.is_empty() {
            record_autosave_slot(dir, slot, data);
        }
    }
}

pub(crate) fn write_managed_file(vm: &mut Vm, filename: String, data: String) {
    let Some(data) = crate::host_platform::defer_autosave(vm, &filename, data) else {
        return;
    };
    let path = Path::new(&filename);
    if fs::write(path, data.as_bytes()).is_ok() {
        autosave_written(vm, path, data.as_bytes());
    }
}

pub(super) fn autosave_slot(dir: &Path, path: &Path) -> Option<usize> {
    if fs::metadata(path).ok()?.len() > MAX_SAVE_BYTES { return None; }
    let data = fs::read(path).ok()?;
    if let Ok(info) = fs::read_to_string(dir.join("autosave-slot.txt")) {
        if let Some((slot, digest)) = info.split_once('\n') {
            if let Ok(index @ 0..=SLOT_COUNT) = slot.parse::<usize>() {
                if digest == format!("{:x}", Md5::digest(&data)) {
                    return (index > 0 && slot_path(dir, index).is_file()).then_some(index);
                }
            }
        }
    }
    // Older profiles can be associated only when exactly one slot matches.
    let mut matches = (1..ROW_COUNT).filter(|&index| {
        let slot = slot_path(dir, index);
        fs::metadata(&slot).is_ok_and(|m| m.len() == data.len() as u64)
            && fs::read(slot).is_ok_and(|save| save == data)
    });
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}

pub(super) fn play_time(seconds: f64) -> String {
    let s = seconds as u64;
    if s >= 86400 {
        format!("{}d {}h {:02}m", s / 86400, s / 3600 % 24, s / 60 % 60)
    } else if s >= 3600 {
        format!("{}h {:02}m", s / 3600, s / 60 % 60)
    } else {
        format!("{}m {:02}s", s / 60, s % 60)
    }
}

pub(super) fn date(time: SystemTime) -> String {
    let s = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let (year, month, day) = civil_from_days(s / 86400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        s / 3600 % 24,
        s / 60 % 60
    )
}

pub(super) fn load_entries(dir: &Path) -> Vec<Entry> {
    (0..ROW_COUNT)
        .map(|i| {
            let path = slot_path(dir, i);
            if !path.exists() {
                return Entry::Empty;
            }
            match read_save(&path) {
                Ok((_, info)) => {
                    Entry::Save(info, fs::metadata(&path).and_then(|m| m.modified()).ok())
                }
                Err(_) => Entry::Invalid,
            }
        })
        .collect()
}
