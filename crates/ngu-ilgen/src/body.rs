use super::*;

impl G<'_> {
    pub(super) fn gen_body(&mut self, mid: u32, ps: &[RT], ret: &RT) -> Result<String, String> {
        let code: std::rc::Rc<Code> = self.vm.lower(mid);
        let prog = self.vm.prog.clone();
        let _ = &prog;
        let regions = exception_regions(&code)?;
        let info_owner = self.vm.prog.methods[mid as usize].owner;
        let _ = info_owner;
        let mi = prog.methods[mid as usize].mi.unwrap();
        let mut locals: Vec<RT> = code.local_defaults.iter().map(|t| self.ty_to_rt(t)).collect();
        if let Some(body) = &prog.res[mi].body {
            for (i, l) in body.header.local_variables.iter().enumerate() {
                if let LocalVariable::Variable { by_ref: false, var_type, .. } = l {
                    locals[i] = self.rt_of(var_type);
                }
            }
        }
        let n = code.ops.len();
        let starts = block_starts(&code, &regions);
        let mut entry: BTreeMap<usize, Vec<RT>> = BTreeMap::new();
        entry.insert(0, vec![]);
        let mut work = vec![0usize];
        let mut arms: BTreeMap<usize, String> = BTreeMap::new();
        let mut spill_decl: BTreeMap<String, String> = BTreeMap::new();
        for r in &regions {
            for c in &r.catches {
                entry.insert(c.0, vec![RT::Obj]);
                spill_decl.insert(format!("x0_{}", sanitize(&self.rs(&RT::Obj))), self.rs(&RT::Obj));
                work.push(c.0);
            }
            if let Some(f) = &r.fin {
                entry.insert(f.0, vec![]);
                work.push(f.0);
            }
        }
        let ret_rs = if *ret == RT::Void { "()".to_string() } else { self.rs(ret) };
        let mut tmp = 0usize;
        while let Some(b) = work.pop() {
            if arms.contains_key(&b) {
                continue;
            }
            let end = starts.iter().copied().find(|s| *s > b).unwrap_or(n);
            let mut st: Vec<(String, RT)> = entry[&b].iter().enumerate().map(|(d, t)| match t {
                RT::Ptr(inner, _) => {
                    let nm = format!("(*x{}_{})", d, sanitize(&self.rs(&RT::Ptr(inner.clone(), String::new()))));
                    (nm.clone(), RT::Ptr(inner.clone(), nm))
                }
                _ => (format!("x{}_{}{}", d, sanitize(&self.rs(t)), if self.is_ref(t) { ".clone()" } else { "" }), t.clone()),
            }).collect();
            let mut o = String::new();
            let mut term = false;
            let mut succ: Vec<usize> = vec![];
            for i in b..end {
                let op = &code.ops[i];
                macro_rules! fail {
                    ($($a:tt)*) => { return Err(format!($($a)*)) };
                }
                macro_rules! pop {
                    () => {
                        match st.pop() {
                            Some(x) => x,
                            None => fail!("empty stack at op {}", i),
                        }
                    };
                }
                macro_rules! push {
                    ($e:expr, $t:expr) => {{
                        tmp += 1;
                        let nm = format!("t{}", tmp);
                        let _ = writeln!(o, "        let {} = {};", nm, $e);
                        st.push((nm, $t));
                    }};
                }
                match op {
                    Op::Nop => {}
                    Op::I4(v) => push!(format!("{}i32", v), RT::I4),
                    Op::I8(v) => push!(format!("{}i64", v), RT::I8),
                    Op::R4(v) => push!(format!("f32::from_bits({}u32)", v.to_bits()), RT::R4),
                    Op::R8(v) => push!(format!("f64::from_bits({}u64)", v.to_bits()), RT::R8),
                    Op::Null => {
                        tmp += 1;
                        st.push(("None".into(), RT::Null));
                    }
                    Op::Str(s) => push!(format!("ngu_rt::str_of({:?})", &**s), RT::Str),
                    Op::Ldarg(a) => {
                        let t = ps.get(*a as usize).cloned().ok_or("ldarg out of range")?;
                        if let RT::Ptr(inner, _) = &t {
                            let pl = format!("(*a{})", a);
                            st.push((pl.clone(), RT::Ptr(inner.clone(), pl)));
                        } else if *a == 0 && !self.vm.prog.methods[mid as usize].is_static
                            && matches!(t, RT::Class(id) if self.vm.types[id as usize].is_value)
                        {
                            push!(format!("a{}.clone()", a), self.norm(&t));
                        } else {
                            push!(self.from_decl(&format!("a{}", a), &t), self.norm(&t));
                        }
                    }
                    Op::Starg(a) => {
                        let (v, vt) = pop!();
                        let t = ps.get(*a as usize).cloned().ok_or("starg")?;
                        let _ = writeln!(o, "        a{} = {};", a, self.to_decl(&v, &vt, &t));
                    }
                    Op::Ldloc(l) => {
                        let t = locals.get(*l as usize).cloned().ok_or("ldloc")?;
                        push!(self.from_decl(&format!("l{}", l), &t), self.norm(&t));
                    }
                    Op::Stloc(l) => {
                        let (v, vt) = pop!();
                        let t = locals.get(*l as usize).cloned().ok_or("stloc")?;
                        let _ = writeln!(o, "        l{} = {};", l, self.to_decl(&v, &vt, &t));
                    }
                    Op::Ldloca(l) => {
                        let t = locals.get(*l as usize).cloned().ok_or("ldloca")?;
                        tmp += 1;
                        st.push((format!("l{}", l), RT::Ptr(Box::new(t), format!("l{}", l))));
                    }
                    Op::Ldarga(a) => {
                        let t = ps.get(*a as usize).cloned().ok_or("ldarga")?;
                        st.push((format!("a{}", a), RT::Ptr(Box::new(t), format!("a{}", a))));
                    }
                    Op::Dup => {
                        let (v, t) = st.last().cloned().ok_or("dup on empty stack")?;
                        if matches!(t, RT::Ptr(..)) {
                            st.push((v, t));
                        } else if self.is_ref(&t) {
                            push!(format!("{}.clone()", v), t);
                        } else {
                            push!(v, t);
                        }
                    }
                    Op::Pop => {
                        pop!();
                    }
                    Op::Add | Op::Sub | Op::Mul | Op::Div(_) | Op::Rem(_) | Op::And | Op::Or | Op::Xor | Op::AddOvf(_) | Op::SubOvf(_) | Op::MulOvf(_) => {
                        let (b2, bt) = pop!();
                        let (a2, at) = pop!();
                        let (mut ea, mut eb, k) = (a2.clone(), b2.clone(), if at == RT::R8 || bt == RT::R8 { RT::R8 } else if at == RT::R4 && bt == RT::R4 { RT::R4 } else if at == RT::I8 || bt == RT::I8 { RT::I8 } else { RT::I4 });
                        if k == RT::I8 {
                            if at == RT::I4 { ea = format!("({} as i64)", a2); }
                            if bt == RT::I4 { eb = format!("({} as i64)", b2); }
                        }
                        let expr = match (&k, op) {
                            (RT::R4 | RT::R8, Op::Add) => format!("{} + {}", ea, eb),
                            (RT::R4 | RT::R8, Op::Sub) => format!("{} - {}", ea, eb),
                            (RT::R4 | RT::R8, Op::Mul) => format!("{} * {}", ea, eb),
                            (RT::R4 | RT::R8, Op::Div(_)) => format!("{} / {}", ea, eb),
                            (RT::R4 | RT::R8, Op::Rem(_)) => format!("{} % {}", ea, eb),
                            (_, Op::Add | Op::AddOvf(_)) => format!("{}.wrapping_add({})", ea, eb),
                            (_, Op::Sub | Op::SubOvf(_)) => format!("{}.wrapping_sub({})", ea, eb),
                            (_, Op::Mul | Op::MulOvf(_)) => format!("{}.wrapping_mul({})", ea, eb),
                            (RT::I8, Op::Div(true)) => format!("(({} as u64).wrapping_div({} as u64)) as i64", ea, eb),
                            (_, Op::Div(true)) => format!("(({} as u32).wrapping_div({} as u32)) as i32", ea, eb),
                            (_, Op::Div(false)) => format!("{}.wrapping_div({})", ea, eb),
                            (RT::I8, Op::Rem(true)) => format!("(({} as u64).wrapping_rem({} as u64)) as i64", ea, eb),
                            (_, Op::Rem(true)) => format!("(({} as u32).wrapping_rem({} as u32)) as i32", ea, eb),
                            (_, Op::Rem(false)) => format!("{}.wrapping_rem({})", ea, eb),
                            (_, Op::And) => format!("{} & {}", ea, eb),
                            (_, Op::Or) => format!("{} | {}", ea, eb),
                            (_, Op::Xor) => format!("{} ^ {}", ea, eb),
                            _ => fail!("arith"),
                        };
                        push!(expr, k);
                    }
                    Op::Shl | Op::Shr(_) => {
                        let (n2, _) = pop!();
                        let (v, vt) = pop!();
                        let e = match op {
                            Op::Shl => format!("{}.wrapping_shl({} as u32)", v, n2),
                            Op::Shr(false) => format!("{}.wrapping_shr({} as u32)", v, n2),
                            _ => if vt == RT::I8 { format!("(({} as u64).wrapping_shr({} as u32)) as i64", v, n2) } else { format!("(({} as u32).wrapping_shr({} as u32)) as i32", v, n2) },
                        };
                        push!(e, vt);
                    }
                    Op::Neg => {
                        let (v, t) = pop!();
                        push!(if matches!(t, RT::R4 | RT::R8) { format!("-{}", v) } else { format!("{}.wrapping_neg()", v) }, t);
                    }
                    Op::Not => {
                        let (v, t) = pop!();
                        push!(format!("!{}", v), t);
                    }
                    Op::Ceq | Op::Cgt(_) | Op::Clt(_) => {
                        let (b2, bt) = pop!();
                        let (a2, at) = pop!();
                        let e = if self.is_ref(&at) || self.is_ref(&bt) {
                            if self.is_ref(&at) != self.is_ref(&bt) && at != RT::Null && bt != RT::Null { fail!("ref/value comparison {:?} {:?}", at, bt); }
                            let k = match op { Op::Ceq => "eq", Op::Cgt(_) => "gt", _ => "lt" };
                            match Self::ref_cmp(k, &a2, &at, &b2, &bt) {
                                Some(c) => format!("({}) as i32", c),
                                None => fail!("reference comparison"),
                            }
                        } else {
                            let (x, y) = (a2.clone(), b2.clone());
                            let floaty = matches!(at, RT::R4 | RT::R8);
                            match op {
                                Op::Ceq => format!("({} == {}) as i32", x, y),
                                Op::Cgt(un) => if floaty { if *un { format!("!({} <= {}) as i32", x, y) } else { format!("({} > {}) as i32", x, y) } } else if *un { let u = if at == RT::I8 || bt == RT::I8 { "u64" } else { "u32" }; format!("(({} as {u}) > ({} as {u})) as i32", x, y, u = u) } else { format!("({} > {}) as i32", x, y) },
                                Op::Clt(un) => if floaty { if *un { format!("!({} >= {}) as i32", x, y) } else { format!("({} < {}) as i32", x, y) } } else if *un { let u = if at == RT::I8 || bt == RT::I8 { "u64" } else { "u32" }; format!("(({} as {u}) < ({} as {u})) as i32", x, y, u = u) } else { format!("({} < {}) as i32", x, y) },
                                _ => unreachable!(),
                            }
                        };
                        push!(e, RT::I4);
                    }
                    Op::Conv(t) => {
                        let (v, vt) = pop!();
                        let (cast, k) = match t {
                            Ty::I8 => ("(({} as i8) as i32)", RT::I4),
                            Ty::U8 | Ty::Bool => ("(({} as u8) as i32)", RT::I4),
                            Ty::I16 => ("(({} as i16) as i32)", RT::I4),
                            Ty::U16 | Ty::Char => ("(({} as u16) as i32)", RT::I4),
                            Ty::I32 => ("({} as i32)", RT::I4),
                            Ty::U32 => ("(({} as u32) as i32)", RT::I4),
                            Ty::I64 | Ty::IntPtr => ("({} as i64)", RT::I8),
                            Ty::U64 | Ty::UIntPtr => if vt == RT::I4 { ("(({} as u32) as i64)", RT::I8) } else { ("({} as i64)", RT::I8) },
                            Ty::F32 => ("({} as f32)", RT::R4),
                            Ty::F64 => ("({} as f64)", RT::R8),
                            _ => fail!("conv"),
                        };
                        let v = if matches!(vt, RT::R4 | RT::R8) {
                            // x86-64 (Mono/.NET): out of range or NaN -> minimum value, no saturation
                            match t {
                                Ty::I8 | Ty::U8 | Ty::Bool | Ty::I16 | Ty::U16 | Ty::Char | Ty::I32 => format!("ngu_rt::f2i32({} as f64)", v),
                                Ty::U32 | Ty::I64 | Ty::IntPtr | Ty::U64 | Ty::UIntPtr => format!("ngu_rt::f2i64({} as f64)", v),
                                _ => v,
                            }
                        } else { v };
                        let cast = if matches!(vt, RT::R4 | RT::R8) { match t { Ty::I32 | Ty::I64 | Ty::IntPtr | Ty::U64 | Ty::UIntPtr => "{}", _ => cast } } else { cast };
                        push!(cast.replace("{}", &v), k);
                    }
                    Op::ConvRUn => {
                        let (v, vt) = pop!();
                        push!(if vt == RT::I8 { format!("({} as u64) as f64", v) } else { format!("({} as u32) as f64", v) }, RT::R8);
                    }
                    Op::Br(t) => {
                        self.spill(&mut st, &mut o, &mut spill_decl);
                        let _ = writeln!(o, "        pc = {}; continue;", t);
                        succ.push(*t);
                        term = true;
                    }
                    Op::Brtrue(t) | Op::Brfalse(t) => {
                        let (v, vt) = pop!();
                        let c = if self.is_ref(&vt) { format!("{}.is_some()", v) } else { format!("{} != 0", v) };
                        let c = if matches!(op, Op::Brfalse(_)) { format!("!({})", c) } else { c };
                        self.spill(&mut st, &mut o, &mut spill_decl);
                        let _ = writeln!(o, "        if {} {{ pc = {}; }} else {{ pc = {}; }} continue;", c, t, i + 1);
                        succ.extend([*t, i + 1]);
                        term = true;
                    }
                    Op::Beq(t) | Op::Bne(t) | Op::Bge(_, t) | Op::Bgt(_, t) | Op::Ble(_, t) | Op::Blt(_, t) => {
                        let (b2, bt) = pop!();
                        let (a2, at) = pop!();
                        let floaty = matches!(at, RT::R4 | RT::R8);
                        let c = if self.is_ref(&at) || self.is_ref(&bt) {
                            if self.is_ref(&at) != self.is_ref(&bt) && at != RT::Null && bt != RT::Null { fail!("ref/value branch {:?} {:?}", at, bt); }
                            let k = match op { Op::Beq(_) => "eq", Op::Bne(_) => "ne", Op::Bge(..) => "ge", Op::Bgt(..) => "gt", Op::Ble(..) => "le", _ => "lt" };
                            match Self::ref_cmp(k, &a2, &at, &b2, &bt) {
                                Some(c) => c,
                                None => fail!("reference branch {} {:?} {:?}", k, at, bt),
                            }
                        } else {
                            let (cmp, un) = match op {
                                Op::Beq(_) => ("==", false),
                                Op::Bne(_) => ("!=", false),
                                Op::Bge(u, _) => (">=", *u),
                                Op::Bgt(u, _) => (">", *u),
                                Op::Ble(u, _) => ("<=", *u),
                                Op::Blt(u, _) => ("<", *u),
                                _ => unreachable!(),
                            };
                            if floaty && (un || matches!(op, Op::Bne(_))) && !matches!(op, Op::Bne(_)) {
                                // unordered: true if NaN
                                let inv = match cmp { ">=" => "<", ">" => "<=", "<=" => ">", _ => ">=" };
                                format!("!({} {} {})", a2, inv, b2)
                            } else if !floaty && un && cmp != "==" && cmp != "!=" {
                                { let u = if at == RT::I8 || bt == RT::I8 { "u64" } else { "u32" }; format!("(({} as {u}) {} ({} as {u}))", a2, cmp, b2, u = u) }
                            } else {
                                format!("{} {} {}", a2, cmp, b2)
                            }
                        };
                        self.spill(&mut st, &mut o, &mut spill_decl);
                        let _ = writeln!(o, "        if {} {{ pc = {}; }} else {{ pc = {}; }} continue;", c, t, i + 1);
                        succ.extend([*t, i + 1]);
                        term = true;
                    }
                    Op::Switch(ts) => {
                        let (v, _) = pop!();
                        self.spill(&mut st, &mut o, &mut spill_decl);
                        let _ = writeln!(o, "        match {} as u32 {{", v);
                        for (k, t) in ts.iter().enumerate() {
                            let _ = writeln!(o, "            {} => {{ pc = {}; }}", k, t);
                            succ.push(*t);
                        }
                        let _ = writeln!(o, "            _ => {{ pc = {}; }}\n        }}\n        continue;", i + 1);
                        succ.push(i + 1);
                        term = true;
                    }
                    Op::Ret => {
                        if *ret == RT::Void {
                            let _ = writeln!(o, "        ret!();");
                        } else {
                            let (v, vt) = pop!();
                            let _ = writeln!(o, "        ret!({});", self.to_decl(&v, &vt, ret));
                        }
                        term = true;
                    }
                    Op::Call(m2) => {
                        let (cps, cret) = self.method_sig(*m2).ok_or("call to a ref/out signature")?;
                        let mut args = vec![];
                        for p in cps.iter().rev() {
                            let (v, vt) = pop!();
                            args.push(self.to_decl(&v, &vt, p));
                        }
                        args.reverse();
                        let inst = !self.vm.prog.methods[*m2 as usize].is_static;
                        if inst {
                            args[0] = format!("{}.clone()", args[0]);
                        }
                        let call = format!("{}({})", self.fname(*m2), args.join(", "));
                        if cret == RT::Void {
                            let _ = writeln!(o, "        {};", call);
                        } else {
                            push!(self.from_decl(&call, &cret), self.norm(&cret));
                        }
                    }
                    Op::CallNative { argc, ret: has_ret, .. } | Op::CallVirtExt { argc, ret: has_ret, .. } | Op::CallMissing { argc, ret: has_ret, .. } => {
                        if code.ext.get(i).and_then(|m| m.as_ref()).map(|m| m.key.ends_with("RuntimeHelpers::InitializeArray")).unwrap_or(false) {
                            let (d, dt) = pop!();
                            let (arr, at) = pop!();
                            if !matches!(&dt, RT::Ext(n, _) if n == "@data") || !matches!(at, RT::Arr(_)) {
                                fail!("InitializeArray inattendu");
                            }
                            let _ = writeln!(o, "        ngu_rt::init_array(&{}, {});", arr, d);
                            continue;
                        }
                        let meta: ExtMeta = code.ext.get(i).and_then(|m| m.as_ref()).map(|m| (**m).clone()).ok_or("meta externe absente")?;
                        let _ = argc;
                        if meta.key == "System.IDisposable::Dispose" && meta.instance {
                            // Dispose of an enumerator / disposable struct: no effect in the generated Rust
                            if matches!(st.last().map(|x| &x.1), Some(RT::Ptr(..))) {
                                pop!();
                                continue;
                            }
                        }
                        let mut args: Vec<(String, RT)> = vec![];
                        let total = meta.params.len() + meta.instance as usize;
                        for _ in 0..total {
                            args.push(pop!());
                        }
                        args.reverse();
                        if meta.key == "System.Linq.Enumerable::ElementAt" {
                            if let [(source, RT::Ext(name, types)), (index, RT::I4)] = args.as_slice() {
                                if name == "System.Collections.Generic.Dictionary`2" && types.len() == 2 {
                                    push!(format!("ngu_rt::ext::dictionary_element_at(&{}, {})", source, index),
                                        RT::Ext("System.Collections.Generic.KeyValuePair`2".into(), types.clone()));
                                    continue;
                                }
                            }
                        }
                        let mut names = vec![];
                        let mut ptys: Vec<String> = vec![];
                        let mut exprs = vec![];
                        let mut recv: Option<RT> = None;
                        let generic_recv = meta.instance && matches!(args.first().map(|a| match &a.1 { RT::Ptr(i, _) => (**i).clone(), o => o.clone() }), Some(RT::Ext(_, ref a)) if !a.is_empty());
                        let class_recv = meta.instance && matches!(args.first().map(|a| &a.1), Some(RT::Class(_)));
                        for (k, (v, vt)) in args.iter().enumerate() {
                            let is_recv = meta.instance && k == 0;
                            if is_recv {
                                match vt {
                                    RT::Ptr(inner, place) => {
                                        names.push(format!("Ref_{}", sanitize(&self.rs(inner))));
                                        if matches!(&**inner, RT::Ext(n, _) if n.starts_with("System.Collections.Generic.List`1/Enumerator")) {
                                            ptys.push(format!("*mut {}", self.rs(inner)));
                                            exprs.push(format!("&mut {} as *mut {}", place, self.rs(inner)));
                                        } else {
                                            ptys.push(self.rs(inner));
                                            exprs.push(place.clone());
                                        }
                                        recv = Some((**inner).clone());
                                    }
                                    _ => {
                                        names.push(if matches!(vt, RT::Class(_)) { "This".to_string() } else { format!("This_{}", sanitize(&self.rs(vt))) });
                                        ptys.push(self.rs(vt));
                                        exprs.push(v.clone());
                                        recv = Some(vt.clone());
                                    }
                                }
                            } else {
                                let p = &meta.params[k - meta.instance as usize];
                                let prt = self.ty_to_rt(p);
                                let ex = if let RT::Ptr(pin, place) = vt { format!("&mut {} as *mut {}", place, self.rs(pin)) } else if generic_recv { let ea = match &recv { Some(RT::Ext(_, ea)) if !ea.is_empty() => Some(ea.clone()), _ => None }; match (ea, &prt) { (Some(ea), RT::Obj) => { let gi = meta.params[..k - 1].iter().filter(|q| matches!(q, Ty::Generic)).count(); self.to_decl(v, vt, &ea[gi.min(ea.len() - 1)]) } (Some(_), p) => self.to_decl(v, vt, p), _ => v.clone() } } else if self.is_ref(vt) { self.to_decl(v, vt, &prt) } else { self.to_decl(v, vt, &prt) };
                                // the declared parameter type fixes the name (overload)
                                let nm = if generic_recv { self.rs(vt) } else { self.rs(&prt) };
                                ptys.push(nm.clone());
                                names.push(sanitize(&nm));
                                exprs.push(ex);
                            }
                        }
                        let gsuf = if meta.targs.is_empty() { String::new() } else { format!("__G_{}", sanitize(&meta.targs.iter().map(|t| match t { Ty::Ext(n, _) => n.clone(), Ty::User(i) => self.vm.types[*i as usize].name.clone(), o => self.rs(&self.ty_to_rt(o)) }).collect::<Vec<_>>().join("_"))) };
                        let fname = if generic_recv { format!("{}__{}", sanitize(&meta.tname), sanitize(&meta.name)) } else { format!("{}__{}__{}", sanitize(&meta.tname), sanitize(&meta.name), names.join("_")) };
                        let rret = if meta.key == "System.Linq.Enumerable::ToDictionary" && meta.targs.len() == 3 {
                            RT::Ext("System.Collections.Generic.Dictionary`2".into(), meta.targs[1..].iter().map(|t| self.ty_to_rt(t)).collect())
                        } else { match &meta.ret {
                            // generic return on a receiver whose type is not statically known: dynamic engine value
                            Some(Ty::Generic) if meta.targs.is_empty() && !matches!(&recv, Some(RT::Ext(_, a)) if !a.is_empty()) => RT::Obj,
                            None => RT::Void,
                            Some(Ty::Generic) if !meta.targs.is_empty() => self.ty_to_rt(&meta.targs[0]),
                            Some(_) if meta.name == "GetEnumerator" && matches!(&recv, Some(RT::Ext(n, a)) if n == "System.Collections.Generic.List`1" && a.len() == 1) => {
                                match &recv { Some(RT::Ext(_, a)) => RT::Ext("System.Collections.Generic.List`1/Enumerator".to_string(), a.clone()), _ => unreachable!() }
                            }
                            Some(t) => {
                                let r = self.ty_to_rt(t);
                                if matches!(r, RT::Obj) { if let Some(RT::Ext(_, a)) = &recv { if meta.name == "get_Key" { a.first().cloned().unwrap_or(RT::Obj) } else { a.last().cloned().unwrap_or(RT::Obj) } } else { RT::Obj } } else { r }
                            }
                        }};
                        let _ = has_ret;
                        let fname = format!("{}{}", fname, gsuf);
                                                let rr = self.rs(&rret);
                        let targs_expr = if meta.targs.is_empty() { "None".to_string() } else { format!("Some(vec![{}])", meta.targs.iter().map(ty_expr).collect::<Vec<_>>().join(", ")) };
                        self.ext_needed.entry(fname.clone()).or_insert((ptys, rr, generic_recv || class_recv, meta.key.clone(), targs_expr, meta.instance));
                        let call = format!("ext::{}({})", fname, exprs.join(", "));
                        if rret == RT::Void {
                            let _ = writeln!(o, "        {};", call);
                        } else {
                            push!(self.from_decl(&call, &rret), self.norm(&rret));
                        }
                    }
                    Op::NewObj { ctor, ty, .. } => {
                        let (cps, _) = self.method_sig(*ctor).ok_or("ctor ref/out")?;
                        let mut args = vec![];
                        for p in cps.iter().skip(1).rev() {
                            let (v, vt) = pop!();
                            args.push(self.to_decl(&v, &vt, p));
                        }
                        args.reverse();
                        tmp += 1;
                        let nm = format!("t{}", tmp);
                        let cn = self.cname(*ty);
                        let _ = writeln!(o, "        let {} = Some(Gc::new({}::default()));", nm, cn);
                        let mut call_args = vec![format!("{}.clone()", nm)];
                        call_args.extend(args);
                        let _ = writeln!(o, "        {}({});", self.fname(*ctor), call_args.join(", "));
                        st.push((nm, RT::Class(*ty)));
                    }
                    Op::NewObjExt { ty, argc, is_struct, .. } => {
                        let mut args = vec![];
                        for _ in 0..*argc {
                            let (v, vt) = pop!();
                            args.push(if vt == RT::Null { "None::<std::rc::Rc<dyn std::any::Any>>".to_string() } else { v });
                        }
                        args.reverse();
                        let tn = self.vm.types[*ty as usize].name.clone();
                        let fname = if *argc == 0 { format!("{}___ctor", sanitize(&tn)) } else { format!("{}___ctor_{}", sanitize(&tn), argc) };
                        self.ext_needed.entry(fname.clone()).or_insert((vec!["Obj".into(); *argc], "Obj".into(), false, format!("{}::.ctor", tn), is_struct.to_string(), false));
                        let rtn = if tn == "System.String" {
                            RT::Str
                        } else if tn == "System.Collections.Generic.List`1" {
                            RT::Ext(tn, vec![RT::Infer])
                        } else if tn == "System.Collections.Generic.Dictionary`2" {
                            RT::Ext(tn, vec![RT::Infer, RT::Infer])
                        } else {
                            RT::Ext(tn, vec![])
                        };
                        let ro = if tn_is_str(&rtn) { "Str".to_string() } else { "Obj".to_string() };
                        let _ = ro;
                        push!(format!("ext::{}({})", fname, args.join(", ")), rtn);
                    }
                    Op::Box(t) => {
                        let (v, vt) = pop!();
                        let bt = self.ty_to_rt(t);
                        if matches!(t, Ty::Ext(_, true)) && self.is_ref(&bt) {
                            fail!("box of struct");
                        }
                        if matches!(bt, RT::Class(id) if self.vm.types[id as usize].is_value) {
                            push!(format!("ngu_host::boxv({})", self.to_decl(&v, &vt, &bt)), RT::Obj);
                        } else if self.is_ref(&bt) {
                            push!(v, vt);
                        } else {
                            push!(format!("ngu_host::boxv({})", self.to_decl(&v, &vt, &bt)), RT::Obj);
                        }
                    }
                    Op::LdsfldExt(key) => {
                        let t = Self::static_field_ty(key).ok_or_else(|| format!("unknown external static field {}", key))?;
                        push!(format!("<{} as FromV>::from_v(&ngu_host::static_field({:?}))", self.rs(&t), &**key), self.norm(&t));
                    }
                    Op::LdfldExt(key) => {
                        let (v, vt) = pop!();
                        let (tn, fnm) = key.rsplit_once("::").ok_or("field key")?;
                        let isv = |r: &RT| matches!(r, RT::Ext(n, _) if Self::valstruct(n).is_some());
                        match &vt {
                            RT::Ext(n, _) if n == "UnityEngine.UI.Toggle" && fnm == "graphic" => {
                                push!(format!("ngu_host::native_field({:?}, ToV::to_v(&{}))", &**key, v), RT::Obj);
                            }
                            r if isv(r) => push!(format!("{}.{}", v, fnm), RT::R4),
                            RT::Ptr(inner, place) if isv(inner) => push!(format!("{}.{}", place, fnm), RT::R4),
                            _ => fail!("external ldfld {} on {:?}", tn, vt),
                        }
                    }
                    Op::Unbox(t) => {
                        let (v, vt) = pop!();
                        let r = self.ty_to_rt(t);
                        let struct_value = matches!(r, RT::Class(id) if self.vm.types[id as usize].is_value);
                        if self.is_ref(&r) && !matches!(r, RT::Obj) && !struct_value { fail!("unbox to {:?}", r); }
                        if !(matches!(vt, RT::Obj) || matches!(&vt, RT::Ext(..))) { fail!("unbox depuis {:?}", vt); }
                        let result = format!("ngu_host::unbox::<{}>(&{})", self.rs(&r), v);
                        if struct_value {
                            push!(format!("{}.as_ref().map(|value| Gc::new(value.get().clone()))", result), self.norm(&r));
                        } else {
                            push!(result, self.norm(&r));
                        }
                    }
                    Op::Ldtoken(n) => push!(format!("ngu_host::type_token({:?})", &**n), RT::Obj),
                    Op::Ldftn(m) => push!(format!("{}u32", m), RT::Ext("@ftn".into(), vec![])),
                    Op::NewDelegate => {
                        let (fp, ft) = pop!();
                        let (tg, tt) = pop!();
                        if !matches!(&ft, RT::Ext(n, _) if n == "@ftn") { fail!("delegate newobj without ldftn"); }
                        let tv = if tt == RT::Null { "ngu_host::clr::value::Value::Null".to_string() } else { format!("ToV::to_v(&{})", tg) };
                        push!(format!("ngu_host::new_delegate({}, {})", tv, fp), RT::Obj);
                    }
                    Op::DelegateInvoke { .. } => {
                        let meta: ExtMeta = code.ext.get(i).and_then(|m| m.as_ref()).map(|m| (**m).clone()).ok_or("delegate meta missing")?;
                        let mut args = vec![];
                        for _ in 0..meta.params.len() {
                            let (v, _) = pop!();
                            args.push(format!("ToV::to_v(&{})", v));
                        }
                        args.reverse();
                        let (d, _) = pop!();
                        let call = format!("ngu_host::invoke_delegate(&{}, vec![{}])", d, args.join(", "));
                        match &meta.ret {
                            None => { let _ = writeln!(o, "        {};", call); }
                            Some(Ty::Generic) => fail!("generic delegate return"),
                            Some(t) => {
                                let r = self.ty_to_rt(t);
                                let value = format!("<{} as FromV>::from_v(&{})", self.rs(&r), call);
                                push!(self.from_decl(&value, &r), self.norm(&r));
                            }
                        }
                    }
                    Op::LdtokenData(b) => {
                        let lit: Vec<String> = b.iter().map(|x| x.to_string()).collect();
                        push!(format!("&[{}u8][..]", lit.join("u8, ")), RT::Ext("@data".into(), vec![]));
                    }
                    Op::Ldsfld(t, f) => {
                        let rts = self.static_rts(*t);
                        let ft = rts.get(*f as usize).cloned().ok_or("unknown static field")?;
                        push!(self.from_decl(&format!("st{}().f{}", t, f), &ft), self.norm(&ft));
                    }
                    Op::Stsfld(t, f) => {
                        let (v, vt) = pop!();
                        let rts = self.static_rts(*t);
                        let ft = rts.get(*f as usize).cloned().ok_or("unknown static field")?;
                        let _ = writeln!(o, "        st{}().f{} = {};", t, f, self.to_decl(&v, &vt, &ft));
                    }
                    Op::Ldfld(slot) => {
                        let (v, vt) = pop!();
                        let c = match vt {
                            RT::Class(c) => c,
                            RT::Ext(ref n, _) if Self::valstruct(n).is_some() => {
                                let fnm = clr::bcl::native_layout(n).0.get(*slot as usize).copied().ok_or("unknown struct field")?;
                                { let ft = Self::vs_field_rt(n); push!(self.from_decl(&format!("{}.{}", v, Self::rfield(fnm)), &ft), self.norm(&ft)); }
                                continue;
                            }
                            RT::Ptr(ref inner, ref place) if matches!(&**inner, RT::Ext(n, _) if Self::valstruct(n).is_some()) => {
                                let n = match &**inner { RT::Ext(n, _) => n.clone(), _ => unreachable!() };
                                let fnm = clr::bcl::native_layout(&n).0.get(*slot as usize).copied().ok_or("unknown struct field")?;
                                { let ft = Self::vs_field_rt(&n); push!(self.from_decl(&format!("{}.{}", place, Self::rfield(fnm)), &ft), self.norm(&ft)); }
                                continue;
                            }
                            _ => fail!("ldfld on non-class receiver {:?}", vt),
                        };
                        let ft = self.field_rts(c).get(*slot as usize).cloned().ok_or("unknown field")?;
                        push!(self.from_decl(&format!("{}.as_ref().expect(\"NullReferenceException\").get().f{}", v, slot), &ft), self.norm(&ft));
                    }
                    Op::Stfld(slot) => {
                        let (v, vt) = pop!();
                        let (o2, ot) = pop!();
                        let c = match ot {
                            RT::Class(c) => c,
                            RT::Ptr(ref inner, ref place) if matches!(&**inner, RT::Ext(n, _) if Self::valstruct(n).is_some()) => {
                                let n = match &**inner { RT::Ext(n, _) => n.clone(), _ => unreachable!() };
                                let fnm = clr::bcl::native_layout(&n).0.get(*slot as usize).copied().ok_or("unknown struct field")?;
                                { let ft = Self::vs_field_rt(&n); let _ = writeln!(o, "        {}.{} = {};", place, Self::rfield(fnm), self.to_decl(&v, &vt, &ft)); }
                                continue;
                            }
                            _ => fail!("stfld on non-class receiver {:?}", ot),
                        };
                        let ft = self.field_rts(c).get(*slot as usize).cloned().ok_or("unknown field")?;
                        let _ = writeln!(o, "        {}.as_ref().expect(\"NullReferenceException\").get().f{} = {};", o2, slot, self.to_decl(&v, &vt, &ft));
                    }
                    Op::Ldflda(slot) => {
                        let (v, vt) = pop!();
                        if let RT::Ptr(ref inner, ref place) = vt {
                            if let RT::Ext(n, _) = &**inner {
                                if Self::valstruct(n).is_some() {
                                    let fnm = clr::bcl::native_layout(n).0.get(*slot as usize).copied().ok_or("unknown struct field")?;
                                    let pl = format!("{}.{}", place, Self::rfield(fnm));
                                    st.push((pl.clone(), RT::Ptr(Box::new(Self::vs_field_rt(n)), pl)));
                                    continue;
                                }
                            }
                        }
                        let c = match &vt {
                            RT::Class(c) => *c,
                            RT::Ptr(inner, _) => match &**inner {
                                RT::Class(c) => *c,
                                _ => fail!("ldflda on non-class receiver {:?}", vt),
                            },
                            _ => fail!("ldflda on non-class receiver {:?}", vt),
                        };
                        let ft = self.field_rts(c).get(*slot as usize).cloned().ok_or("unknown field")?;
                        let pl = format!("{}.as_ref().expect(\"NullReferenceException\").get().f{}", v, slot);
                        st.push((pl.clone(), RT::Ptr(Box::new(ft), pl)));
                    }
                    Op::Ldsflda(t, f) => {
                        let ft = self.static_rts(*t).get(*f as usize).cloned().ok_or("unknown static field")?;
                        let pl = format!("st{}().f{}", t, f);
                        st.push((pl.clone(), RT::Ptr(Box::new(ft), pl)));
                    }
                    Op::Ldelema => {
                        let (i2, _) = pop!();
                        let (a2, at) = pop!();
                        let et = match at { RT::Arr(e) => *e, _ => fail!("ldelema on non-array") };
                        let pl = format!("{}.as_ref().expect(\"NullReferenceException\").borrow_mut()[{} as usize]", a2, i2);
                        st.push((pl.clone(), RT::Ptr(Box::new(et), pl)));
                    }
                    Op::Ldind | Op::Ldobj => {
                        let (_, pt) = pop!();
                        match pt {
                            RT::Ptr(inner, place) => push!(self.from_decl(&place, &inner), self.norm(&inner)),
                            _ => fail!("ldind on non-pointer"),
                        }
                    }
                    Op::Stind(_) | Op::Stobj => {
                        let (v, vt) = pop!();
                        let (_, pt) = pop!();
                        match pt {
                            RT::Ptr(inner, place) => { let _ = writeln!(o, "        {} = {};", place, self.to_decl(&v, &vt, &inner)); }
                            _ => fail!("stind on non-pointer"),
                        }
                    }
                    Op::Rethrow => {
                        let _ = writeln!(o, "        ngu_host::rethrow();");
                        term = true;
                    }
                    Op::Leave(t) => {
                        st.clear();
                        let _ = writeln!(o, "        pc = {}; continue;", t);
                        succ.push(*t);
                        term = true;
                    }
                    Op::Endfinally => {
                        let k = regions.iter().position(|r| matches!(&r.fin, Some(f) if f.0 <= i && i < f.1)).ok_or("endfinally hors finally")?;
                        let _ = writeln!(o, "        match std::mem::replace(&mut pend_{k}, ngu_rt::Pend::None) {{ ngu_rt::Pend::Goto(t) => {{ pc = t; continue; }} ngu_rt::Pend::Ret(v) => {{ ret!(v); }} ngu_rt::Pend::Throw(p) => {{ std::panic::resume_unwind(p); }} ngu_rt::Pend::None => {{ unreachable!(); }} }}", k = k);
                        term = true;
                    }
                    Op::CallStructCtor { .. } => {
                        let meta: ExtMeta = code.ext.get(i).and_then(|m| m.as_ref()).map(|m| (**m).clone()).ok_or("struct ctor meta missing")?;
                        let n = meta.params.len();
                        let mut args = vec![];
                        for k in (0..n).rev() {
                            let (v, vt) = pop!();
                            let prt = self.ty_to_rt(&meta.params[k]);
                            args.push(if matches!(vt, RT::Ptr(..)) { v } else { self.to_decl(&v, &vt, &prt) });
                        }
                        args.reverse();
                        let (_, pt) = pop!();
                        let place = match pt { RT::Ptr(_, pl) => pl, other => fail!("struct ctor on {:?}", other) };
                        let fname = format!("{}___ctor_{}", sanitize(&meta.tname), n);
                        let rt_struct = RT::Ext(meta.tname.clone(), vec![]);
                        let rr = self.rs(&rt_struct);
                        self.ext_needed.entry(fname.clone()).or_insert((vec!["Obj".into(); n], rr, false, format!("{}::.ctor", meta.tname), "true".to_string(), false));
                        let _ = writeln!(o, "        {} = ext::{}({});", place, fname, args.join(", "));
                    }
                    Op::Initobj(_) => {
                        let (_, pt) = pop!();
                        match pt {
                            RT::Ptr(inner, place) => {
                                if let RT::Class(id) = *inner {
                                    if self.vm.types[id as usize].is_value {
                                        let _ = writeln!(o, "        {} = Some(Gc::new({}::default()));", place, self.cname(id));
                                    } else {
                                        let _ = writeln!(o, "        {} = Default::default();", place);
                                    }
                                } else {
                                    let _ = writeln!(o, "        {} = Default::default();", place);
                                }
                            }
                            _ => fail!("initobj on non-pointer"),
                        }
                    }
                    Op::Throw => {
                        let (v, _) = pop!();
                        let _ = writeln!(o, "        ngu_host::throw(ToV::to_v(&{}));", v);
                        term = true;
                    }
                    Op::Isinst(_) | Op::Castclass(_) => {
                        let (v, vt) = pop!();
                        let target = match op {
                            Op::Isinst(t) | Op::Castclass(t) => match t { clr::code::TyOp::Class(c) => Some(RT::Class(*c)), clr::code::TyOp::Prim(Ty::String) => Some(RT::Str), _ => None },
                            _ => None,
                        };
                        let is_is = matches!(op, Op::Isinst(_));
                        let cls_target = match op { Op::Isinst(t) | Op::Castclass(t) => match t { clr::code::TyOp::Class(c) => Some(*c), _ => None }, _ => None };
                        let objlike_vt = matches!(&vt, RT::Obj) || matches!(&vt, RT::Ext(n, _) if Self::valstruct(n).is_none());
                        if let (Some(c), true) = (cls_target, objlike_vt) {
                            let tg = if self.vm.types[c as usize].user.is_some() && !self.vm.types[c as usize].is_enum { RT::Class(c) } else { RT::Ext(self.vm.types[c as usize].name.clone(), vec![]) };
                            let ts = self.rs(&tg);
                            let cexp = if c < self.vm.n_user { c.to_string() } else { format!("ngu_host::cid_of({:?})", self.vm.types[c as usize].name) };
                            let e = if is_is {
                                format!("ngu_host::obj_cast::<{}>(&{}, {}).unwrap_or_default()", ts, v, cexp)
                            } else {
                                format!("(if {}.is_none() {{ Default::default() }} else {{ ngu_host::obj_cast::<{}>(&{}, {}).expect(\"InvalidCastException\") }})", v, ts, v, cexp)
                            };
                            push!(e, tg);
                            continue;
                        }
                        match (target, &vt) {
                            (Some(tg), f) if *f == tg => push!(v, vt),
                            (Some(tg), RT::Obj) => {
                                let ts = self.rs(&tg);
                                push!(if is_is { format!("ngu_host::obj_cast_v::<{}>(&{}).unwrap_or_default()", ts, v) } else { format!("ngu_host::obj_cast_v::<{}>(&{}).expect(\"InvalidCastException\")", ts, v) }, tg);
                            }
                            _ => fail!("isinst/castclass non gere {:?} {}", vt, match op { Op::Isinst(t) | Op::Castclass(t) => match t { clr::code::TyOp::Class(c) => format!("class {}", self.vm.types[*c as usize].name), clr::code::TyOp::Prim(p) => format!("prim {:?}", p), clr::code::TyOp::Array => "array".to_string(), clr::code::TyOp::Any => "any".to_string() }, _ => String::new() }),
                        }
                    }
                    Op::Newarr(t) => {
                        let (n2, _) = pop!();
                        let e = self.ty_to_rt(t);
                        let expr = if let RT::Class(id) = &e {
                            if self.vm.types[*id as usize].is_value {
                                format!("Some(std::rc::Rc::new(std::cell::RefCell::new((0..({n2}).max(0)).map(|_| Some(Gc::new({}::default()))).collect::<Vec<_>>())))", self.cname(*id))
                            } else { format!("ngu_rt::new_arr::<{}>({})", self.rs(&e), n2) }
                        } else { format!("ngu_rt::new_arr::<{}>({})", self.rs(&e), n2) };
                        push!(expr, RT::Arr(Box::new(e)));
                    }
                    Op::Ldlen => {
                        let (a2, _) = pop!();
                        push!(format!("{}.as_ref().expect(\"NullReferenceException\").borrow().len() as i32", a2), RT::I4);
                    }
                    Op::Ldelem => {
                        let (i2, _) = pop!();
                        let (a2, at) = pop!();
                        let et = match at { RT::Arr(e) => *e, _ => fail!("ldelem on non-array") };
                        push!(self.from_decl(&format!("{}.as_ref().expect(\"NullReferenceException\").borrow()[{} as usize]", a2, i2), &et), self.norm(&et));
                    }
                    Op::Stelem => {
                        let (v, vt) = pop!();
                        let (i2, _) = pop!();
                        let (a2, at) = pop!();
                        let et = match at { RT::Arr(e) => *e, _ => fail!("stelem on non-array") };
                        let _ = writeln!(o, "        {}.as_ref().expect(\"NullReferenceException\").borrow_mut()[{} as usize] = {};", a2, i2, self.to_decl(&v, &vt, &et));
                    }
                    other => fail!("operation non geree: {:?}", op_name(other)),
                }
                if term {
                    break;
                }
            }
            if !term {
                // fall through to the next block
                self.spill(&mut st, &mut o, &mut spill_decl);
                let _ = writeln!(o, "        pc = {}; continue;", end);
                succ.push(end);
            }
            // exit stack -> entry of the successors
            let exit: Vec<RT> = st.iter().map(|x| match &x.1 { RT::Ptr(i, _) => RT::Ptr(i.clone(), String::new()), t => t.clone() }).collect();
            for s in succ {
                if s < n && !entry.contains_key(&s) {
                    entry.insert(s, exit.clone());
                } else if s < n && entry[&s] != exit {
                    return Err(format!("diverging stacks at block {}: {:?} vs {:?}", s, entry[&s], exit));
                }
                if s < n {
                    work.push(s);
                }
            }
            arms.insert(b, o);
        }
        let mut s = String::new();
        if arms.values().any(|a| a.contains("@@UPCAST@@") || a.contains("@@PTRSPILL@@")) {
            return Err("class conversion (inheritance/interface)".into());
        }
        for (i, t) in locals.iter().enumerate() {
            let _ = writeln!(s, "    let mut l{}: {} = Default::default();", i, self.rs(t));
        }
        for (nm, ty) in &spill_decl {
            if ty.starts_with("*mut") {
                let _ = writeln!(s, "    let mut {}: {} = std::ptr::null_mut();", nm, ty);
            } else {
                let _ = writeln!(s, "    let mut {}: {} = Default::default();", nm, ty);
            }
        }
        for (k, r) in regions.iter().enumerate() {
            if r.fin.is_some() {
                let _ = writeln!(s, "    let mut pend_{}: ngu_rt::Pend<{}> = ngu_rt::Pend::None;", k, ret_rs);
            }
        }
        // context of each block = innermost enclosing try region
        let nreg = regions.len();
        let parent: Vec<Option<usize>> = (0..nreg)
            .map(|k| {
                (0..nreg)
                    .filter(|&j| j != k && regions[j].ts <= regions[k].ts && regions[k].te <= regions[j].te && (regions[j].te - regions[j].ts) > (regions[k].te - regions[k].ts))
                    .min_by_key(|&j| regions[j].te - regions[j].ts)
            })
            .collect();
        let ctx_of: BTreeMap<usize, Option<usize>> = arms
            .keys()
            .map(|&b| (b, (0..nreg).filter(|&k| regions[k].ts <= b && b < regions[k].te).min_by_key(|&k| regions[k].te - regions[k].ts)))
            .collect();
        let cnames: Vec<String> = self.vm.types.iter().map(|t| t.name.clone()).collect();
        let eh = EhGen { cnames: &cnames, n_user: self.vm.n_user, regions: &regions, parent: &parent, ctx_of: &ctx_of, arms: &arms, ret_rs: &ret_rs };
        let _ = writeln!(s, "    ngu_host::tm({});", mid);
        let _ = writeln!(s, "    unsafe {{");
        let _ = writeln!(s, "    macro_rules! ret {{ () => {{ return }}; ($e:expr) => {{ return $e }}; }}");
        let _ = writeln!(s, "    let mut pc: usize = 0;");
        s.push_str(&eh.gen_ctx(None));
        let _ = writeln!(s, "    }}");
        Ok(s)
    }

    /// Writes the remaining stack into x{depth}_{type} variables (values live between blocks).
    fn spill(&self, st: &mut Vec<(String, RT)>, o: &mut String, decl: &mut BTreeMap<String, String>) {
        for (d, (v, t)) in st.iter_mut().enumerate() {
            if let RT::Ptr(inner, place) = t.clone() {
                // pointer live between blocks: kept in a `*mut T` variable
                let nm = format!("x{}_{}", d, sanitize(&self.rs(&RT::Ptr(inner.clone(), String::new()))));
                let expect = format!("(*{})", nm);
                if place != expect {
                    decl.insert(nm.clone(), format!("*mut {}", self.rs(&inner)));
                    let _ = writeln!(o, "        {} = &mut {} as *mut {};", nm, place, self.rs(&inner));
                }
                *v = expect.clone();
                *t = RT::Ptr(inner, expect);
                continue;
            }
            let nm = format!("x{}_{}", d, sanitize(&self.rs(t)));
            if *v != nm {
                let ty = if *t == RT::Null { "Obj".to_string() } else { self.rs(t) };
                decl.insert(nm.clone(), ty);
                let _ = writeln!(o, "        {} = {};", nm, v);
                *v = nm;
            }
        }
    }
}

