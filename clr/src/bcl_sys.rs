//! System intrinsics: Regex, hashes, AES, memory streams, BinaryFormatter (home-made V1 format).
use crate::bcl_core::{iter_values, new_array};
use crate::code::NativeFn;
use crate::program::Ty;
use crate::value::*;
use crate::Vm;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Mutex;

// ------------------------------------------------------------------ Regex
static RE_CACHE: Mutex<Option<HashMap<String, regex::Regex>>> = Mutex::new(None);

fn with_re<R>(pat: &str, f: impl FnOnce(&regex::Regex) -> R) -> Option<R> {
    let mut g = RE_CACHE.lock().unwrap();
    let m = g.get_or_insert_with(HashMap::new);
    if !m.contains_key(pat) {
        match regex::Regex::new(pat) {
            Ok(r) => {
                m.insert(pat.to_string(), r);
            }
            Err(_) => return None,
        }
    }
    Some(f(&m[pat]))
}

fn st(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        _ => String::new(),
    }
}

fn bytes_of(vm: &mut Vm, v: &Value) -> R<Vec<u8>> {
    Ok(iter_values(vm, v)?.iter().map(|x| x.as_i32() as u8).collect())
}

fn byte_array(vm: &mut Vm, b: &[u8]) -> Value {
    new_array(vm, b.iter().map(|x| Value::I32(*x as i32)).collect())
}

// ------------------------------------------------------------------ native objects
enum Hash {
    Md5,
    Sha256,
}
struct Mem {
    buf: Vec<u8>,
    pos: usize,
}
struct FileDlg {
    name: String,
}
struct Aes {
    key: Vec<u8>,
    enc: bool,
}

fn native_obj(vm: &mut Vm, tname: &str, p: Box<dyn std::any::Any>) -> Value {
    let id = vm.type_id(tname);
    Value::Obj(Obj::new(id, 0, Ext::Native(p)))
}

fn set_native(this: &Value, p: Box<dyn std::any::Any>) {
    if let Value::Obj(o) = this {
        *o.x.borrow_mut() = Ext::Native(p);
    }
}

fn with_native<T: 'static, R>(v: &Value, f: impl FnOnce(&mut T) -> R) -> Option<R> {
    if let Value::Obj(o) = v {
        if let Ext::Native(b) = &mut *o.x.borrow_mut() {
            if let Some(m) = b.downcast_mut::<T>() {
                return Some(f(m));
            }
        }
    }
    None
}

fn aes_ecb(key: &[u8], data: &[u8], enc: bool) -> Option<Vec<u8>> {
    use aes::cipher::generic_array::GenericArray;
    use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
    macro_rules! run {
        ($c:ty) => {{
            let c = <$c>::new_from_slice(key).ok()?;
            if enc {
                let pad = 16 - data.len() % 16;
                let mut d = data.to_vec();
                d.extend(std::iter::repeat(pad as u8).take(pad));
                for ch in d.chunks_mut(16) {
                    c.encrypt_block(GenericArray::from_mut_slice(ch));
                }
                Some(d)
            } else {
                if data.is_empty() || data.len() % 16 != 0 {
                    return None;
                }
                let mut d = data.to_vec();
                for ch in d.chunks_mut(16) {
                    c.decrypt_block(GenericArray::from_mut_slice(ch));
                }
                let pad = *d.last()? as usize;
                if pad == 0 || pad > 16 || pad > d.len() {
                    return None;
                }
                d.truncate(d.len() - pad);
                Some(d)
            }
        }};
    }
    match key.len() {
        16 => run!(aes::Aes128),
        24 => run!(aes::Aes192),
        32 => run!(aes::Aes256),
        _ => None,
    }
}

// ------------------------------------------------------------------ home-made serialization
const T_NULL: u8 = 0;
const T_I32: u8 = 1;
const T_I64: u8 = 2;
const T_F32: u8 = 3;
const T_F64: u8 = 4;
const T_STR: u8 = 6;
const T_OBJ: u8 = 10;
const T_ARR: u8 = 11;
const T_LIST: u8 = 12;
const T_DICT: u8 = 13;
const T_STRUCT: u8 = 14;

fn w_u32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn w_str(o: &mut Vec<u8>, s: &str) {
    w_u32(o, s.len() as u32);
    o.extend_from_slice(s.as_bytes());
}

