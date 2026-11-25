use super::*;

impl Vm {
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
    pub(super) fn step(&mut self, code: &Code, base: usize, sb: usize, pc: &mut usize, pending: &mut Vec<Pending>) -> R<Option<Value>> {
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
                    self.call_native_args(*f, *argc, *ret, targs)?;
                }
                Op::CallNative { f, argc, ret, targs } => {
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
}
