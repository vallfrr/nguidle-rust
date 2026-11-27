//! Lowering: dotnetdll::Instruction -> Op (token resolution, 1 instruction = 1 op).
use crate::code::*;
use crate::program::*;
use crate::Vm;
use dotnetdll::prelude::*;
use std::rc::Rc;

enum Target {
    User(u32),
    Ext { tname: String, name: String, argc: usize, ret: bool, instance: bool, ptys: Vec<Ty>, rty: Option<Ty> },
    Missing(String),
}

pub fn ty_name(t: &Ty, vm: &Vm) -> String {
    match t {
        Ty::Bool => "System.Boolean".into(),
        Ty::Char => "System.Char".into(),
        Ty::I8 => "System.SByte".into(),
        Ty::U8 => "System.Byte".into(),
        Ty::I16 => "System.Int16".into(),
        Ty::U16 => "System.UInt16".into(),
        Ty::I32 => "System.Int32".into(),
        Ty::U32 => "System.UInt32".into(),
        Ty::I64 => "System.Int64".into(),
        Ty::U64 => "System.UInt64".into(),
        Ty::F32 => "System.Single".into(),
        Ty::F64 => "System.Double".into(),
        Ty::IntPtr => "System.IntPtr".into(),
        Ty::UIntPtr => "System.UIntPtr".into(),
        Ty::Object => "System.Object".into(),
        Ty::String => "System.String".into(),
        Ty::Array(e) => format!("{}[]", ty_name(e, vm)),
        Ty::User(id) => vm.types[*id as usize].name.clone(),
        Ty::Ext(n, _) => n.clone(),
        Ty::Generic => "T".into(),
        Ty::Pointer => "ptr".into(),
    }
}

fn conv_ty(c: &ConversionType) -> Ty {
    match c {
        ConversionType::Int8 => Ty::I8,
        ConversionType::UInt8 => Ty::U8,
        ConversionType::Int16 => Ty::I16,
        ConversionType::UInt16 => Ty::U16,
        ConversionType::Int32 => Ty::I32,
        ConversionType::UInt32 => Ty::U32,
        ConversionType::Int64 => Ty::I64,
        ConversionType::UInt64 => Ty::U64,
        ConversionType::IntPtr => Ty::I64,
        ConversionType::UIntPtr => Ty::U64,
    }
}

fn store_ty(c: &StoreType) -> Ty {
    match c {
        StoreType::Int8 => Ty::I8,
        StoreType::Int16 => Ty::I16,
        StoreType::Int32 => Ty::I32,
        StoreType::Int64 => Ty::I64,
        StoreType::Float32 => Ty::F32,
        StoreType::Float64 => Ty::F64,
        StoreType::IntPtr => Ty::I64,
        StoreType::Object => Ty::Object,
    }
}

impl Vm {
    pub fn tyop(&mut self, t: &Ty) -> TyOp {
        match t {
            Ty::User(id) => TyOp::Class(*id),
            Ty::Ext(n, _) => {
                if n == "System.Object" {
                    TyOp::Any
                } else {
                    TyOp::Class(self.type_id(n))
                }
            }
            Ty::Array(_) => TyOp::Array,
            Ty::Generic | Ty::Pointer => TyOp::Any,
            Ty::Object => TyOp::Any,
            p => TyOp::Prim(p.clone()),
        }
    }