fn ser_write(vm: &mut Vm, v: &Value, o: &mut Vec<u8>, depth: usize) -> R<()> {
    if depth > 200 {
        return vm.throw("System.Runtime.Serialization.SerializationException", "graph too deep");
    }
    match v {
        Value::Null => o.push(T_NULL),
        Value::I32(i) => {
            o.push(T_I32);
            o.extend_from_slice(&i.to_le_bytes());
        }
        Value::I64(i) => {
            o.push(T_I64);
            o.extend_from_slice(&i.to_le_bytes());
        }
        Value::F32(f) => {
            o.push(T_F32);
            o.extend_from_slice(&f.to_le_bytes());
        }
        Value::F64(f) => {
            o.push(T_F64);
            o.extend_from_slice(&f.to_le_bytes());
        }
        Value::Str(s) => {
            o.push(T_STR);
            w_str(o, s);
        }
        Value::Ptr(p) => {
            let x = vm.load_ptr(p);
            return ser_write(vm, &x, o, depth + 1);
        }
        Value::Struct(s) => {
            o.push(T_STRUCT);
            let n = vm.types[s.ty as usize].name.clone();
            w_str(o, &n);
            w_u32(o, s.f.len() as u32);
            for f in &s.f {
                ser_write(vm, f, o, depth + 1)?;
            }
        }
        Value::Obj(ob) => {
            enum K {
                Boxed(Value),
                Arr(Vec<Value>),
                List(Vec<Value>),
                Dict(Vec<(Value, Value)>),
                Plain,
            }
            let k = match &*ob.x.borrow() {
                Ext::Boxed(b) => K::Boxed(b.clone()),
                Ext::Arr(a) => K::Arr(a.clone()),
                Ext::List(a) => K::List(a.clone()),
                Ext::Dict(d) => K::Dict(d.iter().cloned().collect()),
                _ => K::Plain,
            };
            match k {
                K::Boxed(b) => return ser_write(vm, &b, o, depth + 1),
                K::Arr(a) | K::List(a) => {
                    o.push(if matches!(&*ob.x.borrow(), Ext::Arr(_)) { T_ARR } else { T_LIST });
                    w_u32(o, a.len() as u32);
                    for x in &a {
                        ser_write(vm, x, o, depth + 1)?;
                    }
                }
                K::Dict(d) => {
                    o.push(T_DICT);
                    w_u32(o, d.len() as u32);
                    for (k, v) in &d {
                        ser_write(vm, k, o, depth + 1)?;
                        ser_write(vm, v, o, depth + 1)?;
                    }
                }
                K::Plain => {
                    let ty = ob.ty;
                    if vm.types[ty as usize].user.is_none() {
                        return vm.throw("System.Runtime.Serialization.SerializationException", &format!("type not serializable: {}", vm.types[ty as usize].name));
                    }
                    o.push(T_OBJ);
                    let name = vm.types[ty as usize].name.clone();
                    w_str(o, &name);
                    let fields = vm.types[ty as usize].fields.clone();
                    let skip = vm.types[ty as usize].field_skip.clone();
                    let vals: Vec<Value> = ob.f.borrow().clone();
                    let n = (0..fields.len()).filter(|i| !skip.get(*i).copied().unwrap_or(false)).count();
                    w_u32(o, n as u32);
                    for i in 0..fields.len() {
                        if skip.get(i).copied().unwrap_or(false) {
                            continue;
                        }
                        w_str(o, &fields[i]);
                        ser_write(vm, &vals[i], o, depth + 1)?;
                    }
                }
            }
        }
    }
    Ok(())
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}
impl Rd<'_> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.p..self.p + n)?;
        self.p += n;
        Some(s)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u32()? as usize;
        Some(String::from_utf8_lossy(self.take(n)?).to_string())
    }
}

fn coerce(v: Value, ty: &Ty) -> Value {
    match (ty, &v) {
        (Ty::I64 | Ty::U64, Value::I32(i)) => Value::I64(*i as i64),
        (Ty::F32, x) if !matches!(x, Value::F32(_) | Value::Null) => Value::F32(x.as_f32()),
        (Ty::F64, x) if !matches!(x, Value::F64(_) | Value::Null) => Value::F64(x.as_f64()),
        (Ty::I32 | Ty::U32 | Ty::Bool | Ty::Char | Ty::I16 | Ty::U16 | Ty::I8 | Ty::U8, Value::I64(i)) => Value::I32(*i as i32),
        _ => v,
    }
}

