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
