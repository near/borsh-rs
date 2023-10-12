#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "unstable__schema")]

use borsh::schema::*;

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;

use core::marker::PhantomData;
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

#[test]
pub fn unit_struct() {
    #[derive(borsh::BorshSchema)]
    struct A;

    // https://github.com/near/borsh-rs/issues/112
    #[allow(unused)]
    impl A {
        pub fn declaration() -> usize {
            42
        }
    }
    assert_eq!("A".to_string(), <A as borsh::BorshSchema>::declaration());
    let mut defs = Default::default();
    A::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "A" => Definition::Struct {fields: Fields::Empty}
        },
        defs
    );
}

#[test]
pub fn simple_struct() {
    #[derive(borsh::BorshSchema)]
    struct A {
        _f1: u64,
        _f2: String,
    }
    assert_eq!("A".to_string(), A::declaration());
    let mut defs = Default::default();
    A::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "A" => Definition::Struct{ fields: Fields::NamedFields(vec![
        ("_f1".to_string(), "u64".to_string()),
        ("_f2".to_string(), "String".to_string())
        ])},
        "u64" => Definition::Primitive(8),
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

#[test]
pub fn boxed() {
    #[derive(borsh::BorshSchema)]
    struct A {
        _f1: Box<u64>,
        _f2: Box<str>,
        _f3: Box<[u8]>,
    }
    assert_eq!("A".to_string(), A::declaration());
    let mut defs = Default::default();
    A::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
            "Vec<u8>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "u8".to_string(),
            },
        "A" => Definition::Struct{ fields: Fields::NamedFields(vec![
        ("_f1".to_string(), "u64".to_string()),
        ("_f2".to_string(), "String".to_string()),
        ("_f3".to_string(), "Vec<u8>".to_string())
        ])},
        "u64" => Definition::Primitive(8),
        "u8" => Definition::Primitive(1),
        "String" => Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: "u8".to_string()
        }
        },
        defs
    );
}

#[test]
pub fn wrapper_struct() {
    #[derive(borsh::BorshSchema)]
    struct A<T>(T);
    assert_eq!("A<u64>".to_string(), <A<u64>>::declaration());
    let mut defs = Default::default();
    <A<u64>>::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "A<u64>" => Definition::Struct {fields: Fields::UnnamedFields(vec!["u64".to_string()])},
        "u64" => Definition::Primitive(8)
        },
        defs
    );
}

#[test]
pub fn tuple_struct() {
    #[derive(borsh::BorshSchema)]
    struct A(u64, String);
    assert_eq!("A".to_string(), A::declaration());
    let mut defs = Default::default();
    A::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "A" => Definition::Struct {fields: Fields::UnnamedFields(vec![
         "u64".to_string(), "String".to_string()
        ])},
        "u64" => Definition::Primitive(8),
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

#[test]
pub fn tuple_struct_params() {
    #[derive(borsh::BorshSchema)]
    struct A<K, V>(K, V);
    assert_eq!(
        "A<u64, String>".to_string(),
        <A<u64, String>>::declaration()
    );
    let mut defs = Default::default();
    <A<u64, String>>::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "A<u64, String>" => Definition::Struct { fields: Fields::UnnamedFields(vec![
            "u64".to_string(), "String".to_string()
        ])},
        "u64" => Definition::Primitive(8),
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

#[cfg(hash_collections)]
#[test]
pub fn simple_generics() {
    #[derive(borsh::BorshSchema)]
    struct A<K, V> {
        _f1: HashMap<K, V>,
        _f2: String,
    }
    assert_eq!(
        "A<u64, String>".to_string(),
        <A<u64, String>>::declaration()
    );
    let mut defs = Default::default();
    <A<u64, String>>::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "A<u64, String>" => Definition::Struct {
        fields: Fields::NamedFields(vec![
        ("_f1".to_string(), "HashMap<u64, String>".to_string()),
        ("_f2".to_string(), "String".to_string())
        ])
        },
            "HashMap<u64, String>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "(u64, String)".to_string(),
            },
        "(u64, String)" => Definition::Tuple{elements: vec!["u64".to_string(), "String".to_string()]},
        "u64" => Definition::Primitive(8),
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

fn common_map() -> BTreeMap<String, Definition> {
    map! {

        "Parametrized<String, i8>" => Definition::Struct {
            fields: Fields::NamedFields(vec![
                ("field".to_string(), "i8".to_string()),
                ("another".to_string(), "String".to_string())
            ])
        },
        "i8" => Definition::Primitive(1),
        "String" => Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: "u8".to_string()
        },
        "u8" => Definition::Primitive(1)
    }
}

#[test]
pub fn generic_associated_item() {
    trait TraitName {
        type Associated;
        fn method(&self);
    }

    impl TraitName for u32 {
        type Associated = i8;
        fn method(&self) {}
    }

    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    struct Parametrized<V, T>
    where
        T: TraitName,
    {
        field: T::Associated,
        another: V,
    }

    assert_eq!(
        "Parametrized<String, i8>".to_string(),
        <Parametrized<String, u32>>::declaration()
    );

    let mut defs = Default::default();
    <Parametrized<String, u32>>::add_definitions_recursively(&mut defs);
    assert_eq!(common_map(), defs);
}

#[test]
pub fn generic_associated_item2() {
    trait TraitName {
        type Associated;
        fn method(&self);
    }

    impl TraitName for u32 {
        type Associated = i8;
        fn method(&self) {}
    }

    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    struct Parametrized<V, T>
    where
        T: TraitName,
    {
        #[borsh(schema(params = "T => <T as TraitName>::Associated"))]
        field: <T as TraitName>::Associated,
        another: V,
    }

    assert_eq!(
        "Parametrized<String, i8>".to_string(),
        <Parametrized<String, u32>>::declaration()
    );

    let mut defs = Default::default();
    <Parametrized<String, u32>>::add_definitions_recursively(&mut defs);
    assert_eq!(common_map(), defs);
}

#[test]
pub fn generic_associated_item3() {
    trait TraitName {
        type Associated;
        fn method(&self);
    }

    impl TraitName for u32 {
        type Associated = i8;
        fn method(&self) {}
    }

    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    struct Parametrized<V, T>
    where
        T: TraitName,
    {
        #[borsh(schema(params = "T => T, T => <T as TraitName>::Associated"))]
        field: (<T as TraitName>::Associated, T),
        another: V,
    }

    assert_eq!(
        "Parametrized<String, u32, i8>".to_string(),
        <Parametrized<String, u32>>::declaration()
    );

    let mut defs = Default::default();
    <Parametrized<String, u32>>::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
            "Parametrized<String, u32, i8>" => Definition::Struct {
                fields: Fields::NamedFields(vec![
                    ("field".to_string(), "(i8, u32)".to_string()),
                    ("another".to_string(), "String".to_string())
                ])
            },
            "(i8, u32)" => Definition::Tuple {
                elements: vec!["i8".to_string(), "u32".to_string()]
            },
            "i8" => Definition::Primitive(1),
            "u32" => Definition::Primitive(4),
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

#[test]
pub fn with_phantom_data() {
    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    struct Parametrized<K, V> {
        field: K,
        another: PhantomData<V>,
    }

    assert_eq!(
        "Parametrized<String>".to_string(),
        <Parametrized<String, u32>>::declaration()
    );

    let mut defs = Default::default();
    <Parametrized<String, u32>>::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
            "Parametrized<String>" => Definition::Struct {
                fields: Fields::NamedFields(vec![
                    ("field".to_string(), "String".to_string()),
                    ("another".to_string(), "()".to_string())
                ])
            },
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
