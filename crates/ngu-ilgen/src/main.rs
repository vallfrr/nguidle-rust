//! ngu-ilgen: C# (IL) -> Rust from the resolved IR of `clr`. Input: the user's Assembly-CSharp.dll.
//! Usage : ngu-ilgen <dll> <dossier-sortie> <Classe1,Classe2|ALL>
//! Outputs: gen.rs (code), ext_stubs.rs (missing BCL), coverage.txt (untranslated methods + reason).
mod body;

use clr::code::{Code, EhKind, ExtMeta, Op};
use clr::program::Ty;
use clr::Vm;
use dotnetdll::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

#[derive(Clone, PartialEq, Debug)]
enum RT {
    Void, Bool, Char, I1, U1, I2, U2, I4, U4, I8, U8, R4, R8, Str, Obj, Null,
    Arr(Box<RT>),
    Class(u32),
    Ext(String, Vec<RT>),
    /// type to be inferred by rustc (generic arguments of a `new List<T>()` whose T is only known later)
    Infer,
    Ptr(Box<RT>, String), // pointer to a local/arg slot (Rust name)
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

struct G<'a> {
    vm: &'a mut Vm,
    ext_needed: BTreeMap<String, (Vec<String>, String, bool, String, String, bool)>,
    translated: BTreeSet<u32>,
    coverage: Vec<String>,
    ok: usize,
    fail: usize,
}

