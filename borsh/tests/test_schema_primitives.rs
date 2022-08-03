use borsh::schema::*;
use std::collections::HashMap;

#[test]
fn isize_schema() {
    let schema = isize::schema_container();
    assert_eq!(
        schema,
        BorshSchemaContainer {
            declaration: "i64".to_string(),
            definitions: HashMap::new()
        }
    )
}

#[test]
fn usize_schema() {
    let schema = usize::schema_container();
    assert_eq!(
        schema,
        BorshSchemaContainer {
            declaration: "u64".to_string(),
            definitions: HashMap::new()
        }
    )
}