    fn resolve_method(&mut self, src: &MethodSource) -> (Target, Option<Rc<Vec<Ty>>>) {
        let prog = self.prog.clone();
        let (user, targs) = match src {
            MethodSource::User(u) => (u, None),
            MethodSource::Generic(g) => (&g.base, Some(Rc::new(g.parameters.iter().map(|p| prog.ty_of(p)).collect::<Vec<_>>()))),
        };
        let t = match user {
            UserMethod::Definition(mi) => Target::User(prog.mid[mi]),
            UserMethod::Reference(mr) => {
                let r = &prog.res[*mr];
                let argc = r.signature.parameters.len() + r.signature.instance as usize;
                let ret = r.signature.return_type.1.is_some();
                let name = r.name.to_string();
                match &r.parent {
                    MethodReferenceParent::Type(mt) => match prog.ty_of(mt) {
                        Ty::User(id) => match self.find_method(id, &name, argc) {
                            Some(m) => Target::User(m),
                            None => {
                                // abstract / interface: take the declaration
                                let tn = self.types[id as usize].methods.get(&name).and_then(|v| v.iter().copied().find(|&m| self.prog.methods[m as usize].argc == argc));
                                match tn {
                                    Some(m) => Target::User(m),
                                    None => Target::Missing(format!("{}::{}", self.types[id as usize].name, name)),
                                }
                            }
                        },
                        other => {
                            let pt = |p: &dotnetdll::resolved::signature::Parameter<MethodType>| match &p.1 {
                                dotnetdll::resolved::signature::ParameterType::Value(t) | dotnetdll::resolved::signature::ParameterType::Ref(t) => prog.ty_of(t),
                                _ => Ty::Object,
                            };
                            let ptys: Vec<Ty> = r.signature.parameters.iter().map(pt).collect();
                            let tname = ty_name(&other, self);
                            let rty = r.signature.return_type.1.as_ref().and_then(|x| match x {
                                dotnetdll::resolved::signature::ParameterType::Value(t) | dotnetdll::resolved::signature::ParameterType::Ref(t) => {
                                    if name == "Invoke" && crate::bcl::is_delegate_type(&tname) {
                                        // A closed delegate's return type can refer to a parameter of the receiver type.
                                        if let (MethodType::TypeGeneric(index), MethodType::Base(receiver)) = (t, mt) {
                                            if let BaseType::Type { source: TypeSource::Generic { parameters, .. }, .. } = &**receiver {
                                                if let Some(parameter) = parameters.get(*index) {
                                                    return Some(prog.ty_of(parameter));
                                                }
                                            }
                                        }
                                    }
                                    Some(prog.ty_of(t))
                                }
                                _ => None,
                            });
                            Target::Ext { tname, name, argc, ret, instance: r.signature.instance, ptys, rty }
                        }
                    },
                    _ => Target::Missing(format!("<module>::{}", name)),
                }
            }
        };
        (t, targs)
    }

    fn lower_call(&mut self, src: &MethodSource, virt: bool, ops_name: &str) -> Op {
        let (t, targs) = self.resolve_method(src);
        match t {
            Target::User(m) => {
                let mi = &self.prog.methods[m as usize];
                let has_body = mi.mi.map(|x| self.prog.res[x].body.is_some()).unwrap_or(false);
                if !has_body && mi.name == "Invoke" {
                    return Op::DelegateInvoke { argc: mi.argc, ret: mi.has_ret };
                }
                if virt && mi.virt || virt && self.types[mi.owner as usize].is_iface {
                    Op::CallVirtUser(m)
                } else {
                    Op::Call(m)
                }
            }
            Target::Ext { tname, name, argc, ret, instance, ptys, rty } => {
                self.last_meta = Some(Rc::new(ExtMeta { key: format!("{}::{}", tname, name), tname: tname.clone(), name: name.clone(), params: ptys, ret: rty, instance, targs: targs.as_ref().map(|t| (**t).clone()).unwrap_or_default() }));
                if name == "Invoke" && instance && crate::bcl::is_delegate_type(&tname) {
                    return Op::DelegateInvoke { argc, ret };
                }
                let key = format!("{}::{}", tname, name);
                match crate::bcl::lookup(&key) {
                    Some(f) => {
                        if name == ".ctor" && crate::bcl::is_native_struct(&tname) {
                            Op::CallStructCtor { f, argc }
                        } else if virt {
                            Op::CallVirtExt { name: Rc::from(name.as_str()), argc, f, ret, targs: targs.clone() }
                        } else {
                            Op::CallNative { f, argc, ret, targs: targs.clone() }
                        }
                    }
                    None => {
                        let _ = ops_name;
                        Op::CallMissing { key: Rc::from(key.as_str()), argc, ret }
                    }
                }
            }
            Target::Missing(k) => Op::CallMissing { key: Rc::from(k.as_str()), argc: 0, ret: false },
        }
    }