fn ser_read(vm: &mut Vm, r: &mut Rd, depth: usize) -> R<Value> {
    macro_rules! bad {
        () => {
            return vm.throw("System.Runtime.Serialization.SerializationException", "invalid save data")
        };
    }
    if depth > 200 {
        bad!();
    }
    let t = match r.u8() {
        Some(t) => t,
        None => bad!(),
    };
    Ok(match t {
        T_NULL => Value::Null,
        T_I32 => match r.take(4) {
            Some(b) => Value::I32(i32::from_le_bytes(b.try_into().unwrap())),
            None => bad!(),
        },
        T_I64 => match r.take(8) {
            Some(b) => Value::I64(i64::from_le_bytes(b.try_into().unwrap())),
            None => bad!(),
        },
        T_F32 => match r.take(4) {
            Some(b) => Value::F32(f32::from_le_bytes(b.try_into().unwrap())),
            None => bad!(),
        },
        T_F64 => match r.take(8) {
            Some(b) => Value::F64(f64::from_le_bytes(b.try_into().unwrap())),
            None => bad!(),
        },
        T_STR => match r.str() {
            Some(s) => Value::str(&s),
            None => bad!(),
        },
        T_ARR | T_LIST => {
            let n = match r.u32() {
                Some(n) => n as usize,
                None => bad!(),
            };
            let mut items = Vec::with_capacity(n.min(1 << 20));
            for _ in 0..n {
                items.push(ser_read(vm, r, depth + 1)?);
            }
            if t == T_ARR {
                new_array(vm, items)
            } else {
                crate::bcl_core::new_list(vm, items)
            }
        }
        T_DICT => {
            let n = match r.u32() {
                Some(n) => n as usize,
                None => bad!(),
            };
            let mut d = Dict::default();
            for _ in 0..n {
                let k = ser_read(vm, r, depth + 1)?;
                let v = ser_read(vm, r, depth + 1)?;
                d.set(k, v);
            }
            let id = vm.type_id("System.Collections.Generic.Dictionary`2");
            Value::Obj(Obj::new(id, 0, Ext::Dict(d)))
        }
        T_STRUCT => {
            let name = match r.str() {
                Some(s) => s,
                None => bad!(),
            };
            let n = match r.u32() {
                Some(n) => n as usize,
                None => bad!(),
            };
            let id = vm.type_id(&name);
            let mut sv = vm.default_struct(id);
            for i in 0..n {
                let v = ser_read(vm, r, depth + 1)?;
                if let Value::Struct(s) = &mut sv {
                    if i < s.f.len() {
                        let fty = vm.types[id as usize].field_tys.get(i).cloned().unwrap_or(Ty::Object);
                        s.f[i] = coerce(v, &fty);
                    }
                }
            }
            sv
        }
        T_OBJ => {
            let name = match r.str() {
                Some(s) => s,
                None => bad!(),
            };
            let n = match r.u32() {
                Some(n) => n as usize,
                None => bad!(),
            };
            let id = match vm.type_ids.get(&name) {
                Some(i) => *i,
                None => bad!(),
            };
            let o = vm.new_obj(id); // without constructor, like BinaryFormatter
            vm.eng.deser.insert(Rc::as_ptr(&o) as *const u8 as usize);
            for _ in 0..n {
                let fname = match r.str() {
                    Some(s) => s,
                    None => bad!(),
                };
                let v = ser_read(vm, r, depth + 1)?;
                if let Some(slot) = vm.types[id as usize].fields.iter().position(|f| *f == fname) {
                    let fty = vm.types[id as usize].field_tys[slot].clone();
                    o.f.borrow_mut()[slot] = coerce(v, &fty);
                }
            }
            Value::Obj(o)
        }
        _ => bad!(),
    })
}

