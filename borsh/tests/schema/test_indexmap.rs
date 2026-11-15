use crate::common_macro::schema_imports::*;

use indexmap::{IndexMap, IndexSet};

#[test]
fn simple_map() {
    let actual_name = IndexMap::<u64, String>::declaration();
    let mut actual_defs = schema_map!();
    IndexMap::<u64, String>::add_definitions_recursively(&mut actual_defs);
    assert_eq!("indexmap::IndexMap<u64, String>", actual_name);
    assert_eq!(
        schema_map! {
            "indexmap::IndexMap<u64, String>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "(u64, String)".to_string(),
            },
            "(u64, String)" => Definition::Tuple {
                elements: vec!["u64".to_string(), "String".to_string()],
            },
            "u64" => Definition::Primitive(8),
            "String" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "u8".to_string()
            },
            "u8" => Definition::Primitive(1)
        },
        actual_defs
    );
}

#[test]
fn simple_set() {
    let actual_name = IndexSet::<String>::declaration();
    let mut actual_defs = schema_map!();
    IndexSet::<String>::add_definitions_recursively(&mut actual_defs);
    assert_eq!("indexmap::IndexSet<String>", actual_name);
    assert_eq!(
        schema_map! {
            "indexmap::IndexSet<String>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "String".to_string(),
            },
            "String" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "u8".to_string()
            },
            "u8" => Definition::Primitive(1)
        },
        actual_defs
    );
}

#[test]
fn containers_validate() {
    assert_eq!(
        Ok(()),
        schema_container_of::<IndexMap<u64, String>>().validate()
    );
    assert_eq!(Ok(()), schema_container_of::<IndexSet<String>>().validate());
}