impl<'a> G<'a> {
    fn cname(&self, t: u32) -> String {
        format!("C{}_{}", t, sanitize(&self.vm.types[t as usize].name))
    }
    fn gen_ext(&self, base: &UserType, n: Vec<RT>) -> Option<RT> {
        match base {
            UserType::Reference(r) => Some(RT::Ext(clr::program::typeref_name(&self.vm.prog.res, *r), n)),
            _ => None,
        }
    }
    fn rt_of(&self, t: &MethodType) -> RT {
        if let MethodType::Base(b) = t {
            match &**b {
                BaseType::Type { source: TypeSource::Generic { base, parameters }, .. } => {
                    if let Some(r) = self.gen_ext(base, parameters.iter().map(|p| self.rt_of(p)).collect()) {
                        return r;
                    }
                }
                BaseType::Vector(_, e) => return RT::Arr(Box::new(self.rt_of(e))),
                _ => {}
            }
        }
        self.ty_to_rt(&self.vm.prog.ty_of(t))
    }
    fn rt_of_member(&self, t: &MemberType) -> RT {
        if let MemberType::Base(b) = t {
            match &**b {
                BaseType::Type { source: TypeSource::Generic { base, parameters }, .. } => {
                    if let Some(r) = self.gen_ext(base, parameters.iter().map(|p| self.rt_of_member(p)).collect()) {
                        return r;
                    }
                }
                BaseType::Vector(_, e) => return RT::Arr(Box::new(self.rt_of_member(e))),
                _ => {}
            }
        }
        self.ty_to_rt(&self.vm.prog.ty_of_member(t))
    }
    fn rs(&self, t: &RT) -> String {
        match t {
            RT::Void => "()".into(),
            RT::Bool => "bool".into(),
            RT::Char | RT::U2 => "u16".into(),
            RT::I1 => "i8".into(),
            RT::U1 => "u8".into(),
            RT::I2 => "i16".into(),
            RT::I4 => "i32".into(),
            RT::U4 => "u32".into(),
            RT::I8 => "i64".into(),
            RT::U8 => "u64".into(),
            RT::R4 => "f32".into(),
            RT::R8 => "f64".into(),
            RT::Str => "Str".into(),
            RT::Obj | RT::Null => "Obj".into(),
            RT::Arr(e) => format!("Arr<{}>", self.rs(e)),
            RT::Class(c) => format!("Option<Gc<{}>>", self.cname(*c)),
            RT::Ext(n, a) => match (n.as_str(), a.len()) {
                ("System.Collections.Generic.List`1", 1) => format!("List<{}>", self.rs(&a[0])),
                ("System.Collections.Generic.Dictionary`2", 2) => format!("Dict<{}, {}>", self.rs(&a[0]), self.rs(&a[1])),
                ("System.Collections.Generic.KeyValuePair`2", 2) => format!("({}, {})", self.rs(&a[0]), self.rs(&a[1])),
                (n2, 1) if n2 == "System.Collections.Generic.List`1/Enumerator" => format!("ListEnum<{}>", self.rs(&a[0])),
                _ => Self::valstruct(n).unwrap_or("Obj").into(),
            },
            RT::Ptr(t, _) => format!("*mut {}", self.rs(t)),
            RT::Infer => "_".into(),
        }
    }
    fn norm(&self, t: &RT) -> RT {
        match t {
            RT::Bool | RT::Char | RT::I1 | RT::U1 | RT::I2 | RT::U2 | RT::I4 | RT::U4 => RT::I4,
            RT::U8 => RT::I8,
            RT::Ext(name, _) if clr::bcl::is_delegate_type(name) => RT::Obj,
            o => o.clone(),
        }
    }
    fn is_ref(&self, t: &RT) -> bool {
        match t {
            RT::Ext(n, _) => Self::valstruct(n).is_none() && !n.starts_with("System.Collections.Generic.KeyValuePair") && !n.starts_with("System.Collections.Generic.List`1/Enumerator"),
            _ => matches!(t, RT::Str | RT::Obj | RT::Null | RT::Arr(_) | RT::Class(_)),
        }
    }
    fn valstruct(n: &str) -> Option<&'static str> {
        Some(match n {
            "UnityEngine.Color" => "Color",
            "UnityEngine.Vector2" => "Vector2",
            "UnityEngine.Vector3" => "Vector3",
            "UnityEngine.Vector4" => "Vector4",
            "UnityEngine.Quaternion" => "Quaternion",
            "UnityEngine.Color32" => "Color32",
            _ => return None,
        })
    }
    fn ty_to_rt(&self, t: &Ty) -> RT {
        match t {
            Ty::Bool => RT::Bool,
            Ty::Char => RT::Char,
            Ty::I8 => RT::I1,
            Ty::U8 => RT::U1,
            Ty::I16 => RT::I2,
            Ty::U16 => RT::U2,
            Ty::I32 => RT::I4,
            Ty::U32 => RT::U4,
            Ty::I64 | Ty::IntPtr => RT::I8,
            Ty::U64 | Ty::UIntPtr => RT::U8,
            Ty::F32 => RT::R4,
            Ty::F64 => RT::R8,
            Ty::String => RT::Str,
            Ty::Array(e) => RT::Arr(Box::new(self.ty_to_rt(e))),
            Ty::User(i) => if self.vm.types[*i as usize].is_enum { RT::I4 } else { RT::Class(*i) },
            Ty::Ext(n, _) => match n.as_str() {
                "System.Object" => RT::Obj,
                "System.String" => RT::Str,
                "System.Single" => RT::R4,
                "System.Double" => RT::R8,
                "System.Int32" => RT::I4,
                "System.Int64" => RT::I8,
                "System.Boolean" => RT::Bool,
                "System.Char" => RT::Char,
                "System.UInt32" => RT::U4,
                "System.Byte" => RT::U1,
                _ if matches!(n.rsplit(|c| c == '.' || c == '/').next(), Some("RuntimePlatform" | "InputButton" | "DialogResult" | "KeyCode" | "CipherMode" | "PaddingMode" | "FontStyle" | "TextAnchor" | "FileMode" | "FileAccess" | "DayOfWeek" | "FileShare")) => RT::I4,
                _ => RT::Ext(n.clone(), vec![]),
            },
            Ty::Object | Ty::Generic | Ty::Pointer => RT::Obj,
            _ => RT::Obj,
        }
    }
    /// Conversion of a (normalised) stack value to a declared type.
    fn to_decl(&self, v: &str, from: &RT, to: &RT) -> String {
        let objlike = |t: &RT| match t { RT::Obj => true, RT::Ext(n, _) => Self::valstruct(n).is_none() && !n.starts_with("System.Collections.Generic.KeyValuePair") && !n.starts_with("System.Collections.Generic.List`1/Enumerator"), _ => false };
        if objlike(to) {
            return match from {
                f if objlike(f) => v.to_string(),
                RT::Null => "None::<std::rc::Rc<dyn std::any::Any>>".to_string(),
                _ => format!("ngu_host::boxv({})", v),
            };
        }
        if let (RT::Ptr(pin, _), RT::Ptr(..)) = (from, to) {
            return format!("&mut {} as *mut {}", v, self.rs(pin));
        }
        if let (RT::Class(a), RT::Class(b)) = (from, to) {
            if a != b { return format!("{}@@UPCAST@@", v); }
        }
        match to {
            RT::Bool => format!("({} != 0)", v),
            RT::Char | RT::U2 => format!("({} as u16)", v),
            RT::I1 => format!("({} as i8)", v),
            RT::U1 => format!("({} as u8)", v),
            RT::I2 => format!("({} as i16)", v),
            RT::U4 => format!("({} as u32)", v),
            RT::U8 => format!("({} as u64)", v),
            RT::I4 => format!("{}", v),
            RT::I8 => match from {
                RT::I4 => format!("({} as i64)", v),
                _ => v.to_string(),
            },
            _ => v.to_string(),
        }
    }
    fn from_decl(&self, v: &str, t: &RT) -> String {
        match t {
            RT::Bool | RT::Char | RT::I1 | RT::U1 | RT::I2 | RT::U2 | RT::U4 => format!("({} as i32)", v),
            RT::U8 => format!("({} as i64)", v),
            RT::Class(id) if self.vm.types[*id as usize].is_value =>
                format!("{}.as_ref().map(|value| Gc::new(value.get().clone()))", v),
            RT::Str | RT::Obj | RT::Arr(_) | RT::Class(_) | RT::Ext(..) => format!("{}.clone()", v),
            _ => v.to_string(),
        }
    }

    /// name of the native field (r,g,b,a,x,y,z,w) as defined in `ngu-rt::unity`
    /// type of a field of a Unity value struct
    fn vs_field_rt(n: &str) -> RT {
        if n == "UnityEngine.Color32" { RT::U1 } else { RT::R4 }
    }
    fn rfield(n: &str) -> String {
        n.to_string()
    }
    fn static_field_ty(key: &str) -> Option<RT> {
        Some(match key {
            k if k.starts_with("UnityEngine.Vector3::") => RT::Ext("UnityEngine.Vector3".into(), vec![]),
            k if k.starts_with("UnityEngine.Vector2::") => RT::Ext("UnityEngine.Vector2".into(), vec![]),
            k if k.starts_with("UnityEngine.Color::") => RT::Ext("UnityEngine.Color".into(), vec![]),
            "UnityEngine.Quaternion::identity" => RT::Ext("UnityEngine.Quaternion".into(), vec![]),
            "System.String::Empty" => RT::Str,
            "System.Int32::MaxValue" | "System.Int32::MinValue" => RT::I4,
            "System.Int64::MaxValue" | "System.Int64::MinValue" => RT::I8,
            "System.Double::MaxValue" | "System.Double::MinValue" | "System.Double::PositiveInfinity" | "System.Double::NegativeInfinity" | "System.Double::NaN" | "System.Double::Epsilon" | "System.Math::PI" | "System.Math::E" => RT::R8,
            "System.Single::MaxValue" | "System.Single::MinValue" | "System.Single::PositiveInfinity" | "System.Single::NegativeInfinity" | "UnityEngine.Mathf::PI" | "UnityEngine.Mathf::Infinity" => RT::R4,
            _ => return None,
        })
    }

    fn ref_cmp(k: &str, a: &str, at: &RT, b: &str, bt: &RT) -> Option<String> {
        let (an, bn) = (*at == RT::Null, *bt == RT::Null);
        Some(match (k, an, bn) {
            ("eq", false, true) => format!("{}.is_none()", a),
            ("eq", true, false) => format!("{}.is_none()", b),
            ("eq", true, true) => "true".into(),
            ("eq", _, _) => format!("ngu_rt::ref_eq(&{}, &{})", a, b),
            ("ne", _, _) => format!("!({})", Self::ref_cmp("eq", a, at, b, bt)?),
            ("gt", false, true) => format!("{}.is_some()", a),
            ("lt", true, false) => format!("{}.is_some()", b),
            ("le", false, true) => format!("{}.is_none()", a),
            ("ge", true, false) => format!("{}.is_none()", b),
            _ => return None,
        })
    }

    fn method_sig(&self, mid: u32) -> Option<(Vec<RT>, RT)> {
        let info = &self.vm.prog.methods[mid as usize];
        let mi = info.mi?;
        let m = &self.vm.prog.res[mi];
        let mut ps = vec![];
        if m.signature.instance {
            ps.push(RT::Class(info.owner));
        }
        for p in &m.signature.parameters {
            match &p.1 {
                dotnetdll::resolved::signature::ParameterType::Value(t) => ps.push(self.rt_of(t)),
                dotnetdll::resolved::signature::ParameterType::Ref(t) => ps.push(RT::Ptr(Box::new(self.rt_of(t)), String::new())),
                _ => return None,
            }
        }
        let ret = match &m.signature.return_type.1 {
            None => RT::Void,
            Some(dotnetdll::resolved::signature::ParameterType::Value(t)) => self.rt_of(t),
            _ => return None,
        };
        Some((ps, ret))
    }

    fn fname(&self, mid: u32) -> String {
        let i = &self.vm.prog.methods[mid as usize];
        format!("m{}_{}_{}", mid, sanitize(&self.vm.types[i.owner as usize].name), sanitize(&i.name))
    }

    fn field_rts(&self, tid: u32) -> Vec<RT> {
        let t = &self.vm.types[tid as usize];
        let mut v: Vec<RT> = t.field_tys.iter().map(|x| self.ty_to_rt(x)).collect();
        if let Some(ti) = t.user {
            let own: Vec<RT> = self.vm.prog.res.enumerate_fields(ti).filter(|(_, f)| !f.static_member).map(|(_, f)| self.rt_of_member(&f.return_type)).collect();
            let off = v.len().saturating_sub(own.len());
            for (i, o) in own.into_iter().enumerate() {
                v[off + i] = o;
            }
        }
        if let Some(b) = t.base {
            if self.vm.types[b as usize].user.is_some() {
                for (i, x) in self.field_rts(b).into_iter().enumerate() {
                    v[i] = x;
                }
            }
        }
        v
    }
    fn static_rts(&self, tid: u32) -> Vec<RT> {
        let t = &self.vm.types[tid as usize];
        let mut v: Vec<RT> = t.static_tys.iter().map(|x| self.ty_to_rt(x)).collect();
        if let Some(ti) = t.user {
            let mut i = 0;
            for (_, f) in self.vm.prog.res.enumerate_fields(ti) {
                if f.static_member {
                    v[i] = self.rt_of_member(&f.return_type);
                    i += 1;
                }
            }
        }
        v
    }

    fn gen_types(&self, tid: u32, out: &mut String) {
        let cn = self.cname(tid);
        let fr = self.field_rts(tid);
        let derive = if self.vm.types[tid as usize].is_value { "" } else { "#[derive(Clone)]" };
        let _ = writeln!(out, "{derive}
pub struct {} {{
    pub __h: u32,", cn);
        for (i, t) in fr.iter().enumerate() {
            let _ = writeln!(out, "    pub f{}: {},", i, self.rs(t));
        }
        let _ = writeln!(out, "}}");
        let _ = writeln!(out, "impl Default for {cn} {{ fn default() -> Self {{ Self {{ __h: 0,");
        for (i, t) in fr.iter().enumerate() {
            if let RT::Class(id) = t {
                if self.vm.types[*id as usize].is_value {
                    let _ = writeln!(out, "f{i}: Some(Gc::new({}::default())),", self.cname(*id));
                    continue;
                }
            }
            let _ = writeln!(out, "f{i}: Default::default(),");
        }
        let _ = writeln!(out, "}} }} }}");
        if self.vm.types[tid as usize].is_value {
            let _ = writeln!(out, "impl Clone for {cn} {{ fn clone(&self) -> Self {{ Self {{ __h: 0,");
            for (i, t) in fr.iter().enumerate() {
                if let RT::Class(id) = t {
                    if self.vm.types[*id as usize].is_value {
                        let _ = writeln!(out, "f{i}: self.f{i}.as_ref().map(|value| Gc::new(value.get().clone())),");
                        continue;
                    }
                }
                let _ = writeln!(out, "f{i}: self.f{i}.clone(),");
            }
            let _ = writeln!(out, "}} }} }}");
        }
        let sr = self.static_rts(tid);
        let _ = writeln!(out, "#[derive(Default)]
pub struct S{} {{", cn);
        for (i, t) in sr.iter().enumerate() {
            let _ = writeln!(out, "    pub f{}: {},", i, self.rs(t));
        }
        let init = match self.vm.types[tid as usize].cctor {
            Some(c) => format!(" {}();", self.fname(c)),
            None => String::new(),
        };
        let _ = writeln!(out, "}}
static mut ST{t}: Option<S{c}> = None;
#[allow(static_mut_refs)]
pub fn st{t}() -> &'static mut S{c} {{ unsafe {{ if ST{t}.is_none() {{ ST{t} = Some(Default::default());{i} }} ST{t}.as_mut().unwrap() }} }}
", t = tid, c = cn, i = init);
    }

    /// impl UserClass + method dispatch for the engine.
    fn gen_registry(&self, out: &mut String) {
        for t in 0..self.vm.n_user {
            if self.vm.types[t as usize].is_enum {
                continue;
            }
            let cn = self.cname(t);
            let n = self.field_rts(t).len();
            let ctor = self.vm.types[t as usize].methods.get(".ctor").and_then(|v| v.iter().copied().find(|&m| self.vm.prog.methods[m as usize].argc == 1)).filter(|m| self.translated.contains(m));
            let call = match ctor { Some(c) => format!("{}(Some(g.clone()));", self.fname(c)), None => String::new() };
            let mut exp = String::new();
            for i in 0..n {
                let _ = write!(exp, "if {i} < f.len() {{ f[{i}] = ToV::to_v(&g.get().f{i}); }} ", i = i);
            }
            let mut imp = String::new();
            for i in 0..n {
                let skip = self.vm.types[t as usize].field_skip.get(i).copied().unwrap_or(false);
                let _ = write!(imp, "if {i} < f.len() && (if all {{ !{skip} }} else {{ !ngu_host::is_default(&f[{i}]) }}) {{ g.get().f{i} = FromV::from_v(&f[{i}]); }} ", i = i, skip = skip);
            }
            let _ = writeln!(out, "impl ngu_host::UserClass for {cn} {{
    const CID: u32 = {t};
    fn construct() -> Gc<Self> {{ let g = Gc::new({cn}::default()); {call} g }}
    fn import(g: &Gc<Self>, f: &[ngu_host::clr::value::Value], all: bool) {{ {imp} }}
    fn blank() -> Gc<Self> {{ Gc::new({cn}::default()) }}
    fn export(g: &Gc<Self>, f: &mut Vec<ngu_host::clr::value::Value>) {{ {exp} }}
    fn hget(&self) -> u32 {{ self.__h }}
    fn hset(&mut self, h: u32) {{ self.__h = h; }}
}}", cn = cn, t = t, call = call, imp = imp, exp = exp);
        }
        // differential fuzz table: static methods with primitive signatures (oracle vs native, see `ngu-gen fuzz`)
        let _ = writeln!(out, "pub const FUZZ: &[(u32, &str, &str)] = &[");
        for &m in &self.translated {
            let info = &self.vm.prog.methods[m as usize];
            if !info.is_static {
                continue;
            }
            let (ps, ret) = match self.method_sig(m) { Some(x) => x, None => continue };
            let k = |t: &RT| -> Option<char> {
                Some(match t { RT::I4 => 'i', RT::I8 => 'l', RT::R4 => 'f', RT::R8 => 'd', RT::Bool => 'b', RT::Str => 's', _ => return None })
            };
            let pk: Option<String> = ps.iter().map(|p| k(p)).collect();
            let rk = if ret == RT::Void { Some('v') } else { k(&ret) };
            if let (Some(pk), Some(rk)) = (pk, rk) {
                let _ = writeln!(out, "    ({}, {:?}, {:?}),", m, pk, rk.to_string());
            }
        }
        let _ = writeln!(out, "];");
        let _ = writeln!(out, "pub fn invoke_values(mid: u32, a: &[ngu_host::clr::value::Value]) -> Option<ngu_host::clr::value::Value> {{
    use ngu_host::clr::value::Value;
    Some(match mid {{");
        for &m in &self.translated {
            let (ps, ret) = match self.method_sig(m) { Some(x) => x, None => continue };
            if ps.iter().any(|p| matches!(p, RT::Ptr(..))) {
                continue;
            }
            let args: Vec<String> = (0..ps.len()).map(|i| format!("FromV::from_v(&a[{}])", i)).collect();
            let call = format!("{}({})", self.fname(m), args.join(", "));
            if ret == RT::Void {
                let _ = writeln!(out, "        {} => {{ {}; Value::Null }}", m, call);
            } else {
                let _ = writeln!(out, "        {} => ToV::to_v(&{}),", m, call);
            }
        }
        let _ = writeln!(out, "        _ => return None,
    }})
}}
pub fn hook(vm: *mut ngu_host::clr::Vm, mid: u32, a: &[ngu_host::clr::value::Value]) -> Option<Result<ngu_host::clr::value::Value, ngu_host::clr::value::Value>> {{
    ngu_host::set_vm(vm);
    ngu_host::trace_invoke(vm, mid);
    match ngu_host::try_catch(|| invoke_values(mid, a)) {{
        Ok(r) => r.map(Ok),
        Err(p) => Some(Err(ngu_host::to_exc(&p))),
    }}
}}");
    }

    fn gen_method(&mut self, mid: u32) -> String {
        let name = self.fname(mid);
        let (ps, ret) = match self.method_sig(mid) {
            Some(x) => x,
            None => {
                self.fail += 1;
                self.coverage.push(format!("{}: signature with ref/out", name));
                return String::new();
            }
        };
        let mut sig = format!("pub fn {}(", name);
        for (i, p) in ps.iter().enumerate() {
            let _ = write!(sig, "{}mut a{}: {}", if i > 0 { ", " } else { "" }, i, self.rs(p));
        }
        let rtxt = if ret == RT::Void { String::new() } else { format!(" -> {}", self.rs(&ret)) };
        let _ = write!(sig, "){}", rtxt);
        let has_body = self.vm.prog.res[self.vm.prog.methods[mid as usize].mi.unwrap()].body.is_some();
        if !has_body {
            return format!("#[allow(unused_variables)]
pub {} {{ unimplemented!(\"externe/abstrait\") }}
", &sig[4..]);
        }
        match self.gen_body(mid, &ps, &ret) {
            Ok(body) => {
                self.ok += 1;
                self.translated.insert(mid);
                let fast_path = self.vita_idle_wishes(mid).unwrap_or_default();
                format!("#[allow(unused_variables, unused_mut, unused_assignments, unreachable_code, unused_parens)]\n{} {{\n{}{}}}\n", sig, fast_path, body)
            }
            Err(e) => {
                self.fail += 1;
                self.coverage.push(format!("{}: {}", name, e));
                format!("pub {} {{ unimplemented!(\"{}\") }}\n", &sig[4..], e.replace('"', "'"))
            }
        }
    }

    fn vita_idle_wishes(&self, mid: u32) -> Option<String> {
        if self.vm.prog.methods[mid as usize].guard != 3 { return None; }
        let slot = |ty: &str, field: &str| {
            let ty = *self.vm.type_ids.get(ty)?;
            self.vm.types[ty as usize].fields.iter().position(|name| name == field)
        };
        let player = slot("WishesController", "character")?;
        let wishes = slot("Character", "wishes")?;
        let entries = slot("Wishes", "wishes")?;
        let energy = slot("Wish", "energy")?;
        let magic = slot("Wish", "magic")?;
        let resource = slot("Wish", "res3")?;
        Some(format!(r#"    #[cfg(target_os = "vita")]
    if let Some(entries) = a0.as_ref().and_then(|c| c.get().f{player}.as_ref())
        .and_then(|c| c.get().f{wishes}.as_ref()).and_then(|c| c.get().f{entries}.as_ref())
    {{
        if entries.borrow().iter().all(|entry| entry.as_ref().is_some_and(|wish| {{
            let wish = wish.get();
            wish.f{energy} == 0 || wish.f{magic} == 0 || wish.f{resource} == 0
        }})) {{ return; }}
    }}
"#))
    }
}

fn ty_expr(t: &Ty) -> String {
    match t {
        Ty::User(i) => format!("ngu_host::clr::program::Ty::User({})", i),
        Ty::Ext(n, v) => format!("ngu_host::clr::program::Ty::Ext({:?}.to_string(), {})", n, v),
        Ty::Array(e) => format!("ngu_host::clr::program::Ty::Array(Box::new({}))", ty_expr(e)),
        Ty::String => "ngu_host::clr::program::Ty::String".into(),
        Ty::Object => "ngu_host::clr::program::Ty::Object".into(),
        Ty::I32 => "ngu_host::clr::program::Ty::I32".into(),
        Ty::F32 => "ngu_host::clr::program::Ty::F32".into(),
        _ => "ngu_host::clr::program::Ty::Generic".into(),
    }
}

struct Region {
    ts: usize,
    te: usize,
    /// (start, end, caught class; None = everything)
    catches: Vec<(usize, usize, Option<u32>)>,
    /// (start, end, is_fault)
    fin: Option<(usize, usize, bool)>,
}

struct EhGen<'a> {
    cnames: &'a [String],
    n_user: u32,
    regions: &'a [Region],
    parent: &'a [Option<usize>],
    ctx_of: &'a BTreeMap<usize, Option<usize>>,
    arms: &'a BTreeMap<usize, String>,
    ret_rs: &'a str,
}

impl<'a> EhGen<'a> {
    /// stable class id: native types are designated by name (ids depend on creation order)
    fn cexp(&self, c: u32) -> String {
        if c < self.n_user { c.to_string() } else { format!("ngu_host::cid_of({:?})", self.cnames[c as usize]) }
    }
    /// `loop { match pc { .. } }` loop of a context (None = function body, Some(k) = inside try k).
    fn gen_ctx(&self, ctx: Option<usize>) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "    loop {{ match pc {{");
        for (b, text) in self.arms {
            if self.ctx_of[b] == ctx {
                let _ = writeln!(s, "        {} => {{
{}        }}", b, text);
            }
        }
        for (k, r) in self.regions.iter().enumerate() {
            if self.parent[k] != ctx {
                continue;
            }
            let _ = writeln!(s, "        {} => {{", r.ts);
            let _ = writeln!(s, "        let r = ngu_host::try_catch(|| -> ngu_rt::Flow<{}> {{", self.ret_rs);
            let _ = writeln!(s, "        macro_rules! ret {{ () => {{ return ngu_rt::Flow::Ret(()) }}; ($e:expr) => {{ return ngu_rt::Flow::Ret($e) }}; }}");
            s.push_str(&self.gen_ctx(Some(k)));
            let _ = writeln!(s, "        }});");
            let _ = writeln!(s, "        match r {{");
            match &r.fin {
                Some((hs, _, is_fault)) => {
                    if *is_fault {
                        let _ = writeln!(s, "            Ok(ngu_rt::Flow::Goto(t)) => {{ pc = t; continue; }}");
                        let _ = writeln!(s, "            Ok(ngu_rt::Flow::Ret(v)) => {{ ret!(v); }}");
                    } else {
                        let _ = writeln!(s, "            Ok(ngu_rt::Flow::Goto(t)) => {{ pend_{k} = ngu_rt::Pend::Goto(t); pc = {hs}; continue; }}", k = k, hs = hs);
                        let _ = writeln!(s, "            Ok(ngu_rt::Flow::Ret(v)) => {{ pend_{k} = ngu_rt::Pend::Ret(v); pc = {hs}; continue; }}", k = k, hs = hs);
                    }
                    let _ = writeln!(s, "            Err(p) => {{ pend_{k} = ngu_rt::Pend::Throw(p); pc = {hs}; continue; }}", k = k, hs = hs);
                }
                None => {
                    let _ = writeln!(s, "            Ok(ngu_rt::Flow::Goto(t)) => {{ pc = t; continue; }}");
                    let _ = writeln!(s, "            Ok(ngu_rt::Flow::Ret(v)) => {{ ret!(v); }}");
                    let _ = writeln!(s, "            Err(p) => {{ let exc = ngu_host::to_exc(&p);");
                    for (hs, _, cid) in &r.catches {
                        let cond = match cid { Some(c) => format!("ngu_host::exc_is(&exc, {})", self.cexp(*c)), None => "true".to_string() };
                        let _ = writeln!(s, "                if {} {{ x0_Obj = ngu_host::exc_obj(&exc); ngu_host::set_cur(exc); pc = {}; continue; }}", cond, hs);
                    }
                    let _ = writeln!(s, "                std::panic::resume_unwind(p); }}");
                }
            }
            let _ = writeln!(s, "        }}");
            let _ = writeln!(s, "        }}");
        }
        if ctx.is_some() {
            let _ = writeln!(s, "        _ => return ngu_rt::Flow::Goto(pc),");
        } else {
            let _ = writeln!(s, "        _ => unreachable!(),");
        }
        let _ = writeln!(s, "    }} }}");
        s
    }
}

fn tn_is_str(t: &RT) -> bool {
    *t == RT::Str
}

fn op_name(op: &Op) -> String {
    match op {
        Op::Ldflda(_) => "ldflda".into(),
        Op::Box(_) => "box".into(),
        Op::Unbox(_) => "unbox".into(),
        Op::Castclass(_) => "castclass".into(),
        Op::Isinst(_) => "isinst".into(),
        Op::NewObj { .. } => "newobj".into(),
        Op::NewObjExt { .. } => "newobj_ext".into(),
        Op::CallVirtUser(_) => "callvirt_user".into(),
        Op::DelegateInvoke { .. } => "delegue".into(),
        Op::Ldelema => "ldelema".into(),
        Op::LdsfldExt(_) => "ldsfld_ext".into(),
        Op::Throw => "throw".into(),
        Op::CallMissing { key, .. } => format!("manquant:{}", key),
        Op::Unsupported(s) => format!("unsupported:{}", s),
        Op::ConvOvf(..) => "conv_ovf".into(),
        Op::Rethrow => "rethrow".into(),
        Op::Ldtoken(n) => format!("ldtoken:{}", n),
        Op::Starg(_) | Op::Ldarga(_) => "arg".into(),
        Op::ConvRUn => "conv_r_un".into(),
        Op::Stsfld(..) => "stsfld".into(),
        Op::Leave(_) => "leave".into(),
        Op::Endfinally => "endfinally".into(),
        Op::CallStructCtor { .. } => "struct_ctor_natif".into(),
        Op::NewObjMissing { key, .. } => format!("ctor_manquant:{}", key),
        Op::Ldsflda(..) => "ldsflda".into(),
        Op::Ldind | Op::Stind(_) | Op::Ldobj | Op::Stobj | Op::Initobj(_) => "adresse".into(),
        Op::NewDelegate | Op::Ldftn(_) | Op::Ldvirtftn(_) => "delegue".into(),
        _ => "autre".into(),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage: ngu-ilgen <Assembly-CSharp.dll> <dossier-sortie> <Classe1,Classe2|ALL>");
        std::process::exit(2);
    }
    let mut vm = Vm::new(&a[1]);
    let out_dir = &a[2];
    let want: Vec<&str> = a[3].split(',').collect();
    let all = want == ["ALL"];
    // a selected class brings its nested compiler-generated classes (lambdas, closures: `Class/<>c`, `Class/<>c__DisplayClass0_0`)
    let selected = |name: &str| want.iter().any(|w| name == *w || name.strip_prefix(w).is_some_and(|r| r.starts_with(['/', '+', '.'])));
    // C# can place string-switch helpers in this top-level compiler-generated type,
    // outside the requested class. Include it so selected methods can call it.
    let tids: Vec<u32> = (0..vm.n_user).filter(|t| {
        let name = vm.types[*t as usize].name.as_str();
        all || selected(name) || name == "<PrivateImplementationDetails>"
    }).collect();
    let mut g = G { vm: &mut vm, ext_needed: BTreeMap::new(), translated: BTreeSet::new(), coverage: vec![], ok: 0, fail: 0 };
    let mut code = String::from("// GENERATED by ngu-ilgen from the user's Assembly-CSharp.dll. Do not version.\nuse ngu_rt::*;\nuse ngu_host::{ToV, FromV};\nuse crate::ext;\n\n");
    for t in 0..g.vm.n_user {
        g.gen_types(t, &mut code);
    }
    for &t in &tids {
        let ids: Vec<u32> = g.vm.prog.methods.iter().enumerate().filter(|(_, m)| m.owner == t && m.mi.is_some()).map(|(i, _)| i as u32).collect();
        for m in ids {
            let text = g.gen_method(m);
            code.push_str(&text);
        }
    }
    g.gen_registry(&mut code);
    // missing BCL stubs
    let rt_ext = std::fs::read_to_string("crates/ngu-rt/src/ext.rs").unwrap_or_default() + &std::fs::read_to_string("crates/ngu-rt/src/unity.rs").unwrap_or_default();
    let mut stubs = String::from("// missing BCL stubs (to implement in ngu-rt)\nuse ngu_rt::*;\nuse crate::gen::*;\nuse ngu_host::{ToV, FromV};\n");
    for (name, (ps, r, gen_recv, key, targs, inst)) in &g.ext_needed {
        if rt_ext.contains(&format!("pub fn {}(", name)) || rt_ext.contains(&format!("pub fn {}<", name)) {
            continue;
        }
        let params: Vec<String> = ps.iter().enumerate().map(|(i, t)| if (i == 0 && *gen_recv) || t == "Obj" { format!("_a{}: T{}", i, i) } else { format!("_a{}: {}", i, t) }).collect();
        let ret = if r == "()" { String::new() } else { format!(" -> {}", r) };
        let gl: Vec<String> = ps.iter().enumerate().filter(|(i, t)| (*i == 0 && *gen_recv) || *t == "Obj").map(|(i, _)| format!("T{}: ToV", i)).collect();
        let gp = if gl.is_empty() { String::new() } else { format!("<{}>", gl.join(", ")) };
        let has_ptr = ps.iter().any(|t| t.starts_with("*mut"));
        let deep = key.contains("::Serialize");
        let argv: Vec<String> = (0..ps.len()).map(|i| if deep { format!("ngu_host::deep(&_a{})", i) } else { format!("_a{}.to_v()", i) }).collect();
        let body = if name.ends_with("___ctor") && r == "()" && key.starts_with("UnityEngine.MonoBehaviour") {
            "{}".to_string()
        } else if key.ends_with("::.ctor") && targs == "None" {
            // call of the native base constructor on `this` (Object, MonoBehaviour...): nothing to do
            "{}".to_string()
        } else if key.ends_with("::.ctor") {
            let tn = &key[..key.len() - 7];
            format!("{{ FromV::from_v(&ngu_host::native_new({:?}, {}, vec![{}])) }}", tn, targs, argv.join(", "))
        } else if has_ptr {
            format!("{{ unimplemented!(\"{}\") }}", name)
        } else if r == "()" {
            format!("{{ ngu_host::{}({:?}, {}, vec![{}]); }}", if *inst { "native_i" } else { "native" }, key, targs, argv.join(", "))
        } else {
            format!("{{ FromV::from_v(&ngu_host::{}({:?}, {}, vec![{}])) }}", if *inst { "native_i" } else { "native" }, key, targs, argv.join(", "))
        };
        let _ = writeln!(stubs, "pub fn {}{}({}){} {}", name, gp, params.join(", "), ret, body);
    }
    std::fs::create_dir_all(out_dir).unwrap();
    std::fs::write(format!("{}/gen.rs", out_dir), code).unwrap();
    std::fs::write(format!("{}/ext_stubs.rs", out_dir), stubs).unwrap();
    std::fs::write(format!("{}/coverage.txt", out_dir), g.coverage.join("\n")).unwrap();
    if let Some(e) = a.get(4) {
        let (t, m) = e.split_once("::").expect("Type::methode");
        let tid = *g.vm.type_ids.get(t).expect("type inconnu");
        let mid = g.vm.types[tid as usize].methods.get(m).and_then(|v| v.first().copied()).expect("methode inconnue");
        let (ps, ret) = g.method_sig(mid).expect("signature");
        let mut ent = String::from("use crate::gen::*;
use ngu_rt::*;
pub fn entry(a: &[String]) -> String {
");
        let mut args = vec![];
        for (i, p) in ps.iter().enumerate() {
            args.push(match p {
                RT::R8 => format!("a[{}].parse::<f64>().unwrap()", i),
                RT::R4 => format!("a[{}].parse::<f32>().unwrap()", i),
                RT::I4 => format!("a[{}].parse::<i32>().unwrap()", i),
                RT::I8 => format!("a[{}].parse::<i64>().unwrap()", i),
                RT::Str => format!("str_of(&a[{}])", i),
                _ => panic!("unhandled argument type for entry"),
            });
        }
        let call = format!("{}({})", g.fname(mid), args.join(", "));
        let fmt = match ret {
            RT::Str => format!("str_ref(&{}).to_string()", call),
            RT::R8 => format!("ngu_rt::fmt::fmt_float({}, 15)", call),
            RT::Bool => format!("(if {} {{ \"True\" }} else {{ \"False\" }}).to_string()", call),
            RT::Void => format!("{{ {}; String::new() }}", call),
            _ => format!("{}.to_string()", call),
        };
        let _ = writeln!(ent, "    {}
}}", fmt);
        std::fs::write(format!("{}/entry.rs", out_dir), ent).unwrap();
    }
    println!("types: {}, methodes traduites: {}, non traduites: {} (voir coverage.txt)", tids.len(), g.ok, g.fail);
    let _ = EhKind::Finally;
}
