use crate::common_macro::schema_imports::*;
use borsh::{from_slice, to_vec, try_from_slice_with_schema, try_to_vec_with_schema};
use uuid::Uuid;

#[test]
fn test_uuid() {
    assert_eq!("uuid::Uuid", Uuid::declaration());

    let want = schema_map! {
        "uuid::Uuid" => Definition::Sequence {
            length_width: Definition::ARRAY_LENGTH_WIDTH,
            length_range: 16..=16,
            elements: "u8".to_string(),
        },
        "u8" => Definition::Primitive(1)
    };

    let mut actual_defs = schema_map!();
    Uuid::add_definitions_recursively(&mut actual_defs);
    assert_eq!(want, actual_defs);
}

#[test]
fn test_uuid_schema_matches_encoding() {
    let id = Uuid::from_bytes([
        0xf9, 0x16, 0x8c, 0x5e, 0xce, 0xb2, 0x4f, 0xaa, 0xb6, 0xbf, 0x32, 0x9b, 0xf3, 0x9f, 0xa1,
        0xe4,
    ]);

    let schema = schema_container_of::<Uuid>();
    assert_eq!(Ok(()), schema.validate());
    // The schema has to describe exactly the bytes `BorshSerialize` writes.
    let encoded = to_vec(&id).unwrap();
    assert_eq!(16, encoded.len());
    assert_eq!(Ok(encoded.len()), schema.max_serialized_size());

    let with_schema = try_to_vec_with_schema(&id).unwrap();
    assert_eq!(id, try_from_slice_with_schema::<Uuid>(&with_schema).unwrap());
    assert_eq!(id, from_slice::<Uuid>(&encoded).unwrap());
}

#[test]
fn test_uuid_in_derived_struct() {
    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    struct Account {
        id: Uuid,
        parent: Option<Uuid>,
    }

    let schema = schema_container_of::<Account>();
    assert_eq!(Ok(()), schema.validate());
    // 16 bytes for `id`, plus a 1-byte tag and 16 bytes for `parent`.
    assert_eq!(Ok(33), schema.max_serialized_size());
    assert_eq!(
        Some(&Definition::Sequence {
            length_width: Definition::ARRAY_LENGTH_WIDTH,
            length_range: 16..=16,
            elements: "u8".to_string(),
        }),
        schema.get_definition("uuid::Uuid")
    );
}
