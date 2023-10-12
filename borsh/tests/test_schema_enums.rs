#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // Local structures do not have their fields used.
#![cfg(feature = "unstable__schema")]

use core::fmt::{Debug, Display};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
};

use borsh::schema::*;
use borsh::{try_from_slice_with_schema, try_to_vec_with_schema};

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
pub fn simple_enum() {
    #[derive(borsh::BorshSchema)]
    enum A {
        Bacon,
        Eggs,
    }
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
        "ABacon" => Definition::Struct{ fields: Fields::Empty },
        "AEggs" => Definition::Struct{ fields: Fields::Empty },
            "A" => Definition::Enum {
                tag_width: 1,
                variants: vec![(0, "Bacon".to_string(), "ABacon".to_string()), (1, "Eggs".to_string(), "AEggs".to_string())]
            }
        },
        defs
    );
}

#[test]
pub fn single_field_enum() {
    #[derive(borsh::BorshSchema)]
    enum A {
        Bacon,
    }
    assert_eq!("A".to_string(), A::declaration());
    let mut defs = Default::default();
    A::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
            "ABacon" => Definition::Struct {fields: Fields::Empty},
            "A" => Definition::Enum {
                tag_width: 1,
                variants: vec![(0, "Bacon".to_string(), "ABacon".to_string())]
            }
        },
        defs
    );
}

/// test: Sausage wasn't populated with param Sausage<W>
#[derive(borsh::BorshSchema, Debug)]
enum AWithSkip<C, W> {
    Bacon,
    Eggs,
    Salad(u32, C, u32),
    Sausage {
        #[borsh(skip)]
        wrapper: W,
        filling: u32,
    },
}

/// test: inner structs in BorshSchema derive don't need any bounds, unrelated to BorshSchema
// #[derive(borsh::BorshSchema)]
// struct SideLeft<A>(
//     A,
// )
// where
//     A: Display + Debug,
//     B: Display + Debug;
#[derive(borsh::BorshSchema)]
enum Side<A, B>
where
    A: Display + Debug,
    B: Display + Debug,
{
    Left(A),
    Right(B),
}

#[test]
pub fn complex_enum_with_schema() {
    #[derive(
        borsh::BorshSchema,
        Default,
        borsh::BorshSerialize,
        borsh::BorshDeserialize,
        PartialEq,
        Debug,
    )]
    struct Tomatoes;
    #[derive(
        borsh::BorshSchema,
        Default,
        borsh::BorshSerialize,
        borsh::BorshDeserialize,
        PartialEq,
        Debug,
    )]
    struct Cucumber;
    #[derive(
        borsh::BorshSchema,
        Default,
        borsh::BorshSerialize,
        borsh::BorshDeserialize,
        PartialEq,
        Debug,
    )]
    struct Oil;
    #[derive(
        borsh::BorshSchema,
        Default,
        borsh::BorshSerialize,
        borsh::BorshDeserialize,
        PartialEq,
        Debug,
    )]
    struct Wrapper;
    #[derive(
        borsh::BorshSchema,
        Default,
        borsh::BorshSerialize,
        borsh::BorshDeserialize,
        PartialEq,
        Debug,
    )]
    struct Filling;
    #[derive(
        borsh::BorshSchema, borsh::BorshSerialize, borsh::BorshDeserialize, PartialEq, Debug,
    )]
    enum A {
        Bacon,
        Eggs,
        Salad(Tomatoes, Cucumber, Oil),
        Sausage { wrapper: Wrapper, filling: Filling },
    }

    impl Default for A {
        fn default() -> Self {
            A::Sausage {
                wrapper: Default::default(),
                filling: Default::default(),
            }
        }
    }
    // First check schema.
    assert_eq!("A".to_string(), A::declaration());
    let mut defs = Default::default();
    A::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "Cucumber" => Definition::Struct {fields: Fields::Empty},
        "ASalad" => Definition::Struct{ fields: Fields::UnnamedFields(vec!["Tomatoes".to_string(), "Cucumber".to_string(), "Oil".to_string()])},
        "ABacon" => Definition::Struct {fields: Fields::Empty},
        "Oil" => Definition::Struct {fields: Fields::Empty},
            "A" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "Bacon".to_string(), "ABacon".to_string()),
                    (1, "Eggs".to_string(), "AEggs".to_string()),
                    (2, "Salad".to_string(), "ASalad".to_string()),
                    (3, "Sausage".to_string(), "ASausage".to_string())
                ]
            },
        "Wrapper" => Definition::Struct {fields: Fields::Empty},
        "Tomatoes" => Definition::Struct {fields: Fields::Empty},
        "ASausage" => Definition::Struct { fields: Fields::NamedFields(vec![
        ("wrapper".to_string(), "Wrapper".to_string()),
        ("filling".to_string(), "Filling".to_string())
        ])},
        "AEggs" => Definition::Struct {fields: Fields::Empty},
        "Filling" => Definition::Struct {fields: Fields::Empty}
        },
        defs
    );
    // Then check that we serialize and deserialize with schema.
    let obj = A::default();
    let data = try_to_vec_with_schema(&obj).unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let obj2: A = try_from_slice_with_schema(&data).unwrap();
    assert_eq!(obj, obj2);
}

