//! Reads Unity objects from a type tree into JSON, field order preserved.

use super::SerializedFile;
use serde_json::{Map, Number, Value};
use std::sync::OnceLock;

const ALIGN: u64 = 0x4000;

/// One node of a Unity type tree.
#[derive(Debug, Clone)]
pub struct Node {
    pub ty: String,
    pub name: String,
    pub flags: u64,
    pub children: Vec<Node>,
}

impl Node {
    fn from_json(value: &Value) -> Node {
        let list = value.as_array().expect("type tree node");
        Node {
            ty: list[0].as_str().unwrap().to_owned(),
            name: list[1].as_str().unwrap().to_owned(),
            flags: list[2].as_u64().unwrap(),
            children: list[3]
                .as_array()
                .unwrap()
                .iter()
                .map(Node::from_json)
                .collect(),
        }
    }

    fn aligned(&self) -> bool {
        self.flags & ALIGN != 0
    }
}

/// Layout of an engine class (the same for every Unity 2019.4 game).
pub fn builtin(class_id: i32) -> Option<&'static Node> {
    static TABLE: OnceLock<Vec<(i32, Node)>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let json: Value = serde_json::from_str(include_str!("builtin.json")).expect("builtin.json");
        json.as_object()
            .unwrap()
            .iter()
            .map(|(id, node)| (id.parse().unwrap(), Node::from_json(node)))
            .collect()
    });
    table
        .iter()
        .find(|(id, _)| *id == class_id)
        .map(|(_, node)| node)
}

/// Little-endian cursor over one object's bytes.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    file: &'a SerializedFile,
    /// Above zero while inside a pair: the Python exporter leaves those
    /// references as they are.
    pairs: usize,
}