    fn lower_newobj(&mut self, u: &UserMethod) -> Op {
        let prog = self.prog.clone();
        match u {
            UserMethod::Definition(mi) => {
                let m = prog.mid[mi];
                let info = &prog.methods[m as usize];
                let owner = info.owner;
                // user delegate?
                if prog.res[*mi].body.is_none() && self.types[owner as usize].base.is_some() {
                    return Op::NewDelegate;
                }
                Op::NewObj { ctor: m, argc: info.argc - 1, ty: owner }
            }
            UserMethod::Reference(mr) => {
                let r = &prog.res[*mr];
                let argc = r.signature.parameters.len();
                let tname = match &r.parent {
                    MethodReferenceParent::Type(mt) => prog.ty_of(mt),
                    _ => Ty::Object,
                };
                match tname {
                    Ty::User(id) => match self.find_method(id, ".ctor", argc + 1) {
                        Some(m) => Op::NewObj { ctor: m, argc, ty: id },
                        None => Op::NewObjMissing { key: Rc::from(format!("{}::.ctor", self.types[id as usize].name).as_str()), argc },
                    },
                    other => {
                        let n = ty_name(&other, self);
                        if crate::bcl::is_delegate_type(&n) && argc == 2 {
                            return Op::NewDelegate;
                        }
                        let key = format!("{}::.ctor", n);
                        match crate::bcl::lookup(&key) {
                            Some(f) => {
                                let id = self.type_id(&n);
                                Op::NewObjExt { ty: id, f, argc, is_struct: crate::bcl::is_native_struct(&n) }
                            }
                            None => Op::NewObjMissing { key: Rc::from(key.as_str()), argc },
                        }
                    }
                }
            }
        }
    }

