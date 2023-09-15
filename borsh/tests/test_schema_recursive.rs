#![cfg(feature = "unstable__schema")]

#[cfg(feature = "std")]
use std::collections::BTreeMap;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::ToString, vec};

use borsh::schema::*;

#[allow(unused)]
#[derive(borsh::BorshSchema)]
struct CRecC {
    a: String,
    b: BTreeMap<String, CRecC>,
}

#[allow(unused)]
#[derive(borsh::BorshSchema)]
enum ERecD {
    B { x: String, y: i32 },
    C(u8, Vec<ERecD>),
}

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
pub fn recursive_struct_schema() {
    let mut defs = Default::default();
    CRecC::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
           "CRecC" => Definition::Struct {
                fields: Fields::NamedFields(
                    vec![
                        (
                            "a".to_string(),
                            "string".to_string(),
                        ),
                        (
                            "b".to_string(),
                            "BTreeMap<string, CRecC>".to_string(),
                        ),
                    ]

                )

            },
            "BTreeMap<string, CRecC>" => Definition::Sequence {
                elements: "Tuple<string, CRecC>".to_string(),

            },
            "Tuple<string, CRecC>" => Definition::Tuple {elements: vec!["string".to_string(), "CRecC".to_string()]}
        },
        defs
    );
}

#[test]
pub fn recursive_enum_schema() {
    let mut defs = Default::default();
    ERecD::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
           "ERecD" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    ("B".to_string(), "ERecDB".to_string()),
                    ("C".to_string(), "ERecDC".to_string()),
                ]
            },
            "ERecDB" => Definition::Struct {
                fields: Fields::NamedFields (
                    vec![
                        ("x".to_string(), "string".to_string()),
                        ("y".to_string(), "i32".to_string()),
                    ]
                )
            },
            "ERecDC" => Definition::Struct {
                fields: Fields::UnnamedFields( vec![
                    "u8".to_string(),
                    "Vec<ERecD>".to_string(),
                ])
            },
            "Vec<ERecD>" => Definition::Sequence {
                elements: "ERecD".to_string(),

            },
            "i32" => Definition::Primitive(4),
            "u8" => Definition::Primitive(1)
        },
        defs
    );
}
