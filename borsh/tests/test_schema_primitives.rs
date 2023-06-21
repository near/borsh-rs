#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(hash_collections)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::ToString;

use borsh::schema::*;

#[test]
fn isize_schema() {
    let schema = isize::schema_container();
    assert_eq!(
        schema,
        BorshSchemaContainer {
            declaration: "i64".to_string(),
            definitions: Default::default()
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
            definitions: Default::default()
        }
    )
}
