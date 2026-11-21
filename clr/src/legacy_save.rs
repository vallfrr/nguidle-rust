use crate::bcl_core::{b64_dec, b64_enc, new_array, new_list};
use crate::bcl_sys::coerce;
use crate::serial;
use crate::value::{Dict, Ext, Obj, Value};
use crate::Vm;
use md5::{Digest, Md5};

const SIGNATURE: &[u8] = b"NGUS1";
const LIMIT: usize = 16 * 1024 * 1024;
type LegacyResult<T> = Result<T, String>;

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, size: usize) -> LegacyResult<&'a [u8]> {
        let (value, tail) = self
            .0
            .split_at_checked(size)
            .ok_or("Truncated legacy save.")?;
        self.0 = tail;
        Ok(value)
    }

    fn count(&mut self) -> LegacyResult<usize> {
        let count = u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize;
        if count > self.0.len() {
            return Err("Invalid legacy record length.".into());
        }
        Ok(count)
    }

    fn text(&mut self) -> LegacyResult<&'a str> {
        let size = self.count()?;
        std::str::from_utf8(self.take(size)?)
            .map_err(|_| "Legacy save contains invalid UTF-8.".into())
    }

    fn value(&mut self, vm: &mut Vm, depth: usize) -> LegacyResult<Value> {
        if depth >= 128 {
            return Err("Legacy save nesting exceeds its supported depth.".into());
        }
        let kind = self.take(1)?[0];
        Ok(match kind {
            0 => Value::Null,
            1 => Value::I32(i32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            2 => Value::I64(i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            3 => Value::F32(f32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            4 => Value::F64(f64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            6 => Value::str(self.text()?),
            10 | 14 => {
                let name = self.text()?;
                let id = *vm
                    .type_ids
                    .get(name)
                    .ok_or_else(|| format!("Unknown legacy type {name}."))?;
                let count = self.count()?;
                if kind == 14 {
                    let Value::Struct(mut value) = vm.default_struct(id) else {
                        return Err(format!("Expected a value type for {name}."));
                    };
                    for index in 0..count {
                        let field = self.value(vm, depth + 1)?;
                        if let Some(slot) = value.f.get_mut(index) {
                            *slot = coerce(field, &vm.types[id as usize].field_tys[index]);
                        }
                    }
                    Value::Struct(value)
                } else {
                    let value = vm.new_obj(id);
                    for _ in 0..count {
                        let name = self.text()?;
                        let field = self.value(vm, depth + 1)?;
                        let ty = &vm.types[id as usize];
                        if let Some(index) =
                            ty.fields.iter().position(|candidate| candidate == name)
                        {
                            value.f.borrow_mut()[index] = coerce(field, &ty.field_tys[index]);
                        }
                    }
                    Value::Obj(value)
                }
            }
            11 | 12 => {
                let count = self.count()?;
                let items = (0..count)
                    .map(|_| self.value(vm, depth + 1))
                    .collect::<LegacyResult<Vec<_>>>()?;
                if kind == 11 {
                    new_array(vm, items)
                } else {
                    new_list(vm, items)
                }
            }
            13 => {
                let count = self.count()?;
                let mut dict = Dict::default();
                for _ in 0..count {
                    let key = self.value(vm, depth + 1)?;
                    let value = self.value(vm, depth + 1)?;
                    dict.set(key, value);
                }
                let id = vm.type_id("System.Collections.Generic.Dictionary`2");
                Value::Obj(Obj::new(id, 0, Ext::Dict(dict)))
            }
            _ => return Err(format!("Unknown legacy record {kind}.")),
        })
    }
}

fn read(vm: &mut Vm, bytes: &[u8], expected: &str) -> LegacyResult<Value> {
    let mut reader = Reader(
        bytes
            .strip_prefix(SIGNATURE)
            .ok_or("Expected an NGUS1 save.")?,
    );
    let value = reader.value(vm, 0)?;
    if !reader.0.is_empty()
        || !matches!(&value, Value::Obj(object) if vm.types[object.ty as usize].name == expected)
    {
        return Err(format!("Invalid legacy {expected} record."));
    }
    Ok(value)
}

pub fn convert(vm: &mut Vm, input: &str) -> LegacyResult<String> {
    if input.len() > LIMIT {
        return Err("The legacy save is too large.".into());
    }
    let root = read(vm, &b64_dec(input), "SaveData")?;
    let Value::Obj(wrapper) = &root else {
        unreachable!()
    };
    let fields = &vm.types[wrapper.ty as usize].fields;
    let player = fields
        .iter()
        .position(|name| name == "playerData")
        .ok_or("Missing player data.")?;
    let checksum = fields
        .iter()
        .position(|name| name == "checksum")
        .ok_or("Missing checksum.")?;
    let (data, digest) = {
        let values = wrapper.f.borrow();
        (
            b64_dec(values[player].as_str().ok_or("Invalid player data.")?),
            values[checksum]
                .as_str()
                .ok_or("Invalid checksum.")?
                .to_string(),
        )
    };
    if digest != b64_enc(&Md5::digest(&data)) {
        return Err("Legacy save checksum does not match.".into());
    }
    let character = read(vm, &data, "PlayerData")?;
    let converted =
        serial::serialize(vm, &character).map_err(|_| "Could not serialize the character.")?;
    {
        let mut values = wrapper.f.borrow_mut();
        values[player] = Value::str(&b64_enc(&converted));
        values[checksum] = Value::str(&b64_enc(&Md5::digest(&converted)));
    }
    let output = serial::serialize(vm, &root).map_err(|_| "Could not serialize the save.")?;
    Ok(b64_enc(&output))
}
