use crate::common_macro::schema_imports::*;

#[test]
fn test_bytes() {
    assert_eq!("bytes::Bytes", bytes::Bytes::declaration());
    assert_eq!("bytes::BytesMut", bytes::BytesMut::declaration());

    let want_bytes = schema_map! {
        "bytes::Bytes" => Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: "u8".to_string(),
        },
        "u8" => Definition::Primitive(1)
    };

    let mut actual_defs = schema_map!();
    bytes::Bytes::add_definitions_recursively(&mut actual_defs);
    assert_eq!(want_bytes, actual_defs);

    let want_bytes_mut = schema_map! {
        "bytes::BytesMut" => Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: "u8".to_string(),
        },
        "u8" => Definition::Primitive(1)
    };
    let mut actual_defs = schema_map!();
    bytes::BytesMut::add_definitions_recursively(&mut actual_defs);
    assert_eq!(want_bytes_mut, actual_defs);
}
