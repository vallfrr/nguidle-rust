//! .NET BinaryFormatter wire format (MS-NRBF): parser into a node tree and writer from a node tree.
//! Used to read and write the game's real save files (PC saves are interchangeable with the original game).
//! This module knows nothing about the VM: `bcl_sys` maps between nodes and engine values.
use std::collections::HashMap;
use std::rc::Rc;

/// Primitive values as stored on the wire (PrimitiveTypeEnum).
#[derive(Clone, Debug, PartialEq)]
pub enum Pv {
    Bool(bool),
    U8(u8),
    Char(String),
    Decimal(String),
    F64(f64),
    I16(i16),
    I32(i32),
    I64(i64),
    I8(i8),
    F32(f32),
    /// TimeSpan ticks
    Ts(i64),
    /// DateTime: ticks | kind << 62
    Dt(u64),
    U16(u16),
    U32(u32),
    U64(u64),
}

impl Pv {
    pub fn code(&self) -> u8 {
        match self {
            Pv::Bool(_) => 1,
            Pv::U8(_) => 2,
            Pv::Char(_) => 3,
            Pv::Decimal(_) => 5,
            Pv::F64(_) => 6,
            Pv::I16(_) => 7,
            Pv::I32(_) => 8,
            Pv::I64(_) => 9,
            Pv::I8(_) => 10,
            Pv::F32(_) => 11,
            Pv::Ts(_) => 12,
            Pv::Dt(_) => 13,
            Pv::U16(_) => 14,
            Pv::U32(_) => 15,
            Pv::U64(_) => 16,
        }
    }
}

/// Additional type information of a member (MemberTypeInfo).
#[derive(Clone, Debug, PartialEq)]
pub enum Add {
    None,
    Prim(u8),
    Sys(String),
    Class(String, i32),
}

/// Class definition: name, members and their binary types.
#[derive(Debug, PartialEq)]
pub struct ClassMeta {
    pub name: String,
    /// library id (user classes); `None` for system classes
    pub lib: Option<i32>,
    pub names: Vec<String>,
    /// BinaryTypeEnum per member (0 primitive, 1 string, 2 object, 3 system class, 4 class, 5 object array, 6 string array, 7 primitive array)
    pub bt: Vec<u8>,
    pub add: Vec<Add>,
}

#[derive(Clone, Debug)]
pub enum N {
    Null,
    P(Pv),
    Str(i32, Rc<str>),
    Ref(i32),
    Obj(Rc<ObjN>),
    PArr(Rc<PArrN>),
    Arr(Rc<ArrN>),
}

#[derive(Debug)]
pub struct ObjN {
    pub id: i32,
    pub meta: Rc<ClassMeta>,
    pub vals: Vec<N>,
}

/// ArraySinglePrimitive
#[derive(Debug)]
pub struct PArrN {
    pub id: i32,
    pub ptype: u8,
    pub vals: Vec<Pv>,
}

/// ArraySingleObject / ArraySingleString / single-dimension BinaryArray: element binary type + info.
#[derive(Debug)]
pub struct ArrN {
    pub id: i32,
    pub etype: u8,
    pub eadd: Add,
    pub items: Vec<N>,
}

/// A parsed stream: the root node, the library names and an index of every identified node (for `Ref`).
pub struct Stream {
    pub root: N,
    pub libs: HashMap<i32, String>,
    pub index: HashMap<i32, N>,
}

type Res<T> = Result<T, String>;

enum Rec {
    Node(N),
    Nulls(usize),
    End,
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
    root: i32,
    libs: HashMap<i32, String>,
    classes: HashMap<i32, Rc<ClassMeta>>,
    index: HashMap<i32, N>,
}

