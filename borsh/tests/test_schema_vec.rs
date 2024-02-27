#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(hash_collections)]
#![cfg(feature = "unstable__schema")]

#[macro_use]
mod common_macro;
use common_macro::schema_imports::*;

#[test]
fn slice_schema_container() {
    let schema = schema_container_of::<[i64]>();

    assert_eq!(
        schema,
        BorshSchemaContainer::new(
            "Vec<i64>".to_string(),
            schema_map! {
                "Vec<i64>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "i64".to_string(),
                },
                "i64" => Definition::Primitive(8)

            }
        )
    )
}

#[test]
fn vec_schema_container() {
    let schema = schema_container_of::<Vec<i64>>();

    assert_eq!(
        schema,
        BorshSchemaContainer::new(
            "Vec<i64>".to_string(),
            schema_map! {
                "Vec<i64>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "i64".to_string(),
                },
                "i64" => Definition::Primitive(8)

            }
        )
    )
}
