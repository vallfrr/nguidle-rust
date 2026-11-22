//! Virtual machine: type registry, calls, execution loop, exceptions.
use crate::code::*;
use crate::cooking_score::{Recipe, ScoreFields};
use crate::program::*;
use crate::value::*;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default, Clone, Copy)]
pub struct FxHasher(u64);
impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0.rotate_left(5) ^ *b as u64).wrapping_mul(0x517cc1b727220a95);
        }
    }
    fn write_u32(&mut self, i: u32) {
        self.0 = (self.0.rotate_left(5) ^ i as u64).wrapping_mul(0x517cc1b727220a95);
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517cc1b727220a95);
    }
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}
pub type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
use std::rc::Rc;

pub(crate) fn field_ref(vm: &Vm, name: &str, field: &str) -> Option<(u32, usize)> {
    let ty = *vm.type_ids.get(name)?;
    let slot = vm.types[ty as usize].fields.iter().position(|name| name == field)?;
    Some((ty, slot))
}

pub(crate) fn read_field(value: &Value, (ty, slot): (u32, usize)) -> Option<Value> {
    let Value::Obj(obj) = value else { return None };
    if obj.ty != ty { return None; }
    obj.f.try_borrow().ok()?.get(slot).cloned()
}

struct WishTickFields {
    caller: u32,
    character: (u32, usize),
    wishes: (u32, usize),
    entries: (u32, usize),
    wish_type: u32,
    resources: [usize; 3],
}

impl WishTickFields {
    fn resolve(vm: &Vm) -> Option<Self> {
        let field = |name, field| field_ref(vm, name, field);
        let character = field("WishesController", "character")?;
        let (wish_type, energy) = field("Wish", "energy")?;
        Some(Self {
            caller: vm.find_method(character.0, "updateAllWishes", 1)?,
            character,
            wishes: field("Character", "wishes")?,
            entries: field("Wishes", "wishes")?,
            wish_type,
            resources: [energy, field("Wish", "magic")?.1, field("Wish", "res3")?.1],
        })
    }

    fn entries(&self, controller: &Value) -> Option<Rc<Obj>> {
        let character = read_field(controller, self.character)?;
        let wishes = read_field(&character, self.wishes)?;
        let Value::Obj(entries) = read_field(&wishes, self.entries)? else { return None };
        Some(entries)
    }

    fn unallocated(&self, wish: &Value) -> bool {
        let Value::Obj(wish) = wish else { return false };
        if wish.ty != self.wish_type { return false; }
        wish.f.try_borrow().is_ok_and(|fields| {
            self.resources.iter().any(|&slot| matches!(fields.get(slot), Some(Value::I64(0))))
        })
    }

    fn no_progress(&self, stack: &[Value], caller: Option<&u32>) -> bool {
        if caller != Some(&self.caller) { return false; }
        let [controller, Value::I32(index), ..] = stack else { return false };
        let Some(entries) = self.entries(controller) else { return false };
        let Ok(entries) = entries.x.try_borrow() else { return false };
        let Ext::List(entries) = &*entries else { return false };
        entries.get(*index as usize).is_some_and(|wish| self.unallocated(wish))
    }

    fn all_idle(&self, controller: &Value) -> Option<bool> {
        let entries = self.entries(controller)?;
        let entries = entries.x.try_borrow().ok()?;
        let Ext::List(entries) = &*entries else { return None };
        Some(entries.iter().all(|wish| self.unallocated(wish)))
    }
}

struct TitanChecks {
    scope: u32,
    character: Rc<Obj>,
    enabled: bool,
    stats: [Option<f32>; 3],
}

pub struct Vm {
    pub prog: Rc<Program>,
    pub types: Vec<TypeInfo>,
    pub type_ids: HashMap<String, u32>,
    pub n_user: u32,
    pub statics: Vec<Vec<Value>>,
    pub slots: Vec<Value>,
    pub vcache: FxMap<(u32, u32), u32>,
    pub scratch: Vec<Vec<Value>>,
    pub targs: Option<Rc<Vec<Ty>>>,
    pub missing: HashSet<String>,
    pub depth: usize,
    /// Engine (Unity) data accessible to intrinsics.
    pub eng: crate::unity::Engine,
    pub steps: u64,
    pub cur_exc: Vec<Value>,
    pub callstack: Vec<u32>,
    /// `CLR_PROF`: per method (calls, inclusive instructions, exclusive instructions)
    pub prof: Option<HashMap<u32, (u64, u64, u64)>>,
    prof_child: u64,
    pub last_meta: Option<Rc<ExtMeta>>,
    /// Native mode: the game logic runs as the generated Rust (ngu-gen), not in the interpreter.
    pub native_inv: Option<fn(*mut Vm, u32, &[Value]) -> Option<R<Value>>>,
    pub native_mode: bool,
    wish_tick_fields: Option<WishTickFields>,
    titan_check_fields: Option<[(u32, usize); 3]>,
    titan_checks: Option<TitanChecks>,
    score_fields: Option<ScoreFields>,
    optimal_score: Option<(Recipe, f32)>,
    achievement_stats: Option<(u32, [Option<Value>; 6])>,
    effective_boss_fields: Option<[(u32, usize); 3]>,
}

impl Vm {
    pub fn new(path: &str) -> Vm {
        let (prog, types) = Program::load(path);
        Vm::from_program(prog, types)
    }

    pub fn from_bytes(data: Vec<u8>) -> Vm {
        let (prog, types) = Program::load_bytes(data);
        Vm::from_program(prog, types)
    }

    fn from_program(mut prog: Program, types: Vec<TypeInfo>) -> Vm {
        prog.fix_fslot(&types);
        let n_user = types.len() as u32;
        let mut type_ids = HashMap::new();
        for (i, t) in types.iter().enumerate() {
            type_ids.insert(t.name.clone(), i as u32);
        }
        let statics = types.iter().map(|_| vec![]).collect();
        let mut vm = Vm {
            prog: Rc::new(prog),
            types,
            type_ids,
            n_user,
            statics,
            slots: Vec::with_capacity(1 << 16),
            vcache: FxMap::default(),
            scratch: vec![],
            targs: None,
            missing: HashSet::new(),
            depth: 0,
            eng: crate::unity::Engine::default(),
            steps: 0,
            cur_exc: vec![],
            callstack: vec![],
            last_meta: None,
            native_inv: None,
            native_mode: false,
            wish_tick_fields: None,
            titan_check_fields: None,
            titan_checks: None,
            score_fields: None,
            optimal_score: None,
            achievement_stats: None,
            effective_boss_fields: None,
            prof: if std::env::var("CLR_PROF").is_ok() { Some(HashMap::new()) } else { None },
            prof_child: 0,
        };
        for i in 0..vm.types.len() {
            vm.init_statics(i as u32);
        }
        vm.wish_tick_fields = WishTickFields::resolve(&vm);
        vm.score_fields = ScoreFields::resolve(&vm);
        vm.titan_check_fields = (|| Some([
            field_ref(&vm, "AdventureController", "character")?,
            field_ref(&vm, "Character", "settings")?,
            field_ref(&vm, "PlayerSettings", "autoKillTitans")?,
        ]))();
        vm.effective_boss_fields = (|| Some([
            field_ref(&vm, "Character", "bossID")?,
            field_ref(&vm, "Character", "settings")?,
            field_ref(&vm, "PlayerSettings", "rebirthDifficulty")?,
        ]))();
        vm
    }

    fn init_statics(&mut self, t: u32) {
        let tys = self.types[t as usize].static_tys.clone();
        let v: Vec<Value> = tys.iter().map(|ty| self.default_value(ty)).collect();
        self.statics[t as usize] = v;
    }

    /// Id of a type by name (creates a native type if needed).
    pub fn type_id(&mut self, name: &str) -> u32 {
        if let Some(&i) = self.type_ids.get(name) {
            return i;
        }
        let (fields, ftys_own) = crate::bcl::native_layout(name);
        let parent = crate::bcl::native_parent(name);
        let base = parent.map(|p| self.type_id(p));
        let id = self.types.len() as u32;
        let mut all: Vec<String> = base.map(|b| self.types[b as usize].fields.clone()).unwrap_or_default();
        let mut ftys: Vec<Ty> = base.map(|b| self.types[b as usize].field_tys.clone()).unwrap_or_default();
        for (f, t) in fields.iter().zip(ftys_own) {
            if !all.iter().any(|x| x == f) {
                all.push(f.to_string());
                ftys.push(t);
            }
        }
        let nf = all.len();
        self.types.push(TypeInfo {
            name: name.to_string(),
            user: None,
            base,
            ifaces: vec![],
            is_value: crate::bcl::is_native_struct(name),
            is_enum: false,
            is_iface: false,
            fields: all,
            field_elems: vec![None; nf],
            field_skip: vec![false; nf],
            field_tys: ftys,
            statics: HashMap::new(),
            static_tys: vec![],
            methods: HashMap::new(),
            cctor: None,
            cctor_state: 1,
        });
        self.statics.push(vec![]);
        self.type_ids.insert(name.to_string(), id);
        id
    }