impl<'a> Rd<'a> {
    fn u8(&mut self) -> Res<u8> {
        let v = *self.b.get(self.p).ok_or("truncated stream")?;
        self.p += 1;
        Ok(v)
    }
    fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        let s = self.b.get(self.p..self.p.checked_add(n).ok_or("bad length")?).ok_or("truncated stream")?;
        self.p += n;
        Ok(s)
    }
    fn i32(&mut self) -> Res<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn lps(&mut self) -> Res<String> {
        let (mut n, mut sh) = (0usize, 0);
        loop {
            let c = self.u8()?;
            n |= ((c & 0x7f) as usize) << sh;
            sh += 7;
            if c & 0x80 == 0 {
                break;
            }
            if sh > 35 {
                return Err("bad length prefix".into());
            }
        }
        Ok(String::from_utf8_lossy(self.take(n)?).to_string())
    }
    fn prim(&mut self, t: u8) -> Res<Pv> {
        Ok(match t {
            1 => Pv::Bool(self.u8()? != 0),
            2 => Pv::U8(self.u8()?),
            3 => {
                let first = self.u8()?;
                let n = if first < 0x80 { 1 } else if first >> 5 == 6 { 2 } else if first >> 4 == 14 { 3 } else { 4 };
                let mut v = vec![first];
                v.extend_from_slice(self.take(n - 1)?);
                Pv::Char(String::from_utf8_lossy(&v).to_string())
            }
            5 => Pv::Decimal(self.lps()?),
            6 => Pv::F64(f64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            7 => Pv::I16(i16::from_le_bytes(self.take(2)?.try_into().unwrap())),
            8 => Pv::I32(self.i32()?),
            9 => Pv::I64(i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            10 => Pv::I8(self.u8()? as i8),
            11 => Pv::F32(f32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            12 => Pv::Ts(i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            13 => Pv::Dt(u64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            14 => Pv::U16(u16::from_le_bytes(self.take(2)?.try_into().unwrap())),
            15 => Pv::U32(u32::from_le_bytes(self.take(4)?.try_into().unwrap())),
            16 => Pv::U64(u64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            t => return Err(format!("unsupported primitive type {t}")),
        })
    }
    fn add(&mut self, bt: u8) -> Res<Add> {
        Ok(match bt {
            0 | 7 => Add::Prim(self.u8()?),
            3 => Add::Sys(self.lps()?),
            4 => {
                let n = self.lps()?;
                Add::Class(n, self.i32()?)
            }
            _ => Add::None,
        })
    }
    fn class_info(&mut self) -> Res<(i32, String, Vec<String>)> {
        let id = self.i32()?;
        let name = self.lps()?;
        let n = self.i32()?;
        if !(0..100_000).contains(&n) {
            return Err("bad member count".into());
        }
        let names = (0..n).map(|_| self.lps()).collect::<Res<Vec<_>>>()?;
        Ok((id, name, names))
    }
    /// Values of `count` object-or-primitive slots; `elem` = fixed primitive element type for primitive slots.
    fn slots(&mut self, count: usize, mut kind: impl FnMut(usize) -> Option<u8>) -> Res<Vec<N>> {
        let mut out = Vec::with_capacity(count.min(1 << 16));
        while out.len() < count {
            if let Some(pt) = kind(out.len()) {
                out.push(N::P(self.prim(pt)?));
                continue;
            }
            match self.record()? {
                Rec::Node(n) => out.push(n),
                Rec::Nulls(k) => (0..k).for_each(|_| out.push(N::Null)),
                Rec::End => return Err("unexpected end of stream".into()),
            }
        }
        if out.len() != count {
            return Err("null run overflows the slot list".into());
        }
        Ok(out)
    }
    fn object(&mut self, id: i32, meta: Rc<ClassMeta>) -> Res<N> {
        let prims: Vec<Option<u8>> = meta.bt.iter().zip(&meta.add).map(|(b, a)| if *b == 0 { if let Add::Prim(t) = a { Some(*t) } else { None } } else { None }).collect();
        let vals = self.slots(meta.names.len(), |i| prims[i])?;
        let n = N::Obj(Rc::new(ObjN { id, meta, vals }));
        self.index.insert(id, n.clone());
        Ok(n)
    }
    fn record(&mut self) -> Res<Rec> {
        let t = self.u8()?;
        Ok(Rec::Node(match t {
            0 => {
                self.root = self.i32()?;
                self.take(12)?;
                return self.record();
            }
            11 => return Ok(Rec::End),
            12 => {
                let id = self.i32()?;
                let name = self.lps()?;
                self.libs.insert(id, name);
                return self.record();
            }
            10 => N::Null,
            13 => return Ok(Rec::Nulls(self.u8()? as usize)),
            14 => return Ok(Rec::Nulls(self.i32()?.max(0) as usize)),
            9 => N::Ref(self.i32()?),
            6 => {
                let id = self.i32()?;
                let s: Rc<str> = Rc::from(self.lps()?.as_str());
                let n = N::Str(id, s);
                self.index.insert(id, n.clone());
                n
            }
            8 => {
                let t = self.u8()?;
                N::P(self.prim(t)?)
            }
            1 => {
                let id = self.i32()?;
                let mid = self.i32()?;
                let meta = self.classes.get(&mid).cloned().ok_or("ClassWithId: unknown metadata id")?;
                self.object(id, meta)?
            }
            4 | 5 => {
                let (id, name, names) = self.class_info()?;
                let bt = (0..names.len()).map(|_| self.u8()).collect::<Res<Vec<_>>>()?;
                let add = bt.iter().map(|t| self.add(*t)).collect::<Res<Vec<_>>>()?;
                let lib = if t == 5 { Some(self.i32()?) } else { None };
                let meta = Rc::new(ClassMeta { name, lib, names, bt, add });
                self.classes.insert(id, meta.clone());
                self.object(id, meta)?
            }
            15 => {
                let id = self.i32()?;
                let n = self.i32()?;
                let pt = self.u8()?;
                if !(0..100_000_000).contains(&n) {
                    return Err("bad array length".into());
                }
                let vals = (0..n).map(|_| self.prim(pt)).collect::<Res<Vec<_>>>()?;
                let a = N::PArr(Rc::new(PArrN { id, ptype: pt, vals }));
                self.index.insert(id, a.clone());
                a
            }
            16 | 17 => {
                let id = self.i32()?;
                let n = self.i32()?;
                if !(0..100_000_000).contains(&n) {
                    return Err("bad array length".into());
                }
                let items = self.slots(n as usize, |_| None)?;
                let (etype, eadd) = if t == 16 { (2, Add::None) } else { (1, Add::None) };
                let a = N::Arr(Rc::new(ArrN { id, etype, eadd, items }));
                self.index.insert(id, a.clone());
                a
            }
            7 => {
                // BinaryArray: only single-dimension (Single / Jagged) arrays are supported
                let id = self.i32()?;
                let kind = self.u8()?;
                let rank = self.i32()?;
                if rank != 1 || !matches!(kind, 0 | 1) {
                    return Err(format!("unsupported BinaryArray (type {kind}, rank {rank})"));
                }
                let n = self.i32()?;
                if !(0..100_000_000).contains(&n) {
                    return Err("bad array length".into());
                }
                let etype = self.u8()?;
                let eadd = self.add(etype)?;
                let a = if etype == 0 {
                    let Add::Prim(pt) = eadd else { return Err("primitive array without type".into()) };
                    let vals = (0..n).map(|_| self.prim(pt)).collect::<Res<Vec<_>>>()?;
                    N::PArr(Rc::new(PArrN { id, ptype: pt, vals }))
                } else {
                    let items = self.slots(n as usize, |_| None)?;
                    N::Arr(Rc::new(ArrN { id, etype, eadd, items }))
                };
                self.index.insert(id, a.clone());
                a
            }
            t => return Err(format!("unsupported record type {t} at offset {}", self.p - 1)),
        }))
    }
}

/// Parses a whole stream (header ... MessageEnd). BinaryFormatter writes referenced objects after the object that
/// uses them (`MemberReference`), so records are read until MessageEnd and indexed by object id.
pub fn parse(bytes: &[u8]) -> Res<Stream> {
    let mut r = Rd { b: bytes, p: 0, root: 0, libs: HashMap::new(), classes: HashMap::new(), index: HashMap::new() };
    let mut first = None;
    loop {
        match r.record()? {
            Rec::End => break,
            Rec::Node(n) => {
                first.get_or_insert(n);
            }
            Rec::Nulls(_) => {}
        }
    }
    let root = r.index.get(&r.root).cloned().or(first).ok_or("empty stream")?;
    Ok(Stream { root, libs: r.libs, index: r.index })
}

/// Name of a primitive type code (debugging).
pub fn prim_name(t: u8) -> &'static str {
    match t {
        1 => "bool",
        2 => "byte",
        3 => "char",
        5 => "decimal",
        6 => "double",
        7 => "short",
        8 => "int",
        9 => "long",
        10 => "sbyte",
        11 => "float",
        12 => "timespan",
        13 => "datetime",
        14 => "ushort",
        15 => "uint",
        16 => "ulong",
        _ => "?",
    }
}

/// Text summary of the stream (debugging): class definitions with member types and instance counts.
pub fn summary(s: &Stream) -> String {
    let mut classes: HashMap<String, (usize, Rc<ClassMeta>)> = HashMap::new();
    fn walk(n: &N, c: &mut HashMap<String, (usize, Rc<ClassMeta>)>, seen: &mut std::collections::HashSet<usize>) {
        match n {
            N::Obj(o) => {
                if !seen.insert(Rc::as_ptr(o) as usize) {
                    return;
                }
                c.entry(o.meta.name.clone()).or_insert((0, o.meta.clone())).0 += 1;
                o.vals.iter().for_each(|v| walk(v, c, seen));
            }
            N::Arr(a) => a.items.iter().for_each(|v| walk(v, c, seen)),
            _ => {}
        }
    }
    let _ = walk;
    for n in s.index.values() {
        if let N::Obj(o) = n {
            classes.entry(o.meta.name.clone()).or_insert((0, o.meta.clone())).0 += 1;
        }
    }
    let mut arrays: HashMap<String, usize> = HashMap::new();
    for n in s.index.values() {
        match n {
            N::PArr(a) => *arrays.entry(format!("PrimitiveArray<{}>", prim_name(a.ptype))).or_default() += 1,
            N::Arr(a) => *arrays.entry(format!("Array etype={} add={:?}", a.etype, a.eadd)).or_default() += 1,
            _ => {}
        }
    }
    let mut names: Vec<_> = classes.into_iter().collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = format!("{} libraries: {:?}\n", s.libs.len(), s.libs.values().collect::<Vec<_>>());
    let mut av: Vec<_> = arrays.into_iter().collect();
    av.sort();
    for (k, c) in av {
        out += &format!("array {k} x{c}
");
    }
    for (name, (count, m)) in names {
        out += &format!("class {name} x{count} lib={:?}\n", m.lib);
        for i in 0..m.names.len() {
            let t = match &m.add[i] {
                Add::Prim(p) => format!("{}{}", if m.bt[i] == 7 { "[]" } else { "" }, prim_name(*p)),
                Add::Sys(n) => format!("sys {n}"),
                Add::Class(n, l) => format!("class {n} lib{l}"),
                Add::None => ["", "string", "object", "", "", "object[]", "string[]", ""][m.bt[i] as usize].to_string(),
            };
            out += &format!("    {} : {}\n", m.names[i], t);
        }
    }
    out
}

/// Kinds of the member values of the root object (debugging): shows which members are inline and which are references.
pub fn root_shape(s: &Stream) -> String {
    let mut out = String::new();
    if let N::Obj(o) = &s.root {
        for (i, v) in o.vals.iter().enumerate() {
            let k = match v {
                N::Null => "null".to_string(),
                N::P(p) => format!("prim {p:?}"),
                N::Str(id, x) => format!("string#{id} {:?}", &x[..x.len().min(20)]),
                N::Ref(id) => format!("ref#{id}"),
                N::Obj(o) => format!("inline object#{} {}", o.id, o.meta.name),
                N::PArr(a) => format!("inline prim array#{}", a.id),
                N::Arr(a) => format!("inline array#{}", a.id),
            };
            out += &format!("{:>3} {:<28} {k}
", i, o.meta.names[i]);
        }
    }
    out
}
