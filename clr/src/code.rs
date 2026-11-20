//! Lowered (resolved) code of a method.
use crate::program::Ty;
use crate::value::*;
use crate::Vm;
use std::rc::Rc;

pub type NativeFn = fn(&mut Vm, &mut Vec<Value>) -> R<Value>;

#[derive(Clone)]
pub enum TyOp {
    Prim(Ty),
    /// Class/interface (id in Vm.types).
    Class(u32),
    Array,
    Any,
}

pub enum Op {
    Nop,
    Ldarg(u16),
    Starg(u16),
    Ldloc(u16),
    Stloc(u16),
    Ldarga(u16),
    Ldloca(u16),
    I4(i32),
    I8(i64),
    R4(f32),
    R8(f64),
    Null,
    Str(Rc<str>),
    Dup,
    Pop,
    Add,
    Sub,
    Mul,
    Div(bool),
    Rem(bool),
    And,
    Or,
    Xor,
    Shl,
    Shr(bool),
    Neg,
    Not,
    AddOvf(bool),
    SubOvf(bool),
    MulOvf(bool),
    Ceq,
    Cgt(bool),
    Clt(bool),
    Br(usize),
    Brtrue(usize),
    Brfalse(usize),
    Beq(usize),
    Bne(usize),
    Bge(bool, usize),
    Bgt(bool, usize),
    Ble(bool, usize),
    Blt(bool, usize),
    Switch(Vec<usize>),
    Conv(Ty),
    ConvOvf(Ty, bool),
    ConvRUn,
    Call(u32),
    CallVirtUser(u32),
    CallVirtExt { name: Rc<str>, argc: usize, f: NativeFn, ret: bool, targs: Option<Rc<Vec<Ty>>> },
    CallNative { f: NativeFn, argc: usize, ret: bool, targs: Option<Rc<Vec<Ty>>> },
    CallStructCtor { f: NativeFn, argc: usize },
    CallMissing { key: Rc<str>, argc: usize, ret: bool },
    DelegateInvoke { argc: usize, ret: bool },
    Ret,
    NewObj { ctor: u32, argc: usize, ty: u32 },
    NewObjExt { ty: u32, f: NativeFn, argc: usize, is_struct: bool },
    NewObjMissing { key: Rc<str>, argc: usize },
    NewDelegate,
    Ldfld(u32),
    Stfld(u32),
    Ldflda(u32),
    Ldsfld(u32, u32),
    Stsfld(u32, u32),
    Ldsflda(u32, u32),
    LdsfldExt(Rc<str>),
    LdfldExt(Rc<str>),
    Newarr(Ty),
    Ldlen,
    Ldelem,
    Stelem,
    Ldelema,
    Box(Ty),
    Unbox(Ty),
    Castclass(TyOp),
    Isinst(TyOp),
    Ldftn(u32),
    Ldvirtftn(u32),
    Ldind,
    Stind(Ty),
    Ldobj,
    Stobj,
    Initobj(Ty),
    Ldtoken(Rc<str>),
    /// Raw data of a static field (array initializer, RuntimeHelpers.InitializeArray).
    LdtokenData(Rc<Vec<u8>>),
    Throw,
    Rethrow,
    Leave(usize),
    Endfinally,
    Unsupported(Rc<str>),
}

pub enum EhKind {
    Catch(TyOp),
    Filter(usize),
    Finally,
    Fault,
}

pub struct Eh {
    pub kind: EhKind,
    pub try_start: usize,
    pub try_end: usize,
    pub h_start: usize,
    pub h_end: usize,
}

/// Typed signature of an external call (BCL/Unity): input of the C# -> Rust generator.
#[derive(Clone, Debug)]
pub struct ExtMeta {
    pub key: String,
    pub tname: String,
    pub name: String,
    pub params: Vec<Ty>,
    pub ret: Option<Ty>,
    pub instance: bool,
    pub targs: Vec<Ty>,
}

pub struct Code {
    pub ops: Vec<Op>,
    pub nargs: usize,
    pub nlocals: usize,
    pub local_defaults: Vec<Ty>,
    pub local_vals: std::cell::RefCell<Option<Vec<Value>>>,
    pub eh: Vec<Eh>,
    pub dbg: Option<Vec<String>>,
    /// metadata of external calls, indexed like `ops`
    pub ext: Vec<Option<Rc<ExtMeta>>>,
}
