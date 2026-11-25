use super::*;

pub(super) fn lookup(key: &str) -> Option<NativeFn> {
    let function: NativeFn = match key {
        // ---------------------------------------------------------- numbers -> string
        "System.Int32::ToString" | "System.Int64::ToString" | "System.Single::ToString" | "System.Double::ToString"
        | "System.Boolean::ToString" | "System.Byte::ToString" | "System.UInt64::ToString" | "System.UInt32::ToString"
        | "System.Char::ToString" | "System.Int16::ToString" | "System.Enum::ToString"
        | "System.Decimal::ToString" => |vm, a| {
            let v = rv(vm, &a[0]);
            if let (Value::I32(c), true) = (&v, a.is_empty()) { let _ = c; }
            if let Value::I32(i) = &v {
                // bool -> True/False not distinguishable here: handled by the method type
                let _ = i;
            }
            tostring_fn(vm, a)
        },
        "System.Int32::Parse" => |vm, a| Ok(Value::I32(parse_integer::<i32>(vm, &a[0])?)),
        "System.Int64::Parse" => |vm, a| Ok(Value::I64(parse_integer::<i64>(vm, &a[0])?)),
        "System.Int16::Parse" => |vm, a| Ok(Value::I32(parse_integer::<i16>(vm, &a[0])? as i32)),
        "System.Byte::Parse" => |vm, a| Ok(Value::I32(parse_integer::<u8>(vm, &a[0])? as i32)),
        "System.UInt32::Parse" => |vm, a| Ok(Value::I32(parse_integer::<u32>(vm, &a[0])? as i32)),
        "System.UInt64::Parse" => |vm, a| Ok(Value::I64(parse_integer::<u64>(vm, &a[0])? as i64)),
        "System.Single::Parse" => |vm, a| parse_float(vm, &a[0], true),
        "System.Double::Parse" => |vm, a| parse_float(vm, &a[0], false),
        "System.Single::IsNaN" | "System.Double::IsNaN" => |_vm, a| Ok(Value::bool(a[0].as_f64().is_nan())),
        "System.Single::IsInfinity" | "System.Double::IsInfinity" => |_vm, a| Ok(Value::bool(a[0].as_f64().is_infinite())),
        "System.Double::IsPositiveInfinity" => |_vm, a| Ok(Value::bool(a[0].as_f64() == f64::INFINITY)),
        "System.Double::IsNegativeInfinity" => |_vm, a| Ok(Value::bool(a[0].as_f64() == f64::NEG_INFINITY)),
        "System.Convert::ToInt32" => |vm, a| Ok(Value::I32(match &a[0] {
            Value::Str(_) => parse_integer::<i32>(vm, &a[0])?,
            Value::F32(_) | Value::F64(_) => rounded_integer(vm, a[0].as_f64(), 2_147_483_648.0)? as i32,
            Value::I64(v) => match i32::try_from(*v) { Ok(x) => x, Err(_) => return integer_overflow(vm) },
            v => v.as_i32(),
        })),
        "System.Convert::ToInt64" => |vm, a| Ok(Value::I64(match &a[0] {
            Value::Str(_) => parse_integer::<i64>(vm, &a[0])?,
            Value::F32(_) | Value::F64(_) => rounded_integer(vm, a[0].as_f64(), 9_223_372_036_854_775_808.0)?,
            v => v.as_i64(),
        })),
        "System.Convert::ToDouble" => |vm, a| match &a[0] {
            Value::Str(_) => parse_float(vm, &a[0], false),
            v => Ok(Value::F64(v.as_f64())),
        },
        "System.Convert::ToBase64String" => |vm, a| {
            let b = bytes_of_value(vm, &a[0])?;
            Ok(Value::str(&b64_enc(&b)))
        },
        "System.Convert::FromBase64String" => |vm, a| {
            let b = b64_dec(&st(&a[0])).iter().map(|x| Value::I32(*x as i32)).collect();
            Ok(new_array(vm, b))
        },
        // The game only constructs Decimal from Int64. Keep that value exact through Floor.
        "System.Decimal::op_Implicit" => |vm, a| {
            let Value::I64(value) = a[0] else {
                return vm.throw("System.NotImplementedException", "Unsupported conversion to System.Decimal.");
            };
            let ty = vm.type_id("System.Decimal");
            Ok(Value::Struct(Box::new(Struct { ty, f: vec![Value::I64(value)] })))
        },
        "System.Decimal::op_Explicit" => |vm, a| {
            let value = rv(vm, &a[0]);
            if is_integral_decimal(vm, &value) {
                if let Value::Struct(s) = value {
                    return Ok(s.f[0].clone());
                }
            }
            vm.throw("System.NotImplementedException", "Unsupported conversion from System.Decimal.")
        },
        // ---------------------------------------------------------- Math
        "System.Math::Min" => |_vm, a| Ok(math_extreme(a, true, false)),
        "System.Math::Max" => |_vm, a| Ok(math_extreme(a, false, false)),
        "UnityEngine.Mathf::Min" => |vm, a| mathf_extreme(vm, a, true),
        "UnityEngine.Mathf::Max" => |vm, a| mathf_extreme(vm, a, false),
        "System.Math::Abs" | "UnityEngine.Mathf::Abs" => |vm, a| Ok(match &a[0] {
            Value::I32(x) => match x.checked_abs() { Some(v) => Value::I32(v), None => return integer_overflow(vm) },
            Value::I64(x) => match x.checked_abs() { Some(v) => Value::I64(v), None => return integer_overflow(vm) },
            Value::F32(x) => Value::F32(x.abs()),
            v => Value::F64(v.as_f64().abs()),
        }),
        "System.Math::Floor" => |vm, a| {
            let value = rv(vm, &a[0]);
            if is_integral_decimal(vm, &value) {
                return Ok(value);
            }
            Ok(Value::F64(value.as_f64().floor()))
        },
        "System.Math::Ceiling" => |_vm, a| Ok(Value::F64(a[0].as_f64().ceil())),
        "System.Math::Round" => |vm, a| {
            let value = a[0].as_f64();
            let Some(digits) = a.get(1).map(Value::as_i32) else {
                return Ok(Value::F64(value.round_ties_even()));
            };
            if !(0..=15).contains(&digits) {
                return vm.throw("System.ArgumentOutOfRangeException", "Rounding digits must be between 0 and 15.");
            }
            if value.abs() >= 1e16 || !value.is_finite() {
                return Ok(Value::F64(value));
            }
            let scale = 10_f64.powi(digits);
            Ok(Value::F64((value * scale).round_ties_even() / scale))
        },
        "System.Math::Sqrt" => |_vm, a| Ok(Value::F64(a[0].as_f64().sqrt())),
        "System.Math::Pow" => |_vm, a| Ok(Value::F64(mono_pow(a[0].as_f64(), a[1].as_f64()))),
        "System.Math::Log" => |_vm, a| Ok(Value::F64(if a.len() > 1 {
            mono_log(a[0].as_f64(), a[1].as_f64())
        } else {
            a[0].as_f64().ln()
        })),
        "System.Math::Log10" => |_vm, a| Ok(Value::F64(a[0].as_f64().log10())),
        "UnityEngine.Mathf::Floor" => |_vm, a| Ok(Value::F32(a[0].as_f32().floor())),
        "UnityEngine.Mathf::Ceil" => |_vm, a| Ok(Value::F32(a[0].as_f32().ceil())),
        "UnityEngine.Mathf::FloorToInt" => |_vm, a| Ok(Value::I32(mono_float_to_i32(f64::from(a[0].as_f32()).floor()))),
        "UnityEngine.Mathf::CeilToInt" => |_vm, a| Ok(Value::I32(mono_float_to_i32(a[0].as_f64().ceil()))),
        "UnityEngine.Mathf::Sqrt" => |_vm, a| Ok(Value::F32(a[0].as_f32().sqrt())),
        "UnityEngine.Mathf::Pow" => |_vm, a| Ok(Value::F32(mono_pow(a[0].as_f64(), a[1].as_f64()) as f32)),
        "UnityEngine.Mathf::Log10" => |_vm, a| Ok(Value::F32(a[0].as_f64().log10() as f32)),
        "UnityEngine.Mathf::Repeat" => |_vm, a| {
            let (t, l) = (a[0].as_f32(), a[1].as_f32());
            // Mathf.Floor receives a Single, but Mono retains the following intermediate product as a Double.
            let wrapped = f64::from(t) - f64::from((t / l).floor()) * f64::from(l);
            Ok(Value::F32(mathf_clamp(wrapped as f32, 0.0, l)))
        },
        "UnityEngine.Mathf::Clamp" => |_vm, a| Ok(match (&a[0], &a[1], &a[2]) {
            (Value::I32(x), Value::I32(lo), Value::I32(hi)) => Value::I32(mathf_clamp(*x, *lo, *hi)),
            _ => Value::F32(mathf_clamp(a[0].as_f32(), a[1].as_f32(), a[2].as_f32())),
        }),
        _ => return None,
    };
    Some(function)
}

