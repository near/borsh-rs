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
fn slice_schema_container() {
    let schema = schema_container_of::<[i64]>();

    assert_eq!(
        schema,
        BorshSchemaContainer::new(
            "Vec<i64>".to_string(),
            map! {
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
            map! {
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
