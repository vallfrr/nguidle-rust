use save_format::{parse, N};

fn header() -> Vec<u8> {
    let mut bytes = vec![0];
    for number in [1_i32, -1, 1, 0] {
        bytes.extend_from_slice(&number.to_le_bytes());
    }
    bytes
}

#[test]
fn reads_a_forward_string_reference_and_null() {
    let mut bytes = header();
    bytes.extend([16, 1, 0, 0, 0, 2, 0, 0, 0]); // Object array, id 1, two elements.
    bytes.extend([9, 2, 0, 0, 0, 10]); // Reference to id 2 and a null.
    bytes.extend([6, 2, 0, 0, 0, 4]);
    bytes.extend(b"test");
    bytes.push(11);

    let stream = parse(&bytes).expect("synthetic array");
    let N::Arr(array) = stream.root else {
        panic!("expected an object array");
    };
    assert!(matches!(&array.items[..], [N::Ref(2), N::Null]));
    assert!(matches!(stream.index.get(&2), Some(N::Str(2, value)) if value.as_ref() == "test"));
}

#[test]
fn rejects_truncated_string_records() {
    let mut bytes = header();
    bytes.extend([6, 1, 0, 0, 0, 3, b'a', b'b', b'c', 11]);
    assert!(parse(&bytes).is_ok());
    for size in 0..bytes.len() {
        assert!(parse(&bytes[..size]).is_err(), "prefix of {size} bytes");
    }
}

#[test]
fn checks_null_runs_against_the_remaining_slots() {
    let mut bytes = header();
    bytes.extend([16, 1, 0, 0, 0, 2, 0, 0, 0, 10, 14, 1, 0, 0, 0, 11]);
    assert!(parse(&bytes).is_ok());

    let run_length = bytes.len() - 5;
    bytes[run_length] = 2;
    assert_eq!(
        parse(&bytes).err().as_deref(),
        Some("invalid null run length")
    );
}

#[test]
fn reads_library_records_without_nesting_the_parser() {
    let mut bytes = header();
    for id in 1_i32..=1024 {
        bytes.push(12);
        bytes.extend(id.to_le_bytes());
        bytes.extend([1, b'L']);
    }
    bytes.extend([10, 11]);
    let stream = parse(&bytes).expect("synthetic records");
    assert_eq!(stream.libs.len(), 1024);
}

#[test]
fn caps_nested_arrays_and_rejects_invalid_length_prefixes() {
    let mut nested = header();
    for id in 1_i32..=130 {
        nested.push(16);
        nested.extend(id.to_le_bytes());
        nested.extend(1_i32.to_le_bytes());
    }
    nested.extend([10, 11]);
    assert_eq!(
        parse(&nested).err().as_deref(),
        Some("save has too many nested records")
    );

    let mut invalid_string = header();
    invalid_string.extend([6, 1, 0, 0, 0, 0x80, 0x80, 0x80, 0x80, 0x08]);
    assert_eq!(
        parse(&invalid_string).err().as_deref(),
        Some("bad length prefix")
    );
}