#[test]
pub fn complex_enum_generics() {
    #[derive(borsh::BorshSchema)]
    struct Tomatoes;
    #[derive(borsh::BorshSchema)]
    struct Cucumber;
    #[derive(borsh::BorshSchema)]
    struct Oil;
    #[derive(borsh::BorshSchema)]
    struct Wrapper;
    #[derive(borsh::BorshSchema)]
    struct Filling;
    #[derive(borsh::BorshSchema)]
    enum A<C, W> {
        Bacon,
        Eggs,
        Salad(Tomatoes, C, Oil),
        Sausage { wrapper: W, filling: Filling },
    }
    assert_eq!(
        "A<Cucumber, Wrapper>".to_string(),
        <A<Cucumber, Wrapper>>::declaration()
    );
    let mut defs = Default::default();
    <A<Cucumber, Wrapper>>::add_definitions_recursively(&mut defs);
    assert_eq!(
        map! {
        "Cucumber" => Definition::Struct {fields: Fields::Empty},
        "ASalad<Cucumber>" => Definition::Struct{
            fields: Fields::UnnamedFields(vec!["Tomatoes".to_string(), "Cucumber".to_string(), "Oil".to_string()])
        },
        "ABacon" => Definition::Struct {fields: Fields::Empty},
        "Oil" => Definition::Struct {fields: Fields::Empty},
        "A<Cucumber, Wrapper>" => Definition::Enum {
            tag_width: 1,
            variants: vec![
                (0, "Bacon".to_string(), "ABacon".to_string()),
                (1, "Eggs".to_string(), "AEggs".to_string()),
                (2, "Salad".to_string(), "ASalad<Cucumber>".to_string()),
                (3, "Sausage".to_string(), "ASausage<Wrapper>".to_string())
            ]
        },
        "Wrapper" => Definition::Struct {fields: Fields::Empty},
        "Tomatoes" => Definition::Struct {fields: Fields::Empty},
        "ASausage<Wrapper>" => Definition::Struct {
            fields: Fields::NamedFields(vec![
            ("wrapper".to_string(), "Wrapper".to_string()),
            ("filling".to_string(), "Filling".to_string())
            ])
        },
        "AEggs" => Definition::Struct {fields: Fields::Empty},
        "Filling" => Definition::Struct {fields: Fields::Empty}
        },
        defs
    );
}

fn common_map() -> BTreeMap<String, Definition> {
    map! {
        "EnumParametrized<String, u32, i8, u16>" => Definition::Enum {
            tag_width: 1,
            variants: vec![
                (0, "B".to_string(), "EnumParametrizedB<u32, i8, u16>".to_string()),
                (1, "C".to_string(), "EnumParametrizedC<String>".to_string())
            ]
        },
        "EnumParametrizedB<u32, i8, u16>" => Definition::Struct { fields: Fields::NamedFields(vec![
            ("x".to_string(), "BTreeMap<u32, u16>".to_string()),
            ("y".to_string(), "String".to_string()),
            ("z".to_string(), "i8".to_string())
        ])},
        "EnumParametrizedC<String>" => Definition::Struct{ fields: Fields::UnnamedFields(vec!["String".to_string(), "u16".to_string()])},
        "BTreeMap<u32, u16>" => Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: "(u32, u16)".to_string(),
        },
        "(u32, u16)" => Definition::Tuple { elements: vec!["u32".to_string(), "u16".to_string()]},
        "u32" => Definition::Primitive(4),
        "i8" => Definition::Primitive(1),
        "u16" => Definition::Primitive(2),
        "String" => Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: "u8".to_string()
        },
        "u8" => Definition::Primitive(1)
    }
}

#[test]
pub fn generic_associated_item1() {
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
    enum EnumParametrized<T, K, V>
    where
        K: TraitName,
        K: core::cmp::Ord,
        V: core::cmp::Ord,
    {
        B {
            x: BTreeMap<K, V>,
            y: String,
            z: K::Associated,
        },
        C(T, u16),
    }

    assert_eq!(
        "EnumParametrized<String, u32, i8, u16>".to_string(),
        <EnumParametrized<String, u32, u16>>::declaration()
    );

    let mut defs = Default::default();
    <EnumParametrized<String, u32, u16>>::add_definitions_recursively(&mut defs);
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
    enum EnumParametrized<T, K, V>
    where
        K: TraitName,
        K: core::cmp::Ord,
        V: core::cmp::Ord,
    {
        B {
            x: BTreeMap<K, V>,
            y: String,
            #[borsh(schema(params = "K => <K as TraitName>::Associated"))]
            z: <K as TraitName>::Associated,
        },
        C(T, u16),
    }

    assert_eq!(
        "EnumParametrized<String, u32, i8, u16>".to_string(),
        <EnumParametrized<String, u32, u16>>::declaration()
    );

    let mut defs = Default::default();
    <EnumParametrized<String, u32, u16>>::add_definitions_recursively(&mut defs);

    assert_eq!(common_map(), defs);
}
