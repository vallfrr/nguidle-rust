//! Native BCL/Unity layer: native types, structs, intrinsics.
use crate::code::NativeFn;
use crate::program::Ty;
use crate::value::*;
use crate::Vm;
use std::rc::Rc;

pub fn is_native_struct(n: &str) -> bool {
    matches!(
        n,
        "UnityEngine.Vector2" | "UnityEngine.Vector3" | "UnityEngine.Vector4" | "UnityEngine.Quaternion"
            | "UnityEngine.Color" | "UnityEngine.Color32" | "UnityEngine.Rect" | "UnityEngine.Matrix4x4"
            | "System.DateTime" | "System.TimeSpan" | "System.Collections.Generic.KeyValuePair`2"
            | "System.Nullable`1" | "System.Decimal" | "UnityEngine.Resolution" | "System.Collections.Generic.List`1/Enumerator"
            | "System.Collections.Generic.Dictionary`2/Enumerator" | "UnityEngine.EventSystems.RaycastResult"
            | "System.Guid" | "System.IntPtr" | "UnityEngine.Bounds" | "UnityEngine.Random/State"
    )
}

pub fn is_native_enum(n: &str) -> bool {
    matches!(
        n,
        "UnityEngine.RuntimePlatform" | "UnityEngine.KeyCode" | "UnityEngine.TextAnchor" | "UnityEngine.FontStyle"
            | "UnityEngine.EventSystems.PointerEventData/InputButton" | "UnityEngine.FilterMode" | "UnityEngine.TextureFormat"
    )
}

pub fn is_delegate_type(n: &str) -> bool {
    n.starts_with("System.Action") || n.starts_with("System.Func`") || n == "UnityEngine.Events.UnityAction"
        || n.starts_with("UnityEngine.Events.UnityAction`") || n == "System.Predicate`1" || n == "System.Comparison`1"
        || n == "System.EventHandler" || n == "System.Delegate" || n == "System.MulticastDelegate"
        || n == "UnityEngine.Application/LogCallback"
}

fn ty_from(c: char) -> Ty {
    match c {
        'f' => Ty::F32,
        'd' => Ty::F64,
        'i' => Ty::I32,
        'l' => Ty::I64,
        _ => Ty::Object,
    }
}

/// Instance fields of a native type: (names, types).
pub fn native_layout(n: &str) -> (Vec<&'static str>, Vec<Ty>) {
    let (names, kinds): (&[&str], &str) = match n {
        "UnityEngine.Vector2" => (&["x", "y"], "ff"),
        "UnityEngine.Vector3" => (&["x", "y", "z"], "fff"),
        "UnityEngine.Vector4" | "UnityEngine.Quaternion" => (&["x", "y", "z", "w"], "ffff"),
        "UnityEngine.Color" => (&["r", "g", "b", "a"], "ffff"),
        "UnityEngine.Color32" => (&["r", "g", "b", "a"], "iiii"),
        "UnityEngine.Rect" => (&["m_XMin", "m_YMin", "m_Width", "m_Height"], "ffff"),
        "UnityEngine.Resolution" => (&["m_Width", "m_Height", "m_RefreshRate"], "iii"),
        "UnityEngine.Random/State" => (&["s0", "s1", "s2", "s3"], "iiii"),
        "System.DateTime" | "System.TimeSpan" => (&["ticks"], "l"),
        "System.Collections.Generic.KeyValuePair`2" => (&["key", "value"], "oo"),
        "System.Nullable`1" => (&["hasValue", "value"], "io"),
        "System.Exception" => (&["_message", "_inner"], "oo"),
        _ => (&[], ""),
    };
    (names.to_vec(), kinds.chars().map(ty_from).collect())
}