fn mono_pow(a: f64, b: f64) -> f64 {
    // The original Mono runtime uses legacy exceptional-value semantics for Pow.
    if a.is_nan() || b.is_nan() || (a == -1.0 && b.is_infinite()) {
        return f64::NAN;
    }
    let base = if a == 0.0 || (a.is_infinite() && b < 0.0) { a.abs() } else { a };
    base.powf(b)
}
fn mono_log(a: f64, b: f64) -> f64 {
    if b == 1.0 || (a != 1.0 && (b == 0.0 || b == f64::INFINITY)) {
        return f64::NAN;
    }
    a.ln() / b.ln()
}
/// `conv.i4` from a float, like x86-64 (cvttsd2si): NaN / out of range -> `i32::MIN`.
fn mono_float_to_i32(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        x as i32
    }
}

#[cfg(test)]
mod tests {
    use super::{mono_float_to_i32, mono_log, mono_pow};

    #[test]
    fn mono_math_exceptional_values() {
        assert!(mono_pow(f64::NAN, 0.0).is_nan());
        assert!(mono_pow(-1.0, f64::INFINITY).is_nan());
        assert!(!mono_pow(-0.0, 3.0).is_sign_negative());
        assert!(!mono_pow(f64::NEG_INFINITY, -3.0).is_sign_negative());
        assert_eq!(mono_pow(2.0, 3.0), 8.0);
        assert!(mono_log(2.0, 1.0).is_nan());
        assert!(mono_log(2.0, 0.0).is_nan());
        assert!(mono_log(2.0, f64::INFINITY).is_nan());
        assert_eq!(mono_log(8.0, 2.0), 3.0);
    }

    #[test]
    fn mono_unchecked_float_conversion() {
        assert_eq!(mono_float_to_i32(2_147_483_647.9), i32::MAX);
        assert_eq!(mono_float_to_i32(2_147_483_648.0), i32::MIN);
        assert_eq!(mono_float_to_i32(-2_147_483_649.0), i32::MIN);
        assert_eq!(mono_float_to_i32(f64::NAN), i32::MIN);
        assert_eq!(mono_float_to_i32(f64::INFINITY), i32::MIN);
        assert_eq!(mono_float_to_i32(-3.9), -3);
    }
}
