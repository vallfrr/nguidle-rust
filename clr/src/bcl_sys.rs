//! System intrinsics: Regex, hashes, AES, memory streams, BinaryFormatter (real .NET wire format through `serial`).
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

pub(crate) fn coerce(v: Value, ty: &Ty) -> Value {
    match (ty, &v) {
        (Ty::I64 | Ty::U64, Value::I32(i)) => Value::I64(*i as i64),
        (Ty::F32, x) if !matches!(x, Value::F32(_) | Value::Null) => Value::F32(x.as_f32()),
        (Ty::F64, x) if !matches!(x, Value::F64(_) | Value::Null) => Value::F64(x.as_f64()),
        (Ty::I32 | Ty::U32 | Ty::Bool | Ty::Char | Ty::I16 | Ty::U16 | Ty::I8 | Ty::U8, Value::I64(i)) => Value::I32(*i as i32),
        _ => v,
    }
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
        // ---------------------------------------------------------- BinaryFormatter (real .NET wire format, see serial.rs)
        "System.Runtime.Serialization.Formatters.Binary.BinaryFormatter::.ctor" => |_vm, _a| Ok(Value::Null),
        "System.Runtime.Serialization.Formatters.Binary.BinaryFormatter::Serialize" => |vm, a| {
            let stream = vm.deref(a[1].clone());
            let out = crate::serial::serialize(vm, &a[2])?;
            with_native::<Mem, _>(&stream, |m| {
                m.buf.extend_from_slice(&out);
                m.pos = m.buf.len();
            });
            Ok(Value::Null)
        },
        "System.Runtime.Serialization.Formatters.Binary.BinaryFormatter::Deserialize" => |vm, a| {
            let stream = vm.deref(a[1].clone());
            let (buf, pos) = with_native::<Mem, _>(&stream, |m| (m.buf.clone(), m.pos)).unwrap_or_default();
            let v = crate::serial::deserialize(vm, &buf[pos.min(buf.len())..])?;
            with_native::<Mem, _>(&stream, |m| m.pos = m.buf.len());
            Ok(v)
        },
        _ => return crate::unity_api::lookup(key),
    };
    Some(f)
}

#[allow(dead_code)]
fn unused(_: Rc<()>) {}
