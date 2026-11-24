#[cfg(feature = "legacy-vita-save")]
use clr::bcl_core::{b64_dec, b64_enc};
use clr::value::Value;
use clr::{serial, Vm};
use dotnetdll::prelude::*;

fn fixture_vm() -> Vm {
    let mut assembly = Resolution::new(Module::new("save-fixture.dll"));
    let player = assembly.push_type_definition(TypeDefinition::new(None, "PlayerData"));
    assembly.push_field(
        player,
        Field::instance(Accessibility::Public, "points", ctype! { int }),
    );
    let wrapper = assembly.push_type_definition(TypeDefinition::new(None, "SaveData"));
    for field in ["playerData", "checksum"] {
        assembly.push_field(
            wrapper,
            Field::instance(Accessibility::Public, field, ctype! { string }),
        );
    }
    Vm::from_bytes(
        assembly
            .write(WriteOptions::default())
            .expect("synthetic type metadata"),
    )
}

fn first_field(value: &Value) -> Value {
    let Value::Obj(object) = value else {
        panic!("expected an object");
    };
    object.f.borrow()[0].clone()
}

#[test]
fn round_trips_an_object_field() {
    let mut vm = fixture_vm();
    let id = vm.type_ids["PlayerData"];
    let object = vm.new_obj(id);
    object.f.borrow_mut()[0] = Value::I32(-42);
    let bytes = serial::serialize(&mut vm, &Value::Obj(object))
        .unwrap_or_else(|error| panic!("serialize fixture: {}", vm.describe(&error)));
    let decoded = serial::deserialize(&mut vm, &bytes)
        .unwrap_or_else(|error| panic!("deserialize fixture: {}", vm.describe(&error)));
    assert!(matches!(first_field(&decoded), Value::I32(-42)));
}

#[cfg(feature = "legacy-vita-save")]
fn legacy_object(name: &str, fields: &[(&str, Vec<u8>)]) -> Vec<u8> {
    fn sized(bytes: &mut Vec<u8>, value: &[u8]) {
        bytes.extend(u32::try_from(value.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(value);
    }

    let mut bytes = b"NGUS1".to_vec();
    bytes.push(10);
    sized(&mut bytes, name.as_bytes());
    bytes.extend(u32::try_from(fields.len()).unwrap().to_le_bytes());
    for (name, field) in fields {
        sized(&mut bytes, name.as_bytes());
        bytes.extend_from_slice(field);
    }
    bytes
}

#[cfg(feature = "legacy-vita-save")]
fn legacy_string(text: &str) -> Vec<u8> {
    let mut bytes = vec![6];
    bytes.extend(u32::try_from(text.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

#[test]
#[cfg(feature = "legacy-vita-save")]
fn legacy_conversion_checks_the_digest_and_preserves_the_payload() {
    use md5::{Digest, Md5};

    let mut vm = fixture_vm();
    let mut points = vec![1];
    points.extend(78_i32.to_le_bytes());
    let player = legacy_object("PlayerData", &[("points", points)]);
    let payload = legacy_string(&b64_enc(&player));
    let valid_digest = b64_enc(&Md5::digest(&player));

    let invalid = legacy_object(
        "SaveData",
        &[
            ("playerData", payload.clone()),
            ("checksum", legacy_string("bad digest")),
        ],
    );
    assert_eq!(
        clr::legacy_save::convert(&mut vm, &b64_enc(&invalid)).unwrap_err(),
        "Legacy save checksum does not match."
    );

    let valid = legacy_object(
        "SaveData",
        &[
            ("playerData", payload),
            ("checksum", legacy_string(&valid_digest)),
        ],
    );
    let converted =
        clr::legacy_save::convert(&mut vm, &b64_enc(&valid)).expect("legacy conversion");
    let wrapper = serial::deserialize(&mut vm, &b64_dec(&converted))
        .unwrap_or_else(|error| panic!("converted wrapper: {}", vm.describe(&error)));
    let field = first_field(&wrapper);
    let inner = b64_dec(field.as_str().expect("encoded player"));
    let restored = serial::deserialize(&mut vm, &inner)
        .unwrap_or_else(|error| panic!("converted player: {}", vm.describe(&error)));
    assert!(matches!(first_field(&restored), Value::I32(78)));
}