    pub fn default_value(&mut self, ty: &Ty) -> Value {
        match ty {
            Ty::Bool | Ty::Char | Ty::I8 | Ty::U8 | Ty::I16 | Ty::U16 | Ty::I32 | Ty::U32 => Value::I32(0),
            Ty::I64 | Ty::U64 | Ty::IntPtr | Ty::UIntPtr => Value::I64(0),
            Ty::F32 => Value::F32(0.0),
            Ty::F64 => Value::F64(0.0),
            Ty::User(id) => {
                let t = &self.types[*id as usize];
                if t.is_enum {
                    Value::I32(0)
                } else if t.is_value {
                    self.default_struct(*id)
                } else {
                    Value::Null
                }
            }
            Ty::Ext(n, true) => {
                if crate::bcl::is_native_struct(n) {
                    let id = self.type_id(n);
                    self.default_struct(id)
                } else if crate::bcl::is_native_enum(n) {
                    Value::I32(0)
                } else {
                    Value::Null
                }
            }
            _ => Value::Null,
        }
    }

    pub fn default_struct(&mut self, id: u32) -> Value {
        let tys = self.types[id as usize].field_tys.clone();
        let f = tys.iter().map(|t| self.default_value(t)).collect();
        // native floating-point fields
        Value::Struct(Box::new(Struct { ty: id, f }))
    }

    pub fn new_obj(&mut self, ty: u32) -> Rc<Obj> {
        let tys = self.types[ty as usize].field_tys.clone();
        let f: Vec<Value> = tys.iter().map(|t| self.default_value(t)).collect();
        Rc::new(Obj { ty, f: std::cell::RefCell::new(f), x: std::cell::RefCell::new(Ext::None) })
    }

    /// Instantiates a user type and runs its default constructor (field initializers).
    pub fn construct(&mut self, ty: u32) -> Rc<Obj> {
        let o = self.new_obj(ty);
        if self.native_mode {
            return o;
        }
        let ctor = self.types[ty as usize].methods.get(".ctor").and_then(|v| v.iter().copied().find(|&m| self.prog.methods[m as usize].argc == 1));
        if let Some(c) = ctor {
            if self.types[ty as usize].cctor_state == 0 {
                let _ = self.ensure_cctor(ty);
            }
            let _ = self.invoke(c, vec![Value::Obj(o.clone())]);
        }
        o
    }

    pub fn new_exc(&mut self, name: &str, msg: &str) -> Value {
        let id = self.type_id(name);
        let o = self.new_obj(id);
        if !o.f.borrow().is_empty() {
            o.f.borrow_mut()[0] = Value::str(msg);
        }
        let n = self.callstack.len();
        let tr: Vec<String> = self.callstack[n.saturating_sub(6)..]
            .iter()
            .rev()
            .map(|&m| {
                let mi = &self.prog.methods[m as usize];
                format!("{}::{}", self.types[mi.owner as usize].name, mi.name)
            })
            .collect();
        *o.x.borrow_mut() = Ext::Native(Box::new(tr.join(" <- ")));
        Value::Obj(o)
    }

    pub fn throw<T>(&mut self, name: &str, msg: &str) -> R<T> {
        Err(self.new_exc(name, msg))
    }

    pub fn is_subclass(&self, mut a: u32, b: u32) -> bool {
        loop {
            if a == b {
                return true;
            }
            let t = &self.types[a as usize];
            if t.ifaces.iter().any(|&i| self.is_subclass(i, b)) {
                return true;
            }
            match t.base {
                Some(x) => a = x,
                None => return false,
            }
        }
    }

    pub fn type_name_of(&self, v: &Value) -> String {
        match v {
            Value::Null => "null".into(),
            Value::I32(_) => "System.Int32".into(),
            Value::I64(_) => "System.Int64".into(),
            Value::F32(_) => "System.Single".into(),
            Value::F64(_) => "System.Double".into(),
            Value::Str(_) => "System.String".into(),
            Value::Obj(o) => self.types[o.ty as usize].name.clone(),
            Value::Struct(s) => self.types[s.ty as usize].name.clone(),
            Value::Ptr(_) => "ref".into(),
        }
    }

    pub fn is_inst(&mut self, v: &Value, t: &TyOp) -> bool {
        match (v, t) {
            (Value::Null, _) => false,
            (_, TyOp::Any) => true,
            (Value::Obj(o), TyOp::Class(c)) => {
                if let Ext::Boxed(b) = &*o.x.borrow() {
                    let _ = b;
                }
                self.is_subclass(o.ty, *c) || self.native_assignable(o.ty, *c)
            }
            (Value::Str(_), TyOp::Class(c)) => {
                let n = self.types[*c as usize].name.as_str();
                matches!(n, "System.String" | "System.Object" | "System.IComparable" | "System.IEnumerable" | "System.IConvertible")
            }
            (Value::Struct(s), TyOp::Class(c)) => self.is_subclass(s.ty, *c) || self.types[*c as usize].name == "System.Object",
            (Value::Obj(o), TyOp::Array) => matches!(&*o.x.borrow(), Ext::Arr(_)),
            (Value::I32(_), TyOp::Prim(p)) => matches!(p, Ty::I32 | Ty::U32 | Ty::Bool | Ty::Char | Ty::I16 | Ty::U16 | Ty::I8 | Ty::U8 | Ty::Object),
            (Value::I64(_), TyOp::Prim(p)) => matches!(p, Ty::I64 | Ty::U64 | Ty::IntPtr | Ty::UIntPtr | Ty::Object),
            (Value::F32(_), TyOp::Prim(p)) => matches!(p, Ty::F32 | Ty::Object),
            (Value::F64(_), TyOp::Prim(p)) => matches!(p, Ty::F64 | Ty::Object),
            (Value::Str(_), TyOp::Prim(p)) => matches!(p, Ty::String | Ty::Object),
            (Value::Obj(_), TyOp::Prim(Ty::Object)) => true,
            (Value::Struct(_), TyOp::Prim(Ty::Object)) => true,
            (Value::I32(_) | Value::I64(_) | Value::F32(_) | Value::F64(_), TyOp::Class(c)) => {
                let n = self.types[*c as usize].name.as_str();
                matches!(n, "System.Object" | "System.ValueType" | "System.IComparable" | "System.IConvertible" | "System.Enum")
            }
            _ => false,
        }
    }

    /// Inheritance between native types (lists of known interfaces).
    pub fn native_assignable(&self, a: u32, b: u32) -> bool {
        let an = &self.types[a as usize].name;
        let bn = &self.types[b as usize].name;
        if bn == "System.Object" {
            return true;
        }
        crate::bcl::native_implements(an, bn)
    }

    // ---------------------------------------------------------------- pointers
    pub fn with_ptr(&mut self, p: &Ptr, f: &mut dyn FnMut(&mut Value)) {
        match p {
            Ptr::Slot(i) => f(&mut self.slots[*i]),
            Ptr::Field(o, i) => f(&mut o.f.borrow_mut()[*i as usize]),
            Ptr::Elem(o, i) => match &mut *o.x.borrow_mut() {
                Ext::Arr(a) | Ext::List(a) => f(&mut a[*i as usize]),
                _ => {}
            },
            Ptr::Static(t, i) => f(&mut self.statics[*t as usize][*i as usize]),
            Ptr::SField(par, i) => {
                let i = *i as usize;
                self.with_ptr(par, &mut |v| match v {
                    Value::Struct(s) => f(&mut s.f[i]),
                    Value::Obj(o) => f(&mut o.f.borrow_mut()[i]),
                    Value::Ptr(_) => {}
                    _ => {}
                });
            }
        }
    }

    pub fn load_ptr(&mut self, p: &Ptr) -> Value {
        let mut out = Value::Null;
        self.with_ptr(p, &mut |v| out = v.clone());
        // ref of ref: transitive deref
        if let Value::Ptr(inner) = &out {
            if !matches!(p, Ptr::Slot(_)) {
                let inner = inner.clone();
                return self.load_ptr(&inner);
            }
        }
        out
    }

    pub fn store_ptr(&mut self, p: &Ptr, val: Value) {
        let mut val = Some(val);
        self.with_ptr(p, &mut |v| *v = val.take().unwrap());
    }

    /// "Dereferenced" value of a receiver (Ptr -> value).
    pub fn deref(&mut self, v: Value) -> Value {
        match v {
            Value::Ptr(p) => {
                let x = self.load_ptr(&p);
                if let Value::Ptr(_) = x { self.deref(x) } else { x }
            }
            o => o,
        }
    }

    // ---------------------------------------------------------------- calls
    pub fn ensure_cctor(&mut self, ty: u32) -> R<()> {
        if self.types[ty as usize].cctor_state != 0 {
            return Ok(());
        }
        self.types[ty as usize].cctor_state = 1;
        if let Some(b) = self.types[ty as usize].base {
            self.ensure_cctor(b)?;
        }
        if let Some(c) = self.types[ty as usize].cctor {
            self.invoke(c, vec![])?;
        }
        Ok(())
    }

