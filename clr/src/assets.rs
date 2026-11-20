//! Access to the prepared data: folder (PC) or single `ngu.pak` archive (Vita), + log.
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Mutex;

struct Pak {
    f: std::fs::File,
    idx: HashMap<String, (u64, u64)>,
}

static PAK: Mutex<Option<Pak>> = Mutex::new(None);
static DIR: Mutex<String> = Mutex::new(String::new());
static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// `dir`: data folder. If `dir/ngu.pak` exists it is used.
pub fn init(dir: &str) {
    *DIR.lock().unwrap() = dir.to_string();
    let p = format!("{}/ngu.pak", dir);
    if let Ok(mut f) = std::fs::File::open(&p) {
        let mut hdr = [0u8; 12];
        if f.read_exact(&mut hdr).is_ok() && &hdr[..8] == b"NGUPAK1\0" {
            let n = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]) as usize;
            let mut idx = HashMap::new();
            for _ in 0..n {
                let mut l = [0u8; 2];
                if f.read_exact(&mut l).is_err() {
                    break;
                }
                let nl = u16::from_le_bytes(l) as usize;
                let mut name = vec![0u8; nl];
                let mut rest = [0u8; 16];
                if f.read_exact(&mut name).is_err() || f.read_exact(&mut rest).is_err() {
                    break;
                }
                let off = u64::from_le_bytes(rest[..8].try_into().unwrap());
                let size = u64::from_le_bytes(rest[8..].try_into().unwrap());
                idx.insert(String::from_utf8_lossy(&name).to_string(), (off, size));
            }
            *PAK.lock().unwrap() = Some(Pak { f, idx });
        }
    }
}

pub fn read(rel: &str) -> Option<Vec<u8>> {
    if let Some(p) = PAK.lock().unwrap().as_mut() {
        if let Some(&(off, size)) = p.idx.get(rel) {
            let mut buf = vec![0u8; size as usize];
            p.f.seek(SeekFrom::Start(off)).ok()?;
            p.f.read_exact(&mut buf).ok()?;
            return Some(buf);
        }
        return None;
    }
    let d = DIR.lock().unwrap().clone();
    std::fs::read(format!("{}/{}", d, rel)).ok()
}

pub fn read_string(rel: &str) -> Option<String> {
    read(rel).map(|b| String::from_utf8_lossy(&b).to_string())
}

pub fn set_log(path: &str) {
    if let Ok(f) = std::fs::File::create(path) {
        *LOG.lock().unwrap() = Some(f);
    }
}

pub fn log(s: &str) {
    if let Some(f) = LOG.lock().unwrap().as_mut() {
        let _ = writeln!(f, "{}", s);
        let _ = f.flush();
    } else {
        eprintln!("{}", s);
    }
}

#[macro_export]
macro_rules! elog {
    ($($arg:tt)*) => {
        $crate::assets::log(&format!($($arg)*))
    };
}