type Result<T> = std::result::Result<T, String>;

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8], file: &'a SerializedFile) -> Self {
        Reader {
            data,
            pos: 0,
            file,
            pairs: 0,
        }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(count)
            .filter(|end| *end <= self.data.len());
        let end = end.ok_or("Unexpected end of object data")?;
        let bytes = &self.data[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().unwrap())
    }

    fn int(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn align(&mut self) {
        self.pos = (self.pos + 3) & !3;
    }

    fn string(&mut self) -> Result<Value> {
        let length = self.int()?;
        if length > 0 && length as usize <= self.data.len() - self.pos {
            let bytes = self.take(length as usize)?;
            self.align();
            return Ok(Value::String(String::from_utf8_lossy(bytes).into_owned()));
        }
        Ok(Value::String(String::new()))
    }

    fn float(value: f64) -> Value {
        Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }

    fn primitive(&mut self, ty: &str) -> Result<Option<Value>> {
        Ok(Some(match ty {
            "SInt8" => Value::from(i8::from_le_bytes(self.array()?)),
            "UInt8" | "char" => Value::from(self.take(1)?[0]),
            "short" | "SInt16" => Value::from(i16::from_le_bytes(self.array()?)),
            "unsigned short" | "UInt16" => Value::from(u16::from_le_bytes(self.array()?)),
            "int" | "SInt32" => Value::from(self.int()?),
            "unsigned int" | "UInt32" | "Type*" => Value::from(u32::from_le_bytes(self.array()?)),
            "long long" | "SInt64" => Value::from(i64::from_le_bytes(self.array()?)),
            "unsigned long long" | "UInt64" | "FileSize" => {
                Value::from(u64::from_le_bytes(self.array()?))
            }
            "float" => Self::float(f64::from(f32::from_le_bytes(self.array()?))),
            "double" => Self::float(f64::from_le_bytes(self.array()?)),
            "bool" => Value::Bool(self.take(1)?[0] != 0),
            "string" => self.string()?,
            "TypelessData" => {
                let length = self.int()?;
                let bytes = self.take(usize::try_from(length).map_err(|_| "Negative length")?)?;
                serde_json::json!({ "$bytes": bytes.len() })
            }
            _ => return Ok(None),
        }))
    }

    /// Turns `{m_FileID, m_PathID}` into `"file:pathid"` (null when empty).
    fn reference(&self, fields: Map<String, Value>) -> Value {
        if self.pairs == 0 && fields.len() == 2 {
            if let (Some(file_id), Some(path_id)) = (
                fields.get("m_FileID").and_then(Value::as_i64),
                fields.get("m_PathID").and_then(Value::as_i64),
            ) {
                if path_id == 0 {
                    return Value::Null;
                }
                return Value::String(format!("{}:{path_id}", self.file.file_name(file_id as i32)));
            }
        }
        Value::Object(fields)
    }

    /// Reads one value. `own_align` is false for elements of an array whose
    /// element type is aligned: such arrays align once, after the last element.
    pub fn value(&mut self, node: &Node, own_align: bool) -> Result<Value> {
        let value = if let Some(value) = self.primitive(&node.ty)? {
            value
        } else if node.children.first().is_some_and(|c| c.ty == "Array") {
            let array = &node.children[0];
            let size = self.int()?;
            if size < 0 {
                return Err("Negative length in type tree".into());
            }
            let element = &array.children[1];
            let mut items = Vec::new();
            for _ in 0..size {
                items.push(self.value(element, !element.aligned())?);
            }
            if array.aligned() || element.aligned() {
                self.align();
            }
            Value::Array(items)
        } else if node.ty == "pair" {
            self.pairs += 1;
            let first = self.value(&node.children[0], true);
            let second = self.value(&node.children[1], true);
            self.pairs -= 1;
            Value::Array(vec![first?, second?])
        } else {
            let mut fields = Map::new();
            for child in &node.children {
                fields.insert(child.name.clone(), self.value(child, true)?);
            }
            self.reference(fields)
        };
        if own_align && node.aligned() {
            self.align();
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(ty: &str, name: &str, flags: u64, children: Vec<Node>) -> Node {
        Node {
            ty: ty.into(),
            name: name.into(),
            flags,
            children,
        }
    }

    fn file() -> SerializedFile {
        let mut bytes = vec![0; 8];
        bytes.extend(21u32.to_be_bytes());
        bytes.extend(0u32.to_be_bytes());
        bytes.extend([0, 0, 0, 0]);
        bytes.extend(b"2019.4.22f1\0");
        bytes.extend(19i32.to_le_bytes());
        bytes.push(0);
        bytes.extend([0; 16]); // no types, objects, scripts or externals
        SerializedFile::parse("level0".into(), bytes).unwrap()
    }

    #[test]
    fn reads_aligned_strings_pairs_and_references() {
        let reference = node(
            "PPtr<Object>",
            "ref",
            0,
            vec![
                node("int", "m_FileID", 0, vec![]),
                node("SInt64", "m_PathID", 0, vec![]),
            ],
        );
        let pair = node(
            "pair",
            "p",
            0,
            vec![node("string", "first", 0x4000, vec![]), reference.clone()],
        );
        let root = node(
            "Root",
            "Base",
            0,
            vec![node("string", "name", 0x4000, vec![]), reference, pair],
        );
        let mut data = Vec::new();
        data.extend(2i32.to_le_bytes());
        data.extend(b"ab\0\0");
        data.extend(0i32.to_le_bytes());
        data.extend(7i64.to_le_bytes());
        data.extend(1i32.to_le_bytes());
        data.extend(b"x\0\0\0");
        data.extend(0i32.to_le_bytes());
        data.extend(7i64.to_le_bytes());
        let file = file();
        let mut reader = Reader::new(&data, &file);
        let value = reader.value(&root, true).unwrap();
        assert_eq!(reader.position(), data.len());
        assert_eq!(value["name"], "ab");
        assert_eq!(value["ref"], "level0:7");
        assert_eq!(value["p"][0], "x");
        assert_eq!(value["p"][1]["m_PathID"], 7, "pairs keep raw references");
    }

    #[test]
    fn truncated_data_is_an_error() {
        let file = file();
        let mut reader = Reader::new(&[1, 0], &file);
        assert!(reader.value(&node("int", "x", 0, vec![]), true).is_err());
    }

    #[test]
    fn engine_layouts_load() {
        assert_eq!(builtin(1).unwrap().ty, "GameObject");
        assert!(builtin(99_999).is_none());
    }
}