    pub fn invoke(&mut self, mid: u32, args: impl AsRef<[Value]> + IntoIterator<Item = Value>) -> R<Value> {
        if self.trace_calls() {
            let mi = &self.prog.methods[mid as usize];
            eprintln!("[call] {}::{}", self.types[mi.owner as usize].name, mi.name);
        }
        if let Some(f) = self.native_inv {
            if let Some(r) = f(self as *mut Vm, mid, args.as_ref()) {
                return r;
            }
        }
        let base = self.slots.len();
        self.slots.extend(args);
        self.call_at(mid, base)
    }

    /// Native call trace (NGU_TRACE=1), same format as `ngu_host::native`: used to compare oracle and native mode.
    fn trace_native(&self, code: &Code, pc: usize, argc: usize) {
        static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !*T.get_or_init(|| std::env::var("NGU_TRACE").is_ok()) {
            return;
        }
        let key = match code.ext.get(pc.wrapping_sub(1)).and_then(|m| m.as_ref()) {
            Some(m) => m.key.clone(),
            None => "?".to_string(),
        };
        let at = self.slots.len() - argc;
        let d: Vec<String> = self.slots[at..].iter().map(|v| self.describe(v)).collect();
        eprintln!("[native] {key} {}", d.join(", "));
    }
    pub fn describe(&self, v: &Value) -> String {
        match v {
            Value::Null => "null".to_string(),
            Value::I32(i) => format!("i32:{i}"),
            Value::I64(i) => format!("i64:{i}"),
            Value::F32(f) => format!("f32:{f}"),
            Value::F64(f) => format!("f64:{f}"),
            Value::Str(s) => format!("str:{:?}", s.chars().take(24).collect::<String>()),
            Value::Obj(o) => format!("obj:{}", self.types[o.ty as usize].name),
            Value::Struct(_) => "struct".to_string(),
            Value::Ptr(_) => "ptr".to_string(),
        }
    }

    fn trace_calls(&self) -> bool {
        static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *T.get_or_init(|| std::env::var("NGU_TRACE_CALLS").is_ok())
    }

    /// Looks up a method by name/arity in a type and its parents.
    pub fn find_method(&self, mut ty: u32, name: &str, argc: usize) -> Option<u32> {
        loop {
            if let Some(v) = self.types[ty as usize].methods.get(name) {
                for &m in v {
                    if self.prog.methods[m as usize].argc == argc && !self.prog.methods[m as usize].abstract_ {
                        return Some(m);
                    }
                }
            }
            match self.types[ty as usize].base {
                Some(b) => ty = b,
                None => return None,
            }
        }
    }