pub fn native_parent(n: &str) -> Option<&'static str> {
    Some(match n {
        "System.SystemException" | "System.ApplicationException" => "System.Exception",
        "System.NullReferenceException" | "System.InvalidCastException" | "System.IndexOutOfRangeException"
        | "System.InvalidOperationException" | "System.ArgumentException" | "System.NotImplementedException"
        | "System.ArithmeticException" | "System.FormatException" | "System.StackOverflowException"
        | "System.InvalidProgramException" | "System.NotSupportedException" | "System.IO.IOException"
        | "System.OutOfMemoryException" => "System.SystemException",
        "System.DivideByZeroException" | "System.OverflowException" => "System.ArithmeticException",
        "System.ArgumentNullException" | "System.ArgumentOutOfRangeException" => "System.ArgumentException",
        "System.Collections.Generic.KeyNotFoundException" => "System.SystemException",
        "System.IO.FileNotFoundException" | "System.IO.DirectoryNotFoundException" => "System.IO.IOException",
        "System.Exception" => "System.Object",
        "UnityEngine.MonoBehaviour" => "UnityEngine.Behaviour",
        "UnityEngine.Behaviour" => "UnityEngine.Component",
        "UnityEngine.Component" => "UnityEngine.Object",
        "UnityEngine.GameObject" => "UnityEngine.Object",
        "UnityEngine.UI.Button" | "UnityEngine.UI.Slider" | "UnityEngine.UI.Toggle" | "UnityEngine.UI.Dropdown"
        | "UnityEngine.UI.InputField" | "UnityEngine.UI.Scrollbar" => "UnityEngine.UI.Selectable",
        "UnityEngine.UI.Selectable" => "UnityEngine.EventSystems.UIBehaviour",
        "UnityEngine.EventSystems.UIBehaviour" => "UnityEngine.MonoBehaviour",
        "UnityEngine.UI.Image" | "UnityEngine.UI.Text" | "UnityEngine.UI.RawImage" => "UnityEngine.UI.MaskableGraphic",
        "UnityEngine.UI.MaskableGraphic" => "UnityEngine.UI.Graphic",
        "UnityEngine.UI.Graphic" => "UnityEngine.EventSystems.UIBehaviour",
        "UnityEngine.RectTransform" => "UnityEngine.Transform",
        "UnityEngine.Transform" => "UnityEngine.Component",
        _ => return None,
    })
}

pub fn native_implements(a: &str, b: &str) -> bool {
    matches!(
        (a, b),
        ("System.Collections.Generic.List`1", "System.Collections.Generic.IList`1")
            | ("System.Collections.Generic.List`1", "System.Collections.IEnumerable")
            | ("System.Collections.Generic.List`1", "System.Collections.Generic.IEnumerable`1")
            | ("System.Array", "System.Collections.IEnumerable")
            | ("System.Array", "System.Collections.Generic.IEnumerable`1")
            | ("System.Array", "System.Collections.IList")
    )
}

pub fn native_field_slot(tname: &str, field: &str) -> Option<u32> {
    let (names, _) = native_layout(tname);
    names.iter().position(|n| *n == field).map(|p| p as u32)
}

pub fn static_field(vm: &mut Vm, key: &str) -> R<Value> {
    Ok(match key {
        "System.String::Empty" => Value::str(""),
        "System.Int32::MaxValue" => Value::I32(i32::MAX),
        "System.Int32::MinValue" => Value::I32(i32::MIN),
        "System.Single::MaxValue" => Value::F32(f32::MAX),
        "System.Single::MinValue" => Value::F32(f32::MIN),
        "System.Double::MaxValue" => Value::F64(f64::MAX),
        "System.Double::MinValue" => Value::F64(f64::MIN),
        "System.Int64::MaxValue" => Value::I64(i64::MAX),
        "System.Int64::MinValue" => Value::I64(i64::MIN),
        "System.Math::PI" => Value::F64(std::f64::consts::PI),
        "System.IntPtr::Zero" => Value::I64(0),
        "System.DateTime::MinValue" => Value::Struct(Box::new(Struct { ty: vm.type_id("System.DateTime"), f: vec![Value::I64(0)] })),
        _ => {
            return vm.throw("System.NotImplementedException", &format!("native static field {}", key));
        }
    })
}

fn nop(_: &mut Vm, _: &mut Vec<Value>) -> R<Value> {
    Ok(Value::Null)
}

/// Intrinsics registry: key "Type::Method" -> function.
pub fn lookup(key: &str) -> Option<NativeFn> {
    let f: NativeFn = match key {
        "System.Object::.ctor" | "UnityEngine.MonoBehaviour::.ctor" | "System.ValueType::.ctor"
        | "UnityEngine.Behaviour::.ctor" | "UnityEngine.Component::.ctor" | "UnityEngine.Object::.ctor"
        | "System.Enum::.ctor" | "System.MulticastDelegate::.ctor" | "System.Attribute::.ctor" => nop,
        _ => return crate::bcl_core::lookup(key),
    };
    Some(f)
}

pub fn rc(s: &str) -> Value {
    Value::Str(Rc::from(s))
}
