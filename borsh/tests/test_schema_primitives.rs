#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(hash_collections)]
#![cfg(feature = "unstable__schema")]

#[cfg(feature = "std")]
use std::collections::BTreeMap;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::ToString};

use borsh::{schema::*, schema_container_of};

macro_rules! map(
    () => { BTreeMap::new() };
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = BTreeMap::new();
            $(
                m.insert($key.to_string(), $value);
            )+
            m
        }
     };
);

#[test]
fn isize_schema() {
    let schema = schema_container_of::<isize>();

    assert_eq!(
        schema,
        BorshSchemaContainer::new(
            "i64".to_string(),
            map! {
                "i64" => Definition::Primitive(8)

            }
        )
    )
}

#[test]
fn usize_schema() {
    let schema = schema_container_of::<usize>();

    assert_eq!(
        schema,
        BorshSchemaContainer::new(
            "u64".to_string(),
            map! {
                "u64" => Definition::Primitive(8)

            }
        )
    )
}