    /// Resolution of a virtual call on a user method.
    pub fn resolve_virtual(&mut self, decl: u32, recv_ty: u32) -> u32 {
        if let Some(&m) = self.vcache.get(&(recv_ty, decl)) {
            return m;
        }
        let d = &self.prog.methods[decl as usize];
        let (name, argc, downer) = (d.name.clone(), d.argc, d.owner);
        let mut found = None;
        // explicit MethodImpl (interfaces)
        let mut t = Some(recv_ty);
        'o: while let Some(ty) = t {
            if let Some(list) = self.prog.overrides.get(&ty) {
                for &(dc, im) in list {
                    if dc == decl {
                        found = Some(im);
                        break 'o;
                    }
                }
            }
            t = self.types[ty as usize].base;
        }
        if found.is_none() {
            // most derived first, by name + arity
            let mut t = Some(recv_ty);
            while let Some(ty) = t {
                if let Some(v) = self.types[ty as usize].methods.get(&name) {
                    for &m in v {
                        let mi = &self.prog.methods[m as usize];
                        if mi.argc == argc && !mi.is_static && !mi.abstract_ && (mi.virt || ty == downer) {
                            found = Some(m);
                            break;
                        }
                    }
                }
                if found.is_some() || ty == downer && !self.types[downer as usize].is_iface {
                    break;
                }
                t = self.types[ty as usize].base;
            }
        }
        let m = found.unwrap_or(decl);
        self.vcache.insert((recv_ty, decl), m);
        m
    }

    pub fn call_at(&mut self, mid: u32, base: usize) -> R<Value> {
        // the program and the code live as long as the VM: no Rc clone
        let prog: &Program = unsafe { &*(&*self.prog as *const Program) };
        let mi = &prog.methods[mid as usize];
        let guarded = mi.guard != 0 && !no_guard();
        if guarded {
            let native = match mi.guard {
                18 => self.effective_boss_id(base),
                // This Update only handles the A/D shortcuts on equipped items.
                19 if self.slots.get(base).and_then(Value::as_obj).is_some()
                    && !self.eng.keys_edge.contains(&97) && !self.eng.keys_edge.contains(&100) =>
                    Some(Value::Null),
                _ => None,
            };
            if let Some(value) = native {
                self.slots.truncate(base);
                return Ok(value);
            }
        }
        let code: *const Code = match mi.code.get() {
            Some(c) => &**c,
            None => {
                let c = self.lower(mid);
                let _ = mi.code.set(c);
                &**mi.code.get().unwrap()
            }
        };
        let code: &Code = unsafe { &*code };
        // The original Cooking search depends on the current recipe, not the
        // amount the player has added. Retain the exact score until it changes.
        let recipe = if guarded && mi.guard == 8 {
            self.score_fields.as_ref().and_then(|fields| fields.recipe(self.slots.get(base)?))
        } else {
            None
        };
        let mut titan_stat = None;
        let mut achievement_stat = None;
        if guarded {
            let result = match mi.guard {
                1 if self.guard_skip(mi.guard, base) => Some(Value::Null),
                // A zero allocation cannot advance a wish. The update loop checks for a
                // positive value. Other callers still receive the exact formula result.
                2 if self.wish_tick_fields.as_ref().is_some_and(|fields| {
                    fields.no_progress(&self.slots[base..], self.callstack.last())
                }) => Some(Value::F32(0.0)),
                3 if self.wish_tick_fields.as_ref().and_then(|fields| {
                    fields.all_idle(self.slots.get(base)?)
                }) == Some(true) => Some(Value::Null),
                5 | 6 if self.is_titan_stat_eligibility(base) => {
                    let stat = (mi.guard - 5) as usize;
                    let checks = self.titan_checks.as_ref().unwrap();
                    if checks.enabled {
                        titan_stat = Some(stat);
                        checks.stats[stat].map(Value::F32)
                    } else {
                        Some(Value::F32(0.0))
                    }
                }
                7 if self.titan_checks.as_ref().is_some_and(|checks| {
                    !checks.enabled && self.callstack.last() == Some(&checks.scope)
                }) => Some(Value::I32(0)),
                8 => recipe.as_ref().and_then(|recipe| {
                    self.optimal_score.as_ref().filter(|(key, _)| key == recipe).map(|(_, score)| Value::F32(*score))
                }),
                // In manageFight, the health clamp can read the same maximum twice.
                9 if self.is_titan_stat_eligibility(base) => {
                    titan_stat = Some(2);
                    self.titan_checks.as_ref().unwrap().stats[2].map(Value::F32)
                }
                12..=17 => {
                    self.achievement_stats.as_ref().and_then(|(scope, stats)| {
                        let [.., scan, condition] = self.callstack.as_slice() else { return None };
                        if scan != scope || prog.methods[*condition as usize].guard != 11 {
                            return None;
                        }
                        let stat = (mi.guard - 12) as usize;
                        achievement_stat = Some(stat);
                        stats[stat].clone()
                    })
                }
                _ => None,
            };
            if let Some(value) = result {
                self.slots.truncate(base);
                return Ok(value);
            }
        }
        if self.depth > 600 {
            self.slots.truncate(base);
            return self.throw("System.StackOverflowException", "depth");
        }
        // Each successful Titan auto-kill returns immediately. Until then, its
        // eligibility checks read the same attack and defense for this character.
        let previous_checks = if guarded && mi.guard == 4 {
            let checks = self.begin_titan_checks(mid, base);
            Some(std::mem::replace(&mut self.titan_checks, checks))
        } else {
            None
        };
        // Each original achievement threshold uses the same resource totals during
        // this scan. Keep the original formulas and the order of unlocks.
        let previous_achievement_stats = (guarded && mi.guard == 10)
            .then(|| self.achievement_stats.replace((mid, [const { None }; 6])));
        self.depth += 1;
        self.callstack.push(mid);
        if crate::vm::trace_m() {
            eprintln!("[m] {}::{}", self.types[mi.owner as usize].name, mi.name);
        }
        while self.slots.len() < base + code.nargs {
            self.slots.push(Value::Null);
        }
        if code.nlocals > 0 {
            if code.local_vals.borrow().is_none() {
                let vals: Vec<Value> = code.local_defaults.iter().map(|t| self.default_value(t)).collect();
                *code.local_vals.borrow_mut() = Some(vals);
            }
            let lv = code.local_vals.borrow();
            self.slots.extend_from_slice(lv.as_ref().unwrap());
        }
        let s0 = self.steps;
        let outer_child = std::mem::take(&mut self.prof_child);
        let r = self.run(code, base);
        if let (Some(recipe), Ok(Value::F32(score))) = (recipe, &r) {
            self.optimal_score = Some((recipe, *score));
        }
        if let (Some(stat), Ok(Value::F32(value))) = (titan_stat, &r) {
            if let Some(checks) = &mut self.titan_checks {
                checks.stats[stat] = Some(*value);
            }
        }
        if let Some(previous) = previous_checks {
            self.titan_checks = previous;
        }
        if let (Some(stat), Ok(value)) = (achievement_stat, &r) {
            self.achievement_stats.as_mut().unwrap().1[stat] = Some(value.clone());
        }
        if let Some(previous) = previous_achievement_stats {
            self.achievement_stats = previous;
        }
        if let Some(p) = &mut self.prof {
            let incl = self.steps - s0;
            let e = p.entry(mid).or_insert((0, 0, 0));
            e.0 += 1;
            e.1 += incl;
            e.2 += incl - self.prof_child;
            self.prof_child = outer_child + incl;
        } else {
            self.prof_child = outer_child;
        }
        self.callstack.pop();
        self.depth -= 1;
        self.slots.truncate(base);
        r
    }

    fn effective_boss_id(&self, base: usize) -> Option<Value> {
        let [boss, settings, difficulty] = self.effective_boss_fields?;
        let character = self.slots.get(base)?;
        let Value::I32(mut boss_id) = read_field(character, boss)? else { return None };
        let settings = read_field(character, settings)?;
        let Value::I32(difficulty) = read_field(&settings, difficulty)? else { return None };
        // Normal is 0, Evil is 1, Sadistic is 2. IL uses wrapping addition.
        boss_id = boss_id.wrapping_add(301 * (i32::from(difficulty >= 1) + i32::from(difficulty >= 2)));
        Some(Value::I32(boss_id))
    }

    fn begin_titan_checks(&self, scope: u32, base: usize) -> Option<TitanChecks> {
        let [controller_player, player_settings, setting] = self.titan_check_fields?;
        let character = read_field(self.slots.get(base)?, controller_player)?;
        let settings = read_field(&character, player_settings)?;
        let Value::I32(enabled) = read_field(&settings, setting)? else { return None };
        let Value::Obj(character) = character else { return None };
        Some(TitanChecks { scope, character, enabled: enabled != 0, stats: [None; 3] })
    }

    fn is_titan_stat_eligibility(&self, base: usize) -> bool {
        let Some(checks) = &self.titan_checks else { return false };
        let Some(Value::Obj(character)) = self.slots.get(base) else { return false };
        if !Rc::ptr_eq(character, &checks.character) { return false; }
        match self.callstack.as_slice() {
            [.., current] if *current == checks.scope => true,
            [.., parent, current] => *parent == checks.scope && self.prog.methods[*current as usize].guard == 7,
            _ => false,
        }
    }

    /// Optimisation with no visible effect: UI refresh of a hidden menu.
    fn guard_skip(&mut self, kind: u8, base: usize) -> bool {
        let this = match self.slots.get(base) {
            Some(Value::Obj(o)) => o.clone(),
            _ => return false,
        };
        match kind {
            1 => {
                // CookingController.updateDishUI: useless while the Cooking menu (id 57) is not displayed
                let ty = this.ty as usize;
                let cs = match self.types[ty].fields.iter().position(|n| n == "character") {
                    Some(s) => s,
                    None => return false,
                };
                let ch = this.f.borrow()[cs].clone();
                if let Value::Obj(c) = ch {
                    let cty = c.ty as usize;
                    if let Some(ms) = self.types[cty].fields.iter().position(|n| n == "menuID") {
                        return c.f.borrow()[ms].as_i32() != 57;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn pop(&mut self) -> Value {
        self.slots.pop().unwrap()
    }

    fn call_native_args(&mut self, f: NativeFn, argc: usize, ret: bool, targs: &Option<Rc<Vec<Ty>>>) -> R<()> {
        let at = self.slots.len() - argc;
        let mut args = self.scratch.pop().unwrap_or_default();
        args.extend(self.slots.drain(at..));
        self.targs = if targs.is_some() { targs.clone() } else { None };
        let r = f(self, &mut args);
        args.clear();
        self.scratch.push(args);
        let r = r?;
        if ret {
            self.slots.push(r);
        }
        Ok(())
    }

    fn null_check(&mut self, v: &Value) -> R<()> {
        if v.is_null() {
            return self.throw("System.NullReferenceException", "Object reference not set to an instance of an object");
        }
        Ok(())
    }

    pub fn call_delegate(&mut self, d: &Rc<Obj>, args: Vec<Value>) -> R<Value> {
        let list: Vec<Deleg> = match &*d.x.borrow() {
            Ext::Delegate(l) => l.clone(),
            _ => vec![],
        };
        let mut last = Value::Null;
        for dg in list {
            let m = &self.prog.methods[dg.method as usize];
            let (is_static, argc) = (m.is_static, m.argc);
            let mut a = vec![];
            if !is_static {
                a.push(dg.target.clone());
            } else if !dg.target.is_null() && argc == args.len() + 1 {
                a.push(dg.target.clone());
            }
            a.extend(args.iter().cloned());
            last = self.invoke(dg.method, a)?;
        }
        Ok(last)
    }

    // ---------------------------------------------------------------- loop
    fn run(&mut self, code: &Code, base: usize) -> R<Value> {
        let sb = base + code.nargs + code.nlocals;
        let mut pc = 0usize;
        // finally in progress: (Pending, state)
        let mut pending: Vec<Pending> = vec![];
        loop {
            let step: R<Option<Value>> = self.step(code, base, sb, &mut pc, &mut pending);
            match step {
                Ok(None) => {}
                Ok(Some(v)) => return Ok(v),
                Err(exc) => {
                    pc = pc.saturating_sub(1); // faulting instruction
                    if let (Some(d), Value::Obj(o)) = (&code.dbg, &exc) {
                        if let Ext::Native(b) = &mut *o.x.borrow_mut() {
                            if let Some(s) = b.downcast_mut::<String>() {
                                if !s.contains("{pc") {
                                    s.push_str(&format!(" {{pc {}: {}}}", pc, d.get(pc).cloned().unwrap_or_default()));
                                }
                            }
                        }
                    }
                    // handler lookup in this method
                    match self.find_handler(code, pc, &exc) {
                        Some((hi, is_catch)) => {
                            self.slots.truncate(sb);
                            let h = &code.eh[hi];
                            if is_catch {
                                self.cur_exc.push(exc.clone());
                                self.slots.push(exc);
                                pc = h.h_start;
                            } else {
                                pending.push(Pending::Exc(exc, hi));
                                pc = h.h_start;
                            }
                        }
                        None => return Err(exc),
                    }
                }
            }
        }
    }

    fn find_handler(&mut self, code: &Code, pc: usize, exc: &Value) -> Option<(usize, bool)> {
        for (i, h) in code.eh.iter().enumerate() {
            if pc >= h.try_start && pc < h.try_end {
                match &h.kind {
                    EhKind::Catch(t) => {
                        let t = t.clone();
                        if self.is_inst(exc, &t) {
                            return Some((i, true));
                        }
                    }
                    EhKind::Finally | EhKind::Fault => return Some((i, false)),
                    EhKind::Filter(_) => {}
                }
            }
        }
        None
    }

    /// Reads successful field loads directly from a local or argument, without
    /// pushing the intermediate objects to the evaluation stack.
    #[inline]
    fn load_slot(&mut self, code: &Code, slot: usize, pc: &mut usize) {
        fn read(value: &Value, field: u32) -> Option<Value> {
            match value {
                Value::Obj(object) => Some(object.f.borrow()[field as usize].clone()),
                Value::Struct(value) => Some(value.f[field as usize].clone()),
                _ => None,
            }
        }

        let source = &self.slots[slot];
        let Some(Op::Ldfld(field)) = code.ops.get(*pc) else {
            self.slots.push(source.clone());
            return;
        };
        let Some(mut value) = read(source, *field) else {
            self.slots.push(source.clone());
            return;
        };
        *pc += 1;
        self.steps += 1;
        while let Some(Op::Ldfld(field)) = code.ops.get(*pc) {
            // Leave pointer loads and exceptions to the regular opcode, at the
            // same instruction and with the same operand it would have seen.
            let Some(next) = read(&value, *field) else { break };
            value = next;
            *pc += 1;
            self.steps += 1;
        }
        self.slots.push(value);
    }

    /// Executes one instruction. `Ok(Some(v))` = return from the method.
    fn step(&mut self, code: &Code, base: usize, sb: usize, pc: &mut usize, pending: &mut Vec<Pending>) -> R<Option<Value>> {
        macro_rules! pop {
            () => {
                self.slots.pop().unwrap()
            };
        }
        macro_rules! push {
            ($v:expr) => {{
                let __v = $v;
                self.slots.push(__v)
            }};
        }
        loop {
            let op = unsafe { code.ops.get_unchecked(*pc) };
            let cur = *pc;
            *pc += 1;
            self.steps += 1;
            match op {
                Op::Nop => {}
                Op::Ldarg(i) => {
                    self.load_slot(code, base + *i as usize, pc);
                }
                Op::Starg(i) => {
                    let v = pop!();
                    self.slots[base + *i as usize] = v;
                }
                Op::Ldloc(i) => {
                    self.load_slot(code, base + code.nargs + *i as usize, pc);
                }
                Op::Stloc(i) => {
                    let v = pop!();
                    self.slots[base + code.nargs + *i as usize] = v;
                }
                Op::Ldarga(i) => push!(Value::Ptr(Rc::new(Ptr::Slot(base + *i as usize)))),
                Op::Ldloca(i) => push!(Value::Ptr(Rc::new(Ptr::Slot(base + code.nargs + *i as usize)))),
                Op::I4(v) => push!(Value::I32(*v)),
                Op::I8(v) => push!(Value::I64(*v)),
                Op::R4(v) => push!(Value::F32(*v)),
                Op::R8(v) => push!(Value::F64(*v)),
                Op::Null => push!(Value::Null),
                Op::Str(s) => push!(Value::Str(s.clone())),
                Op::Dup => {
                    let v = self.slots.last().unwrap().clone();
                    push!(v);
                }
                Op::Pop => {
                    pop!();
                }
                Op::Add => {
                    let b = pop!();
                    let a = pop!();
                    push!(arith(&a, &b, ArOp::Add));
                }
                Op::Sub => {
                    let b = pop!();
                    let a = pop!();
                    push!(arith(&a, &b, ArOp::Sub));
                }
                Op::Mul => {
                    let b = pop!();
                    let a = pop!();
                    push!(arith(&a, &b, ArOp::Mul));
                }
                Op::Div(u) => {
                    let b = pop!();
                    let a = pop!();
                    if matches!((&a, &b), (Value::I32(_) | Value::I64(_), Value::I32(0)) | (Value::I32(_) | Value::I64(_), Value::I64(0))) {
                        return self.throw("System.DivideByZeroException", "Attempted to divide by zero.");
                    }
                    push!(if *u { arith(&a, &b, ArOp::DivUn) } else { arith(&a, &b, ArOp::Div) });
                }
                Op::Rem(u) => {
                    let b = pop!();
                    let a = pop!();
                    if matches!((&a, &b), (Value::I32(_) | Value::I64(_), Value::I32(0)) | (Value::I32(_) | Value::I64(_), Value::I64(0))) {
                        return self.throw("System.DivideByZeroException", "Attempted to divide by zero.");
                    }
                    push!(if *u { arith(&a, &b, ArOp::RemUn) } else { arith(&a, &b, ArOp::Rem) });
                }
                Op::And => {
                    let b = pop!();
                    let a = pop!();
                    push!(arith(&a, &b, ArOp::And));
                }
                Op::Or => {
                    let b = pop!();
                    let a = pop!();
                    push!(arith(&a, &b, ArOp::Or));
                }
                Op::Xor => {
                    let b = pop!();
                    let a = pop!();
                    push!(arith(&a, &b, ArOp::Xor));
                }
                Op::Shl => {
                    let n = pop!().as_i32() as u32;
                    let a = pop!();
                    push!(match a {
                        Value::I64(x) => Value::I64(x.wrapping_shl(n)),
                        o => Value::I32(o.as_i32().wrapping_shl(n)),
                    });
                }
                Op::Shr(u) => {
                    let n = pop!().as_i32() as u32;
                    let a = pop!();
                    push!(match (a, *u) {
                        (Value::I64(x), false) => Value::I64(x.wrapping_shr(n)),
                        (Value::I64(x), true) => Value::I64((x as u64).wrapping_shr(n) as i64),
                        (o, false) => Value::I32(o.as_i32().wrapping_shr(n)),
                        (o, true) => Value::I32((o.as_i32() as u32).wrapping_shr(n) as i32),
                    });
                }
                Op::Neg => {
                    let a = pop!();
                    push!(match a {
                        Value::I32(x) => Value::I32(x.wrapping_neg()),
                        Value::I64(x) => Value::I64(x.wrapping_neg()),
                        Value::F32(x) => Value::F32(-x),
                        Value::F64(x) => Value::F64(-x),
                        o => o,
                    });
                }
                Op::Not => {
                    let a = pop!();
                    push!(match a {
                        Value::I64(x) => Value::I64(!x),
                        o => Value::I32(!o.as_i32()),
                    });
                }
                Op::AddOvf(u) | Op::SubOvf(u) | Op::MulOvf(u) => {
                    let b = pop!();
                    let a = pop!();
                    let o = match op {
                        Op::AddOvf(_) => ArOp::Add,
                        Op::SubOvf(_) => ArOp::Sub,
                        _ => ArOp::Mul,
                    };
                    // overflow -> exception
                    let ok = match (&a, &b) {
                        (Value::I32(x), Value::I32(y)) => {
                            if *u {
                                let (x, y) = (*x as u32, *y as u32);
                                match o { ArOp::Add => x.checked_add(y).is_some(), ArOp::Sub => x.checked_sub(y).is_some(), _ => x.checked_mul(y).is_some() }
                            } else {
                                match o { ArOp::Add => x.checked_add(*y).is_some(), ArOp::Sub => x.checked_sub(*y).is_some(), _ => x.checked_mul(*y).is_some() }
                            }
                        }
                        (Value::I64(x), Value::I64(y)) => {
                            if *u {
                                let (x, y) = (*x as u64, *y as u64);
                                match o { ArOp::Add => x.checked_add(y).is_some(), ArOp::Sub => x.checked_sub(y).is_some(), _ => x.checked_mul(y).is_some() }
                            } else {
                                match o { ArOp::Add => x.checked_add(*y).is_some(), ArOp::Sub => x.checked_sub(*y).is_some(), _ => x.checked_mul(*y).is_some() }
                            }
                        }
                        _ => true,
                    };
                    if !ok {
                        return self.throw("System.OverflowException", "Arithmetic operation resulted in an overflow.");
                    }
                    push!(arith(&a, &b, o));
                }
                Op::Ceq => {
                    let b = pop!();
                    let a = pop!();
                    push!(Value::bool(values_eq(&a, &b)));
                }
                Op::Cgt(u) => {
                    let b = pop!();
                    let a = pop!();
                    push!(Value::bool(compare(&a, &b, *u, Cmp::Gt)));
                }
                Op::Clt(u) => {
                    let b = pop!();
                    let a = pop!();
                    push!(Value::bool(compare(&a, &b, *u, Cmp::Lt)));
                }
                Op::Br(t) => *pc = *t,
                Op::Brtrue(t) => {
                    if pop!().as_bool() {
                        *pc = *t;
                    }
                }
                Op::Brfalse(t) => {
                    if !pop!().as_bool() {
                        *pc = *t;
                    }
                }
                Op::Beq(t) => {
                    let b = pop!();
                    let a = pop!();
                    if values_eq(&a, &b) {
                        *pc = *t;
                    }
                }
                Op::Bne(t) => {
                    let b = pop!();
                    let a = pop!();
                    if !values_eq(&a, &b) {
                        *pc = *t;
                    }
                }
                Op::Bge(u, t) => {
                    let b = pop!();
                    let a = pop!();
                    // a >= b  ==  !(a < b)  (except NaN: ordered bge -> false if NaN)
                    if compare(&a, &b, *u, Cmp::Ge) {
                        *pc = *t;
                    }
                }
                Op::Bgt(u, t) => {
                    let b = pop!();
                    let a = pop!();
                    if compare(&a, &b, *u, Cmp::Gt) {
                        *pc = *t;
                    }
                }
                Op::Ble(u, t) => {
                    let b = pop!();
                    let a = pop!();
                    if compare(&a, &b, *u, Cmp::Le) {
                        *pc = *t;
                    }
                }
                Op::Blt(u, t) => {
                    let b = pop!();
                    let a = pop!();
                    if compare(&a, &b, *u, Cmp::Lt) {
                        *pc = *t;
                    }
                }
                Op::Switch(ts) => {
                    let i = pop!().as_i32() as u32 as usize;
                    if i < ts.len() {
                        *pc = ts[i];
                    }
                }
                Op::Conv(t) => {
                    let a = pop!();
                    push!(convert(&a, t));
                }
                Op::ConvOvf(t, _u) => {
                    let a = pop!();
                    push!(convert(&a, t));
                }
                Op::ConvRUn => {
                    let a = pop!();
                    push!(match a {
                        Value::I32(x) => Value::F64(x as u32 as f64),
                        Value::I64(x) => Value::F64(x as u64 as f64),
                        o => Value::F64(o.as_f64()),
                    });
                }
                Op::Call(mid) => {
                    let (is_static, owner, argc, ret, inst) = {
                        let m = &self.prog.methods[*mid as usize];
                        (m.ctor_or_static, m.owner, m.argc, m.has_ret, !m.is_static)
                    };
                    if is_static && self.types[owner as usize].cctor_state == 0 {
                        self.ensure_cctor(owner)?;
                    }
                    let at = self.slots.len() - argc;
                    if inst && matches!(self.slots[at], Value::Null) {
                        return self.throw("System.NullReferenceException", "Object reference not set to an instance of an object");
                    }
                    let v = self.call_at(*mid, at)?;
                    if ret {
                        push!(v);
                    }
                }
                Op::CallVirtUser(decl) => {
                    let argc = self.prog.methods[*decl as usize].argc;
                    let at = self.slots.len() - argc;
                    let mut recv = self.slots[at].clone();
                    self.null_check(&recv)?;
                    if let Value::Ptr(_) = recv {
                        let tgt_owner = self.prog.methods[*decl as usize].owner;
                        let _ = tgt_owner;
                        recv = self.deref(recv);
                        // value type: pass the address if the method is in a struct, else the value
                        if !matches!(recv, Value::Struct(_)) || !self.types[self.prog.methods[*decl as usize].owner as usize].is_value {
                            self.slots[at] = recv.clone();
                        }
                    }
                    let rty = match &recv {
                        Value::Obj(o) => o.ty,
                        Value::Struct(s) => s.ty,
                        _ => self.prog.methods[*decl as usize].owner,
                    };
                    let target = self.resolve_virtual(*decl, rty);
                    let ret = self.prog.methods[target as usize].has_ret;
                    let v = self.call_at(target, at)?;
                    if ret {
                        push!(v);
                    }
                }
                Op::CallVirtExt { name, argc, f, ret, targs } => {
                    let at = self.slots.len() - *argc;
                    let recv = self.slots[at].clone();
                    self.null_check(&recv)?;
                    // user object? -> dispatch by name
                    let user_ty = match &self.deref_peek(&recv) {
                        Value::Obj(o) if o.ty < self.n_user || self.types[o.ty as usize].user.is_some() => Some(o.ty),
                        Value::Struct(s) if s.ty < self.n_user => Some(s.ty),
                        _ => None,
                    };
                    if let Some(ty) = user_ty {
                        if let Some(m) = self.find_virtual_by_name(ty, name, *argc) {
                            let r = self.prog.methods[m as usize].has_ret;
                            let v = self.call_at(m, at)?;
                            if r {
                                push!(v);
                            }
                            continue;
                        }
                    }
                    self.trace_native(code, *pc, *argc);
                    self.call_native_args(*f, *argc, *ret, targs)?;
                }
                Op::CallNative { f, argc, ret, targs } => {
                    self.trace_native(code, *pc, *argc);
                    self.call_native_args(*f, *argc, *ret, targs)?
                }
                Op::CallStructCtor { f, argc } => {
                    let at = self.slots.len() - *argc;
                    let mut args = self.slots.split_off(at);
                    let this = args.remove(0);
                    let v = f(self, &mut args)?;
                    if let Value::Ptr(p) = this {
                        self.store_ptr(&p, v);
                    }
                }
                Op::CallMissing { key, argc, ret } => {
                    let at = self.slots.len() - *argc;
                    self.slots.truncate(at);
                    if self.missing.insert(key.to_string()) {
                        crate::elog!("[clr] missing intrinsic: {}", key);
                    }
                    let _ = ret;
                    return self.throw("System.NotImplementedException", key);
                }
                Op::DelegateInvoke { argc, ret } => {
                    let at = self.slots.len() - *argc;
                    let mut args = self.slots.split_off(at);
                    let d = args.remove(0);
                    let d = self.deref(d);
                    match d {
                        Value::Obj(o) => {
                            let v = self.call_delegate(&o, args)?;
                            if *ret {
                                push!(v);
                            }
                        }
                        _ => return self.throw("System.NullReferenceException", "delegate null"),
                    }
                }
                Op::Ret => {
                    let has = self.slots.len() > sb;
                    let v = if has { pop!() } else { Value::Null };
                    return Ok(Some(v));
                }
                Op::NewObj { ctor, argc, ty } => {
                    if self.types[*ty as usize].cctor_state == 0 {
                        self.ensure_cctor(*ty)?;
                    }
                    let at = self.slots.len() - *argc;
                    if self.types[*ty as usize].is_value {
                        let sv = self.default_value(&Ty::User(*ty));
                        self.slots.insert(at, sv);
                        self.slots.insert(at + 1, Value::Ptr(Rc::new(Ptr::Slot(at))));
                        // stack: [value][this=ptr][args] -> the body consumes [ptr][args]
                        self.call_at(*ctor, at + 1)?;
                    } else {
                        let o = self.new_obj(*ty);
                        self.slots.insert(at, Value::Obj(o.clone()));
                        self.call_at(*ctor, at)?;
                        push!(Value::Obj(o));
                    }
                }
                Op::NewObjExt { ty, f, argc, is_struct } => {
                    let at = self.slots.len() - *argc;
                    let mut args = self.slots.split_off(at);
                    if *is_struct {
                        let v = f(self, &mut args)?;
                        push!(v);
                    } else {
                        let o = self.new_obj(*ty);
                        let ov = Value::Obj(o);
                        args.insert(0, ov.clone());
                        f(self, &mut args)?;
                        push!(ov);
                    }
                }
                Op::NewDelegate => {
                    let fp = pop!();
                    let target = pop!();
                    let m = match &fp {
                        Value::Obj(o) => match &*o.x.borrow() {
                            Ext::Delegate(l) if !l.is_empty() => l[0].method,
                            _ => 0,
                        },
                        _ => 0,
                    };
                    let dty = self.type_id("System.Delegate");
                    push!(Value::Obj(Obj::new(dty, 0, Ext::Delegate(vec![Deleg { target, method: m }]))));
                }
                Op::NewObjMissing { key, argc } => {
                    let at = self.slots.len() - *argc;
                    self.slots.truncate(at);
                    if self.missing.insert(key.to_string()) {
                        crate::elog!("[clr] missing constructor: {}", key);
                    }
                    return self.throw("System.NotImplementedException", key);
                }
                Op::Ldfld(slot) => {
                    let o = pop!();
                    push!(self.get_field(&o, *slot)?);
                }
                Op::Stfld(slot) => {
                    let v = pop!();
                    let o = pop!();
                    self.set_field(&o, *slot, v)?;
                }
                Op::Ldflda(slot) => {
                    let o = pop!();
                    let p = match o {
                        Value::Obj(o) => Ptr::Field(o, *slot),
                        Value::Ptr(p) => Ptr::SField(p, *slot),
                        Value::Null => return self.throw("System.NullReferenceException", "ldflda"),
                        _ => return self.throw("System.InvalidProgramException", "ldflda on value"),
                    };
                    push!(Value::Ptr(Rc::new(p)));
                }
                Op::Ldsfld(t, i) => {
                    if self.types[*t as usize].cctor_state == 0 {
                        self.ensure_cctor(*t)?;
                    }
                    let v = self.statics[*t as usize][*i as usize].clone();
                    push!(v);
                }
                Op::Stsfld(t, i) => {
                    if self.types[*t as usize].cctor_state == 0 {
                        self.ensure_cctor(*t)?;
                    }
                    let v = pop!();
                    self.statics[*t as usize][*i as usize] = v;
                }
                Op::Ldsflda(t, i) => {
                    if self.types[*t as usize].cctor_state == 0 {
                        self.ensure_cctor(*t)?;
                    }
                    push!(Value::Ptr(Rc::new(Ptr::Static(*t, *i))));
                }
                Op::LdfldExt(key) => {
                    let o = pop!();
                    let o = self.deref(o);
                    let r = crate::unity_api::native_field(self, &o, key)?;
                    push!(r);
                }
                Op::LdsfldExt(key) => {
                    let v = crate::bcl::static_field(self, key)?;
                    push!(v);
                }
                Op::Newarr(t) => {
                    let n = pop!().as_i32();
                    if n < 0 {
                        return self.throw("System.OverflowException", "newarr");
                    }
                    let d = self.default_value(t);
                    let id = self.type_id("System.Array");
                    let o = Obj::new(id, 0, Ext::Arr(vec![d; n as usize]));
                    push!(Value::Obj(o));
                }
                Op::Ldlen => {
                    let a = pop!();
                    match &a {
                        Value::Obj(o) => match &*o.x.borrow() {
                            Ext::Arr(v) => push!(Value::I32(v.len() as i32)),
                            _ => return self.throw("System.InvalidProgramException", "ldlen"),
                        },
                        _ => return self.throw("System.NullReferenceException", "ldlen"),
                    }
                }
                Op::Ldelem => {
                    let i = pop!().as_i32();
                    let a = pop!();
                    let v = self.elem_get(&a, i)?;
                    push!(v);
                }
                Op::Stelem => {
                    let v = pop!();
                    let i = pop!().as_i32();
                    let a = pop!();
                    self.elem_set(&a, i, v)?;
                }
                Op::Ldelema => {
                    let i = pop!().as_i32();
                    let a = pop!();
                    match a {
                        Value::Obj(o) => {
                            let n = match &*o.x.borrow() { Ext::Arr(v) => v.len(), _ => 0 };
                            if i < 0 || i as usize >= n {
                                return self.throw("System.IndexOutOfRangeException", "Index was outside the bounds of the array.");
                            }
                            push!(Value::Ptr(Rc::new(Ptr::Elem(o, i as u32))));
                        }
                        _ => return self.throw("System.NullReferenceException", "ldelema"),
                    }
                }
                Op::Box(t) => {
                    let v = pop!();
                    push!(self.box_value(v, t));
                }
                Op::Unbox(_t) => {
                    let v = pop!();
                    let v = self.deref(v);
                    match v {
                        Value::Obj(o) => {
                            let inner = if let Ext::Boxed(b) = &*o.x.borrow() { Some(b.clone()) } else { None };
                            match inner {
                                Some(b) => push!(b),
                                None => push!(Value::Obj(o)),
                            }
                        }
                        Value::Null => return self.throw("System.NullReferenceException", "unbox"),
                        o => push!(o),
                    }
                }
                Op::Castclass(t) => {
                    let v = self.slots.last().unwrap().clone();
                    if !v.is_null() && !self.is_inst(&v, t) {
                        let n = self.type_name_of(&v);
                        pop!();
                        return self.throw("System.InvalidCastException", &format!("invalid cast: {}", n));
                    }
                }
                Op::Isinst(t) => {
                    let v = pop!();
                    if self.is_inst(&v, t) {
                        push!(v);
                    } else {
                        push!(Value::Null);
                    }
                }
                Op::Ldftn(m) => {
                    let dty = self.type_id("System.Delegate");
                    let o = Obj::new(dty, 0, Ext::Delegate(vec![Deleg { target: Value::Null, method: *m }]));
                    push!(Value::Obj(o));
                }
                Op::Ldvirtftn(decl) => {
                    let recv = pop!();
                    let rty = match &recv { Value::Obj(o) => o.ty, _ => self.prog.methods[*decl as usize].owner };
                    let m = self.resolve_virtual(*decl, rty);
                    let dty = self.type_id("System.Delegate");
                    let o = Obj::new(dty, 0, Ext::Delegate(vec![Deleg { target: Value::Null, method: m }]));
                    push!(Value::Obj(o));
                }
                Op::Ldind => {
                    let p = pop!();
                    match p {
                        Value::Ptr(p) => {
                            let v = self.load_ptr(&p);
                            push!(v);
                        }
                        Value::Null => return self.throw("System.NullReferenceException", "ldind"),
                        o => push!(o),
                    }
                }
                Op::Stind(t) => {
                    let v = pop!();
                    let p = pop!();
                    let v = convert_store(v, t);
                    if let Value::Ptr(p) = p {
                        self.store_ptr(&p, v);
                    } else {
                        return self.throw("System.NullReferenceException", "stind");
                    }
                }
                Op::Ldobj => {
                    let p = pop!();
                    match p {
                        Value::Ptr(p) => {
                            let v = self.load_ptr(&p);
                            push!(v);
                        }
                        Value::Null => return self.throw("System.NullReferenceException", "ldobj"),
                        o => push!(o),
                    }
                }
                Op::Stobj => {
                    let v = pop!();
                    let p = pop!();
                    if let Value::Ptr(p) = p {
                        self.store_ptr(&p, v);
                    } else {
                        return self.throw("System.NullReferenceException", "stobj");
                    }
                }
                Op::Initobj(t) => {
                    let p = pop!();
                    let d = self.default_value(t);
                    if let Value::Ptr(p) = p {
                        self.store_ptr(&p, d);
                    }
                }
                Op::LdtokenData(_) => push!(Value::Null),
                Op::Ldtoken(name) => {
                    let id = self.type_id("System.Type");
                    let o = Obj::new(id, 0, Ext::Native(Box::new(name.to_string())));
                    push!(Value::Obj(o));
                }
                Op::Throw => {
                    let e = pop!();
                    if e.is_null() {
                        return self.throw("System.NullReferenceException", "throw null");
                    }
                    return Err(e);
                }
                Op::Rethrow => {
                    // the current exception is the last "Pending::Exc"/catch: found through the exception stack
                    if let Some(e) = self.cur_exc.last().cloned() {
                        return Err(e);
                    }
                    return self.throw("System.InvalidOperationException", "rethrow");
                }
                Op::Leave(t) => {
                    // finally to run?
                    let mut chain: Vec<usize> = vec![];
                    for (i, h) in code.eh.iter().enumerate() {
                        if matches!(h.kind, EhKind::Finally) && cur >= h.try_start && cur < h.try_end && !(*t >= h.try_start && *t < h.try_end) {
                            chain.push(i);
                        }
                    }
                    self.slots.truncate(sb);
                    for h in code.eh.iter() {
                        if matches!(h.kind, EhKind::Catch(_)) && cur >= h.h_start && cur < h.h_end && !(*t >= h.h_start && *t < h.h_end) {
                            self.cur_exc.pop();
                        }
                    }
                    if chain.is_empty() {
                        *pc = *t;
                    } else {
                        let first = chain[0];
                        pending.push(Pending::Leave(*t, chain[1..].to_vec()));
                        *pc = code.eh[first].h_start;
                    }
                }
                Op::Endfinally => match pending.pop() {
                    Some(Pending::Leave(t, mut rest)) => {
                        if rest.is_empty() {
                            *pc = t;
                        } else {
                            let nxt = rest.remove(0);
                            pending.push(Pending::Leave(t, rest));
                            *pc = code.eh[nxt].h_start;
                        }
                    }
                    Some(Pending::Exc(e, hi)) => {
                        // continue propagation from outside this try
                        let try_start = code.eh[hi].try_start;
                        // look for a strictly enclosing handler
                        let mut found = None;
                        for (i, h) in code.eh.iter().enumerate() {
                            if i != hi && try_start >= h.try_start && try_start < h.try_end && !(h.try_start == code.eh[hi].try_start && h.try_end == code.eh[hi].try_end) {
                                match &h.kind {
                                    EhKind::Catch(t) => {
                                        let t = t.clone();
                                        if self.is_inst(&e, &t) {
                                            found = Some((i, true));
                                            break;
                                        }
                                    }
                                    EhKind::Finally | EhKind::Fault => {
                                        found = Some((i, false));
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        match found {
                            Some((i, true)) => {
                                self.slots.truncate(sb);
                                self.slots.push(e);
                                *pc = code.eh[i].h_start;
                            }
                            Some((i, false)) => {
                                pending.push(Pending::Exc(e, i));
                                *pc = code.eh[i].h_start;
                            }
                            None => return Err(e),
                        }
                    }
                    None => {}
                },
                Op::Unsupported(s) => {
                    return self.throw("System.InvalidProgramException", s);
                }
            }
        }
    }

    fn deref_peek(&mut self, v: &Value) -> Value {
        match v {
            Value::Ptr(p) => {
                let p = p.clone();
                let x = self.load_ptr(&p);
                if let Value::Ptr(_) = x { self.deref_peek(&x) } else { x }
            }
            o => o.clone(),
        }
    }

    /// Virtual dispatch by name for calls through external types (IEnumerator.MoveNext...).
    pub fn find_virtual_by_name(&self, ty: u32, name: &str, argc: usize) -> Option<u32> {
        let mut t = Some(ty);
        while let Some(x) = t {
            if let Some(v) = self.types[x as usize].methods.get(name) {
                for &m in v {
                    let mi = &self.prog.methods[m as usize];
                    if mi.argc == argc && !mi.is_static && !mi.abstract_ {
                        return Some(m);
                    }
                }
            }
            // explicit implementations "System.Collections.IEnumerator.MoveNext"
            for (k, v) in self.types[x as usize].methods.iter() {
                if k.ends_with(&format!(".{}", name)) && k.contains('.') {
                    for &m in v {
                        if self.prog.methods[m as usize].argc == argc {
                            return Some(m);
                        }
                    }
                }
            }
            t = self.types[x as usize].base;
        }
        None
    }

    pub fn get_field(&mut self, o: &Value, slot: u32) -> R<Value> {
        match o {
            Value::Obj(o) => Ok(o.f.borrow()[slot as usize].clone()),
            Value::Struct(s) => Ok(s.f[slot as usize].clone()),
            Value::Ptr(p) => {
                let p2 = Ptr::SField(p.clone(), slot);
                Ok(self.load_ptr(&p2))
            }
            Value::Null => self.throw("System.NullReferenceException", "Object reference not set to an instance of an object"),
            _ => self.throw("System.InvalidProgramException", "ldfld on primitive value"),
        }
    }

    pub fn set_field(&mut self, o: &Value, slot: u32, v: Value) -> R<()> {
        match o {
            Value::Obj(o) => {
                o.f.borrow_mut()[slot as usize] = v;
                Ok(())
            }
            Value::Ptr(p) => {
                let p2 = Ptr::SField(p.clone(), slot);
                self.store_ptr(&p2, v);
                Ok(())
            }
            Value::Null => self.throw("System.NullReferenceException", "Object reference not set to an instance of an object"),
            _ => self.throw("System.InvalidProgramException", "stfld on value"),
        }
    }

    pub fn elem_get(&mut self, a: &Value, i: i32) -> R<Value> {
        match a {
            Value::Obj(o) => {
                let r = match &*o.x.borrow() {
                    Ext::Arr(v) => v.get(i as usize).cloned(),
                    _ => None,
                };
                match r {
                    Some(v) if i >= 0 => Ok(v),
                    _ => self.throw("System.IndexOutOfRangeException", "Index was outside the bounds of the array."),
                }
            }
            _ => self.throw("System.NullReferenceException", "ldelem"),
        }
    }

    pub fn elem_set(&mut self, a: &Value, i: i32, v: Value) -> R<()> {
        match a {
            Value::Obj(o) => {
                let ok = match &mut *o.x.borrow_mut() {
                    Ext::Arr(arr) if i >= 0 && (i as usize) < arr.len() => {
                        arr[i as usize] = v;
                        true
                    }
                    _ => false,
                };
                if ok { Ok(()) } else { self.throw("System.IndexOutOfRangeException", "Index was outside the bounds of the array.") }
            }
            _ => self.throw("System.NullReferenceException", "stelem"),
        }
    }

    pub fn box_value(&mut self, v: Value, t: &Ty) -> Value {
        match (&v, t) {
            (Value::Struct(s), _) => {
                let ty = s.ty;
                Value::Obj(Obj::new(ty, 0, Ext::Boxed(v)))
            }
            (_, Ty::User(id)) if self.types[*id as usize].is_enum => {
                Value::Obj(Obj::new(*id, 0, Ext::Boxed(v)))
            }
            _ => v,
        }
    }
}

pub enum Pending {
    Leave(usize, Vec<usize>),
    Exc(Value, usize),
}


pub enum ArOp { Add, Sub, Mul, Div, DivUn, Rem, RemUn, And, Or, Xor }

pub fn arith(a: &Value, b: &Value, op: ArOp) -> Value {
    use Value::*;
    match (a, b) {
        (I32(x), I32(y)) => I32(match op {
            ArOp::Add => x.wrapping_add(*y),
            ArOp::Sub => x.wrapping_sub(*y),
            ArOp::Mul => x.wrapping_mul(*y),
            ArOp::Div => x.wrapping_div(*y),
            ArOp::DivUn => ((*x as u32) / (*y as u32)) as i32,
            ArOp::Rem => x.wrapping_rem(*y),
            ArOp::RemUn => ((*x as u32) % (*y as u32)) as i32,
            ArOp::And => x & y,
            ArOp::Or => x | y,
            ArOp::Xor => x ^ y,
        }),
        (I64(_), _) | (_, I64(_)) if !matches!(a, F32(_) | F64(_)) && !matches!(b, F32(_) | F64(_)) => {
            let (x, y) = (a.as_i64(), b.as_i64());
            I64(match op {
                ArOp::Add => x.wrapping_add(y),
                ArOp::Sub => x.wrapping_sub(y),
                ArOp::Mul => x.wrapping_mul(y),
                ArOp::Div => x.wrapping_div(y),
                ArOp::DivUn => ((x as u64) / (y as u64)) as i64,
                ArOp::Rem => x.wrapping_rem(y),
                ArOp::RemUn => ((x as u64) % (y as u64)) as i64,
                ArOp::And => x & y,
                ArOp::Or => x | y,
                ArOp::Xor => x ^ y,
            })
        }
        (F32(x), F32(y)) => F32(match op {
            ArOp::Add => x + y,
            ArOp::Sub => x - y,
            ArOp::Mul => x * y,
            ArOp::Div => x / y,
            ArOp::Rem => x % y,
            _ => 0.0,
        }),
        _ => {
            let (x, y) = (a.as_f64(), b.as_f64());
            F64(match op {
                ArOp::Add => x + y,
                ArOp::Sub => x - y,
                ArOp::Mul => x * y,
                ArOp::Div => x / y,
                ArOp::Rem => x % y,
                _ => 0.0,
            })
        }
    }
}

pub fn values_eq(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Null, Null) => true,
        (I32(x), I32(y)) => x == y,
        (I64(_), _) | (_, I64(_)) if !matches!(a, F32(_) | F64(_) | Null) && !matches!(b, F32(_) | F64(_) | Null) => a.as_i64() == b.as_i64(),
        (F32(x), F32(y)) => x == y,
        (F32(_) | F64(_), _) | (_, F32(_) | F64(_)) => a.as_f64() == b.as_f64(),
        (Str(x), Str(y)) => x == y,
        (Obj(x), Obj(y)) => Rc::ptr_eq(x, y),
        (Ptr(x), Ptr(y)) => Rc::ptr_eq(x, y),
        (I32(x), Null) | (Null, I32(x)) => *x == 0,
        _ => false,
    }
}

#[derive(Clone, Copy)]
pub enum Cmp { Gt, Lt, Ge, Le }

pub fn compare(a: &Value, b: &Value, un: bool, c: Cmp) -> bool {
    use Value::*;
    match (a, b) {
        (F32(_) | F64(_), _) | (_, F32(_) | F64(_)) => {
            let (x, y) = (a.as_f64(), b.as_f64());
            if x.is_nan() || y.is_nan() {
                return un;
            }
            match c { Cmp::Gt => x > y, Cmp::Lt => x < y, Cmp::Ge => x >= y, Cmp::Le => x <= y }
        }
        (I32(x), I32(y)) => {
            if un {
                let (x, y) = (*x as u32, *y as u32);
                match c { Cmp::Gt => x > y, Cmp::Lt => x < y, Cmp::Ge => x >= y, Cmp::Le => x <= y }
            } else {
                match c { Cmp::Gt => x > y, Cmp::Lt => x < y, Cmp::Ge => x >= y, Cmp::Le => x <= y }
            }
        }
        (Null, Null) => matches!(c, Cmp::Ge | Cmp::Le),
        (Null, _) => {
            // cgt.un null : ref != null
            if un { matches!(c, Cmp::Lt | Cmp::Le) } else { false }
        }
        (_, Null) => {
            if un { matches!(c, Cmp::Gt | Cmp::Ge) } else { false }
        }
        _ => {
            if un {
                let (x, y) = (a.as_i64() as u64, b.as_i64() as u64);
                match c { Cmp::Gt => x > y, Cmp::Lt => x < y, Cmp::Ge => x >= y, Cmp::Le => x <= y }
            } else {
                let (x, y) = (a.as_i64(), b.as_i64());
                match c { Cmp::Gt => x > y, Cmp::Lt => x < y, Cmp::Ge => x >= y, Cmp::Le => x <= y }
            }
        }
    }
}

pub fn convert(a: &Value, t: &Ty) -> Value {
    use Value::*;
    let f = |v: f64| v;
    match t {
        Ty::I8 => I32(match a { F32(_) | F64(_) => f(a.as_f64()) as i8 as i32, _ => a.as_i64() as i8 as i32 }),
        Ty::U8 | Ty::Bool => I32(match a { F32(_) | F64(_) => a.as_f64() as u8 as i32, _ => a.as_i64() as u8 as i32 }),
        Ty::I16 => I32(match a { F32(_) | F64(_) => a.as_f64() as i16 as i32, _ => a.as_i64() as i16 as i32 }),
        Ty::U16 | Ty::Char => I32(match a { F32(_) | F64(_) => a.as_f64() as u16 as i32, _ => a.as_i64() as u16 as i32 }),
        Ty::I32 => I32(match a { F32(_) | F64(_) => a.as_f64() as i32, _ => a.as_i64() as i32 }),
        Ty::U32 => I32(match a { F32(_) | F64(_) => a.as_f64() as u32 as i32, _ => a.as_i64() as u32 as i32 }),
        Ty::I64 | Ty::IntPtr => I64(match a { F32(_) | F64(_) => a.as_f64() as i64, I32(x) => *x as i64, _ => a.as_i64() }),
        Ty::U64 | Ty::UIntPtr => I64(match a { F32(_) | F64(_) => a.as_f64() as u64 as i64, I32(x) => *x as u32 as i64, _ => a.as_i64() }),
        Ty::F32 => F32(a.as_f64() as f32),
        Ty::F64 => F64(a.as_f64()),
        _ => a.clone(),
    }
}

/// Value stored through stind.*: truncated according to the type.
pub fn convert_store(v: Value, t: &Ty) -> Value {
    match t {
        Ty::I8 | Ty::U8 | Ty::I16 | Ty::U16 | Ty::I32 | Ty::U32 | Ty::Bool | Ty::Char => convert(&v, t),
        Ty::F32 => Value::F32(v.as_f32()),
        Ty::F64 => Value::F64(v.as_f64()),
        _ => v,
    }
}

/// Trace of all user methods called (NGU_TRACE_M=1): same format in the oracle and native mode.
pub fn trace_m() -> bool {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var("NGU_TRACE_M").is_ok())
}

/// NGU_NO_GUARD=1: disables the `guard_skip` optimisation (to compare with native execution, which does not do it).
pub fn no_guard() -> bool {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var("NGU_NO_GUARD").is_ok())
}
