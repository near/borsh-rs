#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "unstable__schema")]

#[cfg(feature = "std")]
use std::collections::BTreeMap;

use borsh::schema::*;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::ToString, vec};

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
fn test_unary_tuple_schema() {
    assert_eq!("(bool,)", <(bool,)>::declaration());
    let mut defs = Default::default();
    <(bool,)>::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "(bool,)" => Definition::Tuple { elements: vec!["bool".to_string()] },
        "bool" => Definition::Primitive(1)
        },
        defs
    );
}