    pub fn lower(&mut self, mid: u32) -> Rc<Code> {
        let prog = self.prog.clone();
        let info = &prog.methods[mid as usize];
        let mi = info.mi.unwrap();
        let m = &prog.res[mi];
        let body = match &m.body {
            Some(b) => b,
            None if m.pinvoke.is_some() => {
                // native Windows call (user32...): neutral value
                return Rc::new(Code {
                    ops: vec![Op::I4(0), Op::Ret],
                    nargs: info.argc,
                    nlocals: 0,
                    local_defaults: vec![],
                    local_vals: Default::default(),
                    eh: vec![],
                    dbg: None,
                    ext: vec![],
                });
            }
            None => {
                return Rc::new(Code {
                    ops: vec![Op::Unsupported(Rc::from(format!("no body: {}::{}", self.types[info.owner as usize].name, info.name).as_str()))],
                    nargs: info.argc,
                    nlocals: 0,
                    local_defaults: vec![],
                    local_vals: Default::default(),
                    eh: vec![],
                    dbg: None,
                    ext: vec![],
                })
            }
        };
        let local_defaults: Vec<Ty> = body
            .header
            .local_variables
            .iter()
            .map(|l| match l {
                LocalVariable::Variable { by_ref: false, var_type, .. } => prog.ty_of(var_type),
                _ => Ty::Object,
            })
            .collect();
        let mut ops = Vec::with_capacity(body.instructions.len());
        let mut ext = Vec::with_capacity(body.instructions.len());
        for ins in &body.instructions {
            self.last_meta = None;
            ops.push(self.lower_ins(ins));
            ext.push(self.last_meta.take());
        }
        let mut eh = vec![];
        for ds in &body.data_sections {
            if let dotnetdll::resolved::body::DataSection::ExceptionHandlers(list) = ds {
                for e in list {
                    use dotnetdll::resolved::body::ExceptionKind as K;
                    let kind = match &e.kind {
                        K::TypedException(t) => {
                            let ty = prog.ty_of(t);
                            EhKind::Catch(self.tyop(&ty))
                        }
                        K::Filter { offset } => EhKind::Filter(*offset),
                        K::Finally => EhKind::Finally,
                        K::Fault => EhKind::Fault,
                    };
                    eh.push(Eh {
                        kind,
                        try_start: e.try_offset,
                        try_end: e.try_offset + e.try_length,
                        h_start: e.handler_offset,
                        h_end: e.handler_offset + e.handler_length,
                    });
                }
            }
        }
        // The original runtime computes `(int)(float * float)` in double precision: widen the operation before an integer conversion.
        for i in 0..ops.len().saturating_sub(1) {
            if !matches!(ops[i + 1], Op::Conv(Ty::I8 | Ty::U8 | Ty::I16 | Ty::U16 | Ty::I32 | Ty::U32 | Ty::I64 | Ty::U64)) {
                continue;
            }
            ops[i] = match ops[i] {
                Op::Add => Op::WideArith(0),
                Op::Sub => Op::WideArith(1),
                Op::Mul => Op::WideArith(2),
                Op::Div(false) => Op::WideArith(3),
                _ => continue,
            };
        }
        // innermost handlers first
        eh.sort_by_key(|h| (h.try_end - h.try_start, h.try_start));
        let dbg = if std::env::var("CLR_DBG").is_ok() { Some(body.instructions.iter().map(|i| i.show(&prog.res)).collect()) } else { None };
        Rc::new(Code { ops, nargs: info.argc, nlocals: local_defaults.len(), local_defaults, local_vals: Default::default(), eh, dbg, ext })
    }

    fn field_target(&mut self, f: &FieldSource) -> FieldT {
        let prog = self.prog.clone();
        match f {
            FieldSource::Definition(fi) => {
                let (t, slot, st) = prog.fslot[fi];
                if st { FieldT::Static(t, slot) } else { FieldT::Inst(slot) }
            }
            FieldSource::Reference(fr) => {
                let r = &prog.res[*fr];
                let tname = match &r.parent {
                    FieldReferenceParent::Type(mt) => {
                        let ty = prog.ty_of(mt);
                        // field of an instantiated generic user type
                        if let Ty::User(id) = ty {
                            let t = &self.types[id as usize];
                            if let Some(p) = t.fields.iter().position(|n| n == r.name.as_ref()) {
                                return FieldT::Inst(p as u32);
                            }
                            if let Some(&i) = t.statics.get(r.name.as_ref()) {
                                return FieldT::Static(id, i);
                            }
                        }
                        ty_name(&ty, self)
                    }
                    _ => "?".into(),
                };
                match crate::bcl::native_field_slot(&tname, &r.name) {
                    Some(s) => FieldT::Inst(s),
                    None => FieldT::Ext(format!("{}::{}", tname, r.name)),
                }
            }
        }
    }

