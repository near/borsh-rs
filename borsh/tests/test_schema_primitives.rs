#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(hash_collections)]
#![cfg(feature = "schema")]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::ToString;

use borsh::{schema::*, schema_container_of};

#[test]
fn isize_schema() {
    let schema = schema_container_of::<isize>();

    assert_eq!(
        schema,
        BorshSchemaContainer::new("i64".to_string(), Default::default())
    )
}

#[test]
fn usize_schema() {
    let schema = schema_container_of::<usize>();

    assert_eq!(
        schema,
        BorshSchemaContainer::new("u64".to_string(), Default::default())
    )
}
