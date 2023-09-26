#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(hash_collections)]
#![allow(dead_code)] // Local structures do not have their fields used.
#![cfg(feature = "unstable__schema")]

use borsh::schema::*;
#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::collections::{BTreeMap, HashMap};

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
};

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

// Checks that recursive definitions work. Also checks that re-instantiations of templated types work.
#[cfg(hash_collections)]
#[test]
pub fn duplicated_instantiations() {
    #[derive(borsh::BorshSchema)]
    struct Tomatoes;
    #[derive(borsh::BorshSchema)]
    struct Cucumber;
    #[derive(borsh::BorshSchema)]
    struct Oil<K, V> {
        seeds: HashMap<K, V>,
        liquid: Option<K>,
    }
    #[derive(borsh::BorshSchema)]
    struct Wrapper<T> {
        foo: Option<T>,
        bar: Box<A<T, T>>,
    }
    #[derive(borsh::BorshSchema)]
    struct Filling;
    #[derive(borsh::BorshSchema)]
    enum A<C, W> {
        Bacon,
        Eggs,
        Salad(Tomatoes, C, Oil<u64, String>),
        Sausage { wrapper: W, filling: Filling },
    }
    assert_eq!(
        "A<Cucumber, Wrapper<String>>".to_string(),
        <A<Cucumber, Wrapper<String>>>::declaration()
    );
    let mut defs = Default::default();
    <A<Cucumber, Wrapper<String>>>::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
            "A<Cucumber, Wrapper<String>>" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "Bacon".to_string(), "ABacon".to_string()),
                    (1, "Eggs".to_string(), "AEggs".to_string()),
                    (2, "Salad".to_string(), "ASalad<Cucumber>".to_string()),
                    (3, "Sausage".to_string(), "ASausage<Wrapper<String>>".to_string())
                ]
            },
            "A<String, String>" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "Bacon".to_string(), "ABacon".to_string()),
                    (1, "Eggs".to_string(), "AEggs".to_string()),
                    (2, "Salad".to_string(), "ASalad<String>".to_string()),
                    (3, "Sausage".to_string(), "ASausage<String>".to_string())
                ]
            },
        "ABacon" => Definition::Struct {fields: Fields::Empty},
        "AEggs" => Definition::Struct {fields: Fields::Empty},
        "ASalad<Cucumber>" => Definition::Struct {fields: Fields::UnnamedFields(vec!["Tomatoes".to_string(), "Cucumber".to_string(), "Oil<u64, String>".to_string()])},
        "ASalad<String>" => Definition::Struct { fields: Fields::UnnamedFields( vec!["Tomatoes".to_string(), "String".to_string(), "Oil<u64, String>".to_string() ])},
        "ASausage<Wrapper<String>>" => Definition::Struct {fields: Fields::NamedFields(vec![("wrapper".to_string(), "Wrapper<String>".to_string()), ("filling".to_string(), "Filling".to_string())])},
        "ASausage<String>" => Definition::Struct{ fields: Fields::NamedFields(vec![("wrapper".to_string(), "String".to_string()), ("filling".to_string(), "Filling".to_string())])},
        "Cucumber" => Definition::Struct {fields: Fields::Empty},
        "Filling" => Definition::Struct {fields: Fields::Empty},
            "HashMap<u64, String>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "(u64, String)".to_string(),
            },
        "Oil<u64, String>" => Definition::Struct { fields: Fields::NamedFields(vec![("seeds".to_string(), "HashMap<u64, String>".to_string()), ("liquid".to_string(), "Option<u64>".to_string())])},
            "Option<String>" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "None".to_string(), "()".to_string()),
                    (1, "Some".to_string(), "String".to_string())
                ]
            },
            "Option<u64>" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "None".to_string(), "()".to_string()),
                    (1, "Some".to_string(), "u64".to_string())
                ]
            },
        "Tomatoes" => Definition::Struct {fields: Fields::Empty},
        "(u64, String)" => Definition::Tuple {elements: vec!["u64".to_string(), "String".to_string()]},
        "Wrapper<String>" => Definition::Struct{ fields: Fields::NamedFields(vec![("foo".to_string(), "Option<String>".to_string()), ("bar".to_string(), "A<String, String>".to_string())])},
        "u64" => Definition::Primitive(8),
        "()" => Definition::Primitive(0),
        "String" => Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: "u8".to_string()
        },
        "u8" => Definition::Primitive(1)
        },
        defs
    );
}
