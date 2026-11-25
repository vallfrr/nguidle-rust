//! Virtual machine: type registry, calls, execution loop, exceptions.
mod instructions;
mod specializations;
use specializations::*;

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
    wish_tick_fields: Option<WishTickFields>,
    titan_check_fields: Option<[(u32, usize); 3]>,
    titan_checks: Option<TitanChecks>,
    score_fields: Option<ScoreFields>,
    optimal_score: Option<(Recipe, f32)>,
    achievement_stats: Option<(u32, [Option<Value>; 6])>,
    effective_boss_fields: Option<[(u32, usize); 3]>,
    equip_spec_fields: Option<EquipSpecFields>,
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
            wish_tick_fields: None,
            titan_check_fields: None,
            titan_checks: None,
            score_fields: None,
            optimal_score: None,
            achievement_stats: None,
            effective_boss_fields: None,
            equip_spec_fields: None,
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
        vm.equip_spec_fields = EquipSpecFields::resolve(&vm);
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

    pub fn invoke(&mut self, mid: u32, args: impl IntoIterator<Item = Value>) -> R<Value> {
        if self.trace_calls() {
            let mi = &self.prog.methods[mid as usize];
            eprintln!("[call] {}::{}", self.types[mi.owner as usize].name, mi.name);
        }
        let base = self.slots.len();
        self.slots.extend(args);
        self.call_at(mid, base)
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
                20 => self.native_equip_spec_bonus(base),
                21 => self.native_spec_bonus(base),
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

    /// `InventoryController.equipSpecBonus(type, equip)` without interpreting it (same f32 arithmetic and order as the IL).
    /// Returns None (the interpreter runs the method) for anything unexpected, including null arguments.
    #[allow(dead_code)]
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
        let stack_base = base + code.nargs + code.nlocals;
        cil_interp::execute(&mut CallFrame { vm: self, code, base, stack_base })
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

struct CallFrame<'a> {
    vm: &'a mut Vm,
    code: &'a Code,
    base: usize,
    stack_base: usize,
}

impl cil_interp::Frame for CallFrame<'_> {
    type Value = Value;
    type Pending = Pending;

    #[inline]
    fn step(&mut self, pc: &mut usize, pending: &mut Vec<Pending>) -> R<Option<Value>> {
        self.vm.step(self.code, self.base, self.stack_base, pc, pending)
    }

    fn handle_exception(&mut self, pc: usize, exc: Value, pending: &mut Vec<Pending>) -> R<usize> {
        if let (Some(d), Value::Obj(o)) = (&self.code.dbg, &exc) {
            if let Ext::Native(b) = &mut *o.x.borrow_mut() {
                if let Some(s) = b.downcast_mut::<String>() {
                    if !s.contains("{pc") {
                        s.push_str(&format!(" {{pc {}: {}}}", pc, d.get(pc).cloned().unwrap_or_default()));
                    }
                }
            }
        }
        let Some((handler_index, is_catch)) = self.vm.find_handler(self.code, pc, &exc) else {
            return Err(exc);
        };
        self.vm.slots.truncate(self.stack_base);
        if is_catch {
            self.vm.cur_exc.push(exc.clone());
            self.vm.slots.push(exc);
        } else {
            pending.push(Pending::Exc(exc, handler_index));
        }
        Ok(self.code.eh[handler_index].h_start)
    }
}

pub enum Pending {
    Leave(usize, Vec<usize>),
    Exc(Value, usize),
}


pub use cil_interp::ArOp;

pub fn arith(a: &Value, b: &Value, op: ArOp) -> Value {
    use Value::*;
    match (a, b) {
        (I32(x), I32(y)) => I32(cil_interp::arith_i32(*x, *y, op)),
        (I64(_), _) | (_, I64(_)) if !matches!(a, F32(_) | F64(_)) && !matches!(b, F32(_) | F64(_)) =>
            I64(cil_interp::arith_i64(a.as_i64(), b.as_i64(), op)),
        (F32(x), F32(y)) => F32(cil_interp::arith_f32(*x, *y, op)),
        _ => F64(cil_interp::arith_f64(a.as_f64(), b.as_f64(), op)),
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

/// Trace user method calls when NGU_TRACE_M=1.
pub fn trace_m() -> bool {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var("NGU_TRACE_M").is_ok())
}

/// NGU_NO_GUARD=1: disables the `guard_skip` optimisation (to compare with native execution, which does not do it).
pub fn no_guard() -> bool {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var("NGU_NO_GUARD").is_ok())
}