fn exception_regions(code: &Code) -> Result<Vec<Region>, String> {
    // try regions (grouped by range), catch / finally / fault handlers
    let mut regions: Vec<Region> = vec![];
    for e in &code.eh {
        let idx = match regions.iter().position(|r| r.ts == e.try_start && r.te == e.try_end) {
            Some(i) => i,
            None => {
                regions.push(Region { ts: e.try_start, te: e.try_end, catches: vec![], fin: None });
                regions.len() - 1
            }
        };
        match &e.kind {
            EhKind::Catch(t) => regions[idx].catches.push((e.h_start, e.h_end, match t { clr::code::TyOp::Class(c) => Some(*c), _ => None })),
            EhKind::Finally => regions[idx].fin = Some((e.h_start, e.h_end, false)),
            EhKind::Fault => regions[idx].fin = Some((e.h_start, e.h_end, true)),
            EhKind::Filter(_) => return Err("filtre d'exception".into()),
        }
    }
    if regions.iter().any(|r| r.fin.is_some() && !r.catches.is_empty()) {
        return Err("catch and finally on the same range".into());
    }
    Ok(regions)
}

fn block_starts(code: &Code, regions: &[Region]) -> Vec<usize> {
    let n = code.ops.len();
    // block leaves
    let mut leaders: BTreeSet<usize> = BTreeSet::new();
    leaders.insert(0);
    for (i, op) in code.ops.iter().enumerate() {
        match op {
            Op::Br(t) | Op::Brtrue(t) | Op::Brfalse(t) | Op::Beq(t) | Op::Bne(t) | Op::Bge(_, t) | Op::Bgt(_, t) | Op::Ble(_, t) | Op::Blt(_, t) => {
                leaders.insert(*t);
                leaders.insert(i + 1);
            }
            Op::Switch(ts) => {
                for t in ts {
                    leaders.insert(*t);
                }
                leaders.insert(i + 1);
            }
            Op::Ret | Op::Throw | Op::Leave(_) | Op::Endfinally | Op::Rethrow => {
                leaders.insert(i + 1);
                if let Op::Leave(t) = op {
                    leaders.insert(*t);
                }
            }
            _ => {}
        }
    }
    for r in regions {
        leaders.insert(r.ts);
        leaders.insert(r.te);
        for c in &r.catches {
            leaders.insert(c.0);
            leaders.insert(c.1);
        }
        if let Some(f) = &r.fin {
            leaders.insert(f.0);
            leaders.insert(f.1);
        }
    }
    leaders.retain(|x| *x < n);
    leaders.into_iter().collect()
}