// ------------------------------------------------------------------ registry
pub fn lookup(key: &str) -> Option<NativeFn> {
    let f: NativeFn = match key {
        // ---------------------------------------------------------- Regex
        "System.Text.RegularExpressions.Regex::Replace" => |_vm, a| {
            let (input, pat, rep) = (st(&a[0]), st(&a[1]), st(&a[2]));
            Ok(Value::str(&with_re(&pat, |r| r.replace_all(&input, rep.as_str()).to_string()).unwrap_or(input)))
        },
        "System.Text.RegularExpressions.Regex::Split" => |vm, a| {
            let (input, pat) = (st(&a[0]), st(&a[1]));
            let parts: Vec<Value> = with_re(&pat, |r| r.split(&input).map(Value::str).collect()).unwrap_or_else(|| vec![Value::str(&input)]);
            Ok(new_array(vm, parts))
        },
        "System.Text.RegularExpressions.Regex::IsMatch" => |_vm, a| {
            let (input, pat) = (st(&a[0]), st(&a[1]));
            Ok(Value::bool(with_re(&pat, |r| r.is_match(&input)).unwrap_or(false)))
        },
        // ---------------------------------------------------------- hashes
        "System.Security.Cryptography.MD5CryptoServiceProvider::.ctor" => |_vm, a| {
            set_native(&a[0], Box::new(Hash::Md5));
            Ok(Value::Null)
        },
        "System.Security.Cryptography.SHA256::Create" => |vm, _a| Ok(native_obj(vm, "System.Security.Cryptography.SHA256", Box::new(Hash::Sha256))),
        "System.Security.Cryptography.HashAlgorithm::ComputeHash" => |vm, a| {
            let this = vm.deref(a[0].clone());
            let data = bytes_of(vm, &a[1])?;
            let md5 = with_native::<Hash, _>(&this, |h| matches!(h, Hash::Md5)).unwrap_or(true);
            let out: Vec<u8> = if md5 {
                use md5::Digest;
                md5::Md5::digest(&data).to_vec()
            } else {
                use sha2::Digest;
                sha2::Sha256::digest(&data).to_vec()
            };
            Ok(byte_array(vm, &out))
        },
        "System.Security.Cryptography.RandomNumberGenerator::Create" => |vm, _a| Ok(native_obj(vm, "System.Security.Cryptography.RandomNumberGenerator", Box::new(()))),
        "System.Security.Cryptography.RandomNumberGenerator::GetNonZeroBytes" => |vm, a| {
            let n = iter_values(vm, &a[1])?.len();
            let mut s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1;
            let b: Vec<Value> = (0..n)
                .map(|_| {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    Value::I32((s % 255 + 1) as i32)
                })
                .collect();
            if let Value::Obj(o) = &a[1] {
                if let Ext::Arr(arr) = &mut *o.x.borrow_mut() {
                    *arr = b;
                }
            }
            Ok(Value::Null)
        },
        // ---------------------------------------------------------- AES (Rijndael ECB PKCS7)
        "System.Security.Cryptography.RijndaelManaged::.ctor" => |_vm, a| {
            set_native(&a[0], Box::new(Aes { key: vec![], enc: true }));
            Ok(Value::Null)
        },
        "System.Security.Cryptography.SymmetricAlgorithm::set_Key" => |vm, a| {
            let this = vm.deref(a[0].clone());
            let k = bytes_of(vm, &a[1])?;
            with_native::<Aes, _>(&this, |x| x.key = k);
            Ok(Value::Null)
        },
        "System.Security.Cryptography.SymmetricAlgorithm::set_Mode" | "System.Security.Cryptography.SymmetricAlgorithm::set_Padding" => |_vm, _a| Ok(Value::Null),
        "System.Security.Cryptography.SymmetricAlgorithm::CreateEncryptor" | "System.Security.Cryptography.SymmetricAlgorithm::CreateDecryptor" => |vm, a| {
            let this = vm.deref(a[0].clone());
            let key = with_native::<Aes, _>(&this, |x| x.key.clone()).unwrap_or_default();
            Ok(native_obj(vm, "System.Security.Cryptography.ICryptoTransform", Box::new(Aes { key, enc: true })))
        },
        "System.Security.Cryptography.ICryptoTransform::TransformFinalBlock" => |vm, a| {
            let this = vm.deref(a[0].clone());
            let (key, enc) = with_native::<Aes, _>(&this, |x| (x.key.clone(), x.enc)).unwrap_or_default();
            let all = bytes_of(vm, &a[1])?;
            let (off, len) = (a[2].as_i32() as usize, a[3].as_i32() as usize);
            let data = &all[off.min(all.len())..(off + len).min(all.len())];
            match aes_ecb(&key, data, enc) {
                Some(o) => Ok(byte_array(vm, &o)),
                None => vm.throw("System.Security.Cryptography.CryptographicException", "invalid encryption"),
            }
        },
        // ---------------------------------------------------------- streams
        "System.IO.MemoryStream::.ctor" => |vm, a| {
            let buf = if a.len() > 1 && matches!(&a[1], Value::Obj(_)) { bytes_of(vm, &a[1])? } else { vec![] };
            set_native(&a[0], Box::new(Mem { buf, pos: 0 }));
            Ok(Value::Null)
        },
        "System.IO.MemoryStream::ToArray" => |vm, a| {
            let this = vm.deref(a[0].clone());
            let b = with_native::<Mem, _>(&this, |m| m.buf.clone()).unwrap_or_default();
            Ok(byte_array(vm, &b))
        },
        "System.IO.Stream::Close" | "System.IO.Stream::Flush" | "System.IO.Stream::Dispose" | "System.IO.MemoryStream::Flush" | "System.IO.MemoryStream::Close" => |_vm, _a| Ok(Value::Null),
        "System.IO.File::Create" | "System.IO.File::Open" => |vm, _a| Ok(native_obj(vm, "System.IO.FileStream", Box::new(()))),
        // ---------------------------------------------------------- file dialogs (Ookii) -> export folder
        "Ookii.Dialogs.VistaSaveFileDialog::.ctor" | "Ookii.Dialogs.VistaOpenFileDialog::.ctor" | "Ookii.Dialogs.VistaFolderBrowserDialog::.ctor" => |_vm, a| {
            set_native(&a[0], Box::new(FileDlg { name: String::new() }));
            Ok(Value::Null)
        },
        "Ookii.Dialogs.VistaFileDialog::set_Filter" | "Ookii.Dialogs.VistaFileDialog::set_AddExtension" | "Ookii.Dialogs.VistaFileDialog::set_DefaultExt"
        | "Ookii.Dialogs.VistaFileDialog::set_FilterIndex" | "Ookii.Dialogs.VistaFileDialog::set_Title" | "Ookii.Dialogs.VistaOpenFileDialog::set_Multiselect"
        | "Ookii.Dialogs.VistaFolderBrowserDialog::set_Description" => |_vm, _a| Ok(Value::Null),
        "Ookii.Dialogs.VistaFileDialog::set_FileName" | "Ookii.Dialogs.VistaFolderBrowserDialog::set_SelectedPath" => |vm, a| {
            let this = vm.deref(a[0].clone());
            let n = st(&a[1]);
            with_native::<FileDlg, _>(&this, |d| d.name = n);
            Ok(Value::Null)
        },
        "Ookii.Dialogs.VistaFileDialog::get_FileName" | "Ookii.Dialogs.VistaFolderBrowserDialog::get_SelectedPath" => |vm, a| {
            let this = vm.deref(a[0].clone());
            let n = with_native::<FileDlg, _>(&this, |d| d.name.clone()).unwrap_or_default();
            let base = n.rsplit(['/', '\\']).next().unwrap_or("export.txt");
            let base = if base.is_empty() { "NGUExport.txt" } else { base };
            let dir = format!("{}/export", vm.eng.save_dir);
            let _ = std::fs::create_dir_all(&dir);
            Ok(Value::str(&format!("{}/{}", dir, base)))
        },
        "Ookii.Dialogs.VistaFileDialog::get_FileNames" => |vm, _a| {
            // the most recent export file
            let dir = format!("{}/export", vm.eng.save_dir);
            let mut best: Option<(std::time::SystemTime, String)> = None;
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    if let Ok(m) = e.metadata() {
                        if let Ok(t) = m.modified() {
                            if best.as_ref().map(|b| t > b.0).unwrap_or(true) {
                                best = Some((t, e.path().to_string_lossy().to_string()));
                            }
                        }
                    }
                }
            }
            let v = best.map(|b| vec![Value::str(&b.1)]).unwrap_or_default();
            Ok(new_array(vm, v))
        },
        "System.Windows.Forms.CommonDialog::ShowDialog" | "Ookii.Dialogs.VistaFileDialog::ShowDialog" => |_vm, _a| Ok(Value::I32(1)),
        // ---------------------------------------------------------- BinaryFormatter (home-made format)
        "System.Runtime.Serialization.Formatters.Binary.BinaryFormatter::.ctor" => |_vm, _a| Ok(Value::Null),
        "System.Runtime.Serialization.Formatters.Binary.BinaryFormatter::Serialize" => |vm, a| {
            let stream = vm.deref(a[1].clone());
            let mut out = b"NGUS1".to_vec();
            ser_write(vm, &a[2], &mut out, 0)?;
            with_native::<Mem, _>(&stream, |m| {
                m.buf.extend_from_slice(&out);
                m.pos = m.buf.len();
            });
            Ok(Value::Null)
        },
        "System.Runtime.Serialization.Formatters.Binary.BinaryFormatter::Deserialize" => |vm, a| {
            let stream = vm.deref(a[1].clone());
            let (buf, pos) = with_native::<Mem, _>(&stream, |m| (m.buf.clone(), m.pos)).unwrap_or_default();
            if buf.len() < pos + 5 || &buf[pos..pos + 5] != b"NGUS1" {
                return vm.throw("System.Runtime.Serialization.SerializationException", "unrecognised save format (save from another platform?)");
            }
            let mut rd = Rd { b: &buf, p: pos + 5 };
            let v = ser_read(vm, &mut rd, 0)?;
            with_native::<Mem, _>(&stream, |m| m.pos = rd.p);
            Ok(v)
        },
        _ => return crate::unity_api::lookup(key),
    };
    Some(f)
}

#[allow(dead_code)]
fn unused(_: Rc<()>) {}