    fn lower_ins(&mut self, ins: &Instruction) -> Op {
        use Instruction as I;
        let uns = |s: &NumberSign| matches!(s, NumberSign::Unsigned);
        match ins {
            I::Add => Op::Add,
            I::AddOverflow(s) => Op::AddOvf(uns(s)),
            I::And => Op::And,
            I::BranchEqual(t) => Op::Beq(*t),
            I::BranchGreaterOrEqual(s, t) => Op::Bge(uns(s), *t),
            I::BranchGreater(s, t) => Op::Bgt(uns(s), *t),
            I::BranchLessOrEqual(s, t) => Op::Ble(uns(s), *t),
            I::BranchLess(s, t) => Op::Blt(uns(s), *t),
            I::BranchNotEqual(t) => Op::Bne(*t),
            I::Branch(t) => Op::Br(*t),
            I::BranchFalsy(t) => Op::Brfalse(*t),
            I::BranchTruthy(t) => Op::Brtrue(*t),
            I::Call { param0: src, .. } => self.lower_call(src, false, "call"),
            I::CallConstrained(_, src) | I::CallConstrainedTail(_, src) => self.lower_call(src, true, "constrained"),
            I::CallVirtual { param0: src, .. } => self.lower_call(src, true, "callvirt"),
            I::CallVirtualConstrained(_, src) | I::CallVirtualTail(src) | I::CallVirtualConstrainedTail(_, src) => self.lower_call(src, true, "callvirt"),
            I::CompareEqual => Op::Ceq,
            I::CompareGreater(s) => Op::Cgt(uns(s)),
            I::CompareLess(s) => Op::Clt(uns(s)),
            I::Convert(c) => Op::Conv(conv_ty(c)),
            I::ConvertOverflow(c, s) => Op::ConvOvf(conv_ty(c), uns(s)),
            I::ConvertFloat32 => Op::Conv(Ty::F32),
            I::ConvertFloat64 => Op::Conv(Ty::F64),
            I::ConvertUnsignedToFloat => Op::ConvRUn,
            I::Divide(s) => Op::Div(uns(s)),
            I::Duplicate => Op::Dup,
            I::EndFinally => Op::Endfinally,
            I::LoadArgument(i) => Op::Ldarg(*i),
            I::LoadArgumentAddress(i) => Op::Ldarga(*i),
            I::LoadConstantInt32(v) => Op::I4(*v),
            I::LoadConstantInt64(v) => Op::I8(*v),
            I::LoadConstantFloat32(v) => Op::R4(*v),
            I::LoadConstantFloat64(v) => Op::R8(*v),
            I::LoadMethodPointer(src) => match self.resolve_method(src).0 {
                Target::User(m) => Op::Ldftn(m),
                _ => Op::Unsupported(Rc::from("external ldftn")),
            },
            I::LoadIndirect { param0: _, .. } => Op::Ldind,
            I::LoadLocal(i) => Op::Ldloc(*i),
            I::LoadLocalAddress(i) => Op::Ldloca(*i),
            I::LoadNull => Op::Null,
            I::Leave(t) => Op::Leave(*t),
            I::Multiply => Op::Mul,
            I::MultiplyOverflow(s) => Op::MulOvf(uns(s)),
            I::Negate => Op::Neg,
            I::NoOperation => Op::Nop,
            I::Not => Op::Not,
            I::Or => Op::Or,
            I::Pop => Op::Pop,
            I::Remainder(s) => Op::Rem(uns(s)),
            I::Return => Op::Ret,
            I::ShiftLeft => Op::Shl,
            I::ShiftRight(s) => Op::Shr(uns(s)),
            I::StoreArgument(i) => Op::Starg(*i),
            I::StoreIndirect { param0: t, .. } => Op::Stind(store_ty(t)),
            I::StoreLocal(i) => Op::Stloc(*i),
            I::Subtract => Op::Sub,
            I::SubtractOverflow(s) => Op::SubOvf(uns(s)),
            I::Switch(ts) => Op::Switch(ts.clone()),
            I::Xor => Op::Xor,
            I::BoxValue(t) => {
                let ty = self.prog.ty_of(t);
                Op::Box(ty)
            }
            I::CastClass { param0: t, .. } => {
                let ty = self.prog.ty_of(t);
                Op::Castclass(self.tyop(&ty))
            }
            I::IsInstance(t) => {
                let ty = self.prog.ty_of(t);
                Op::Isinst(self.tyop(&ty))
            }
            I::InitializeForObject(t) => {
                let ty = self.prog.ty_of(t);
                Op::Initobj(ty)
            }
            I::LoadElement { .. } | I::LoadElementPrimitive { .. } => Op::Ldelem,
            I::StoreElement { .. } | I::StoreElementPrimitive { .. } => Op::Stelem,
            I::LoadElementAddress { .. } | I::LoadElementAddressReadonly(_) => Op::Ldelema,
            I::LoadField { param0: f, .. } | I::LoadFieldSkipNullCheck(f) => match self.field_target(f) {
                FieldT::Inst(s) => Op::Ldfld(s),
                FieldT::Static(t, i) => Op::Ldsfld(t, i),
                FieldT::Ext(k) => Op::LdfldExt(Rc::from(k.as_str())),
            },
            I::LoadFieldAddress(f) => match self.field_target(f) {
                FieldT::Inst(s) => Op::Ldflda(s),
                FieldT::Static(t, i) => Op::Ldsflda(t, i),
                FieldT::Ext(k) => Op::Unsupported(Rc::from(format!("ldflda ext {}", k).as_str())),
            },
            I::LoadLength => Op::Ldlen,
            I::LoadObject { .. } => Op::Ldobj,
            I::LoadStaticField { param0: f, .. } => match self.field_target(f) {
                FieldT::Static(t, i) => Op::Ldsfld(t, i),
                FieldT::Ext(k) => Op::LdsfldExt(Rc::from(k.as_str())),
                FieldT::Inst(_) => Op::Unsupported(Rc::from("ldsfld inst")),
            },
            I::LoadStaticFieldAddress(f) => match self.field_target(f) {
                FieldT::Static(t, i) => Op::Ldsflda(t, i),
                _ => Op::Unsupported(Rc::from("ldsflda ext")),
            },
            I::LoadString(s) => Op::Str(Rc::from(String::from_utf16_lossy(s).as_str())),
            I::LoadTokenField(f) => match f {
                FieldSource::Definition(fi) => match &self.prog.res[*fi].initial_value {
                    Some(b) => Op::LdtokenData(Rc::new(b.to_vec())),
                    None => Op::Unsupported(Rc::from("ldtoken field without data")),
                },
                _ => Op::Unsupported(Rc::from("external ldtoken field")),
            },
            I::LoadTokenType(t) => {
                let ty = self.prog.ty_of(t);
                Op::Ldtoken(Rc::from(ty_name(&ty, self).as_str()))
            }
            I::LoadVirtualMethodPointer { param0: src, .. } => match self.resolve_method(src).0 {
                Target::User(m) => Op::Ldvirtftn(m),
                _ => Op::Unsupported(Rc::from("external ldvirtftn")),
            },
            I::NewArray(t) => {
                let ty = self.prog.ty_of(t);
                Op::Newarr(ty)
            }
            I::NewObject(u) => self.lower_newobj(u),
            I::Rethrow => Op::Rethrow,
            I::Sizeof(_) => Op::I4(4),
            I::StoreField { param0: f, .. } | I::StoreFieldSkipNullCheck(f) => match self.field_target(f) {
                FieldT::Inst(s) => Op::Stfld(s),
                FieldT::Static(t, i) => Op::Stsfld(t, i),
                FieldT::Ext(k) => Op::Unsupported(Rc::from(format!("stfld ext {}", k).as_str())),
            },
            I::StoreObject { .. } => Op::Stobj,
            I::StoreStaticField { param0: f, .. } => match self.field_target(f) {
                FieldT::Static(t, i) => Op::Stsfld(t, i),
                _ => Op::Unsupported(Rc::from("stsfld ext")),
            },
            I::Throw => Op::Throw,
            I::UnboxIntoAddress { .. } => Op::Unbox(Ty::Object),
            I::UnboxIntoValue(t) => Op::Unbox(self.prog.ty_of(t)),
            other => Op::Unsupported(Rc::from(format!("unhandled instruction: {:?}", other.show(&self.prog.res)).as_str())),
        }
    }
}

enum FieldT {
    Inst(u32),
    Static(u32, u32),
    Ext(String),
}
