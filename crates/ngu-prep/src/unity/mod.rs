//! Reader for Unity 2019.4 serialized files (format version 21).
//!
//! The game's files carry no type trees: engine classes use the layouts in
//! `builtin.json` and scripts use layouts built from the game's assemblies.

pub mod assets;
pub mod audio;
pub mod export;
pub mod image;
pub mod layout;
pub mod scene;
pub mod typetree;

use std::path::Path;

type Result<T> = std::result::Result<T, String>;

/// One object stored in a serialized file.
#[derive(Debug, Clone)]
pub struct Object {
    pub path_id: i64,
    pub class_id: i32,
    start: usize,
    size: usize,
}

/// A parsed serialized file kept in memory.
pub struct SerializedFile {
    pub name: String,
    pub objects: Vec<Object>,
    /// File names referenced by a non-zero file id (1-based).
    pub externals: Vec<String>,
    /// (file id, path id) of each script type, indexed by `script_index`.
    pub scripts: Vec<(i32, i64)>,
    data: Vec<u8>,
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8]> {
        let end = self
            .pos
            .checked_add(count)
            .filter(|end| *end <= self.data.len());
        let end = end.ok_or("Truncated serialized file")?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(self.i32()? as u32)
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn align(&mut self) {
        self.pos = (self.pos + 3) & !3;
    }
    fn cstring(&mut self) -> Result<String> {
        let rest = &self.data[self.pos.min(self.data.len())..];
        let end = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or("Unterminated string")?;
        let text = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.pos += end + 1;
        Ok(text)
    }
}

impl SerializedFile {
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Self::parse(name, data)
    }

    pub fn parse(name: String, data: Vec<u8>) -> Result<Self> {
        let big = |at: usize| -> Result<u32> {
            let bytes = data.get(at..at + 4).ok_or("Truncated serialized file")?;
            Ok(u32::from_be_bytes(bytes.try_into().unwrap()))
        };
        let version = big(8)?;
        let data_offset = big(12)? as usize;
        if version != 21 {
            return Err(format!("Unsupported serialized file version {version}."));
        }
        if data.get(16) != Some(&0) {
            return Err("Big-endian serialized files are not supported.".into());
        }
        let mut c = Cursor {
            data: &data,
            pos: 20,
        };
        c.cstring()?; // Unity version
        c.i32()?; // target platform
        let with_trees = c.byte()? != 0;
        if with_trees {
            return Err("Serialized files with embedded type trees are not supported.".into());
        }
        let mut class_ids = Vec::new();
        for _ in 0..c.i32()? {
            let class_id = c.i32()?;
            c.byte()?; // stripped
            c.i16()?; // script type index
            if class_id == 114 {
                c.take(16)?;
            }
            c.take(16)?;
            class_ids.push(class_id);
        }
        let mut objects = Vec::new();
        for _ in 0..c.i32()? {
            c.align();
            let path_id = c.i64()?;
            let start = data_offset + c.u32()? as usize;
            let size = c.u32()? as usize;
            let type_index = c.i32()?;
            let class_id = *class_ids
                .get(usize::try_from(type_index).map_err(|_| "Bad type index")?)
                .ok_or("Bad type index")?;
            if start.checked_add(size).is_none_or(|end| end > data.len()) {
                return Err(format!("Object {path_id} lies outside the file."));
            }
            objects.push(Object {
                path_id,
                class_id,
                start,
                size,
            });
        }
        let mut scripts = Vec::new();
        for _ in 0..c.i32()? {
            let file = c.i32()?;
            c.align();
            scripts.push((file, c.i64()?));
        }
        let mut externals = Vec::new();
        for _ in 0..c.i32()? {
            c.cstring()?;
            c.take(16)?;
            c.i32()?;
            let path = c.cstring()?;
            externals.push(path.rsplit(['/', '\\']).next().unwrap_or("").to_owned());
        }
        Ok(SerializedFile {
            name,
            objects,
            externals,
            scripts,
            data,
        })
    }

    /// Raw bytes of an object.
    pub fn bytes(&self, object: &Object) -> &[u8] {
        &self.data[object.start..object.start + object.size]
    }

    /// `file:path_id` text used for references, as in the exported scene.
    pub fn file_name(&self, file_id: i32) -> String {
        match usize::try_from(file_id) {
            Ok(0) => self.name.clone(),
            Ok(id) if id <= self.externals.len() => self.externals[id - 1].clone(),
            _ => format!("?{file_id}"),
        }
    }
}
