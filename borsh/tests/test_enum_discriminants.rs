#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "derive")]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec;

use borsh::{from_slice, to_vec, BorshDeserialize, BorshSerialize};

// sequence, no unit enums
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
#[repr(u16)]
enum XY {
    A,
    B = 20,
    C,
    D(u32, u32),
    E = 10,
    F(u64),
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = false)]
#[repr(u16)]
enum XYNoDiscriminant {
    A,
    B = 20,
    C,
    D(u32, u32),
    E = 10,
    F(u64),
}

#[test]
fn test_discriminant_serde_no_unit_type() {
    let values = vec![XY::A, XY::B, XY::C, XY::E, XY::D(12, 14), XY::F(35325423)];
    let expected_discriminants = [0u8, 20, 21, 10, 22, 11];

    for (ind, value) in values.iter().enumerate() {
        let data = to_vec(value).unwrap();
        assert_eq!(data[0], expected_discriminants[ind]);
        assert_eq!(from_slice::<XY>(&data).unwrap(), values[ind]);
    }
}

#[test]
fn test_discriminant_serde_no_unit_type_no_use_discriminant() {
    let values = vec![
        XYNoDiscriminant::A,
        XYNoDiscriminant::B,
        XYNoDiscriminant::C,
        XYNoDiscriminant::D(12, 14),
        XYNoDiscriminant::E,
        XYNoDiscriminant::F(35325423),
    ];
    let expected_discriminants = [0u8, 1, 2, 3, 4, 5];

    for (ind, value) in values.iter().enumerate() {
        let data = to_vec(value).unwrap();
        assert_eq!(data[0], expected_discriminants[ind]);
        assert_eq!(from_slice::<XYNoDiscriminant>(&data).unwrap(), values[ind]);
    }
}

// minimal
#[derive(BorshSerialize)]
#[borsh(use_discriminant = true)]
enum MyDiscriminantEnum {
    A = 20,
}

#[derive(BorshSerialize)]
#[borsh(use_discriminant = false)]
enum MyDiscriminantEnumFalse {
    A = 20,
}

#[derive(BorshSerialize)]
enum MyEnumNoDiscriminant {
    A,
}
#[test]
fn test_discriminant_minimal_true() {
    assert_eq!(MyDiscriminantEnum::A as u8, 20);
    assert_eq!(to_vec(&MyDiscriminantEnum::A).unwrap(), vec![20]);
}

#[test]
fn test_discriminant_minimal_false() {
    assert_eq!(MyDiscriminantEnumFalse::A as u8, 20);
    assert_eq!(
        to_vec(&MyEnumNoDiscriminant::A).unwrap(),
        to_vec(&MyDiscriminantEnumFalse::A).unwrap(),
    );
    assert_eq!(to_vec(&MyDiscriminantEnumFalse::A).unwrap(), vec![0]);
}

// sequence
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = false)]
enum XNoDiscriminant {
    A,
    B = 20,
    C,
    D,
    E = 10,
    F,
}

#[test]
fn test_discriminant_serde_no_use_discriminant() {
    let values = vec![
        XNoDiscriminant::A,
        XNoDiscriminant::B,
        XNoDiscriminant::C,
        XNoDiscriminant::D,
        XNoDiscriminant::E,
        XNoDiscriminant::F,
    ];
    let expected_discriminants = [0u8, 1, 2, 3, 4, 5];
    for (index, value) in values.iter().enumerate() {
        let data = to_vec(value).unwrap();
        assert_eq!(data[0], expected_discriminants[index]);
        assert_eq!(from_slice::<XNoDiscriminant>(&data).unwrap(), values[index]);
    }
}
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
struct D {
    x: u64,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
enum C {
    C1,
    C2(u64),
    C3(u64, u64),
    C4 { x: u64, y: u64 },
    C5(D),
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
enum X {
    A,
    B = 20,
    C,
    D,
    E = 10,
    F,
}

#[test]
fn test_discriminant_serialization() {
    let values = vec![X::A, X::B, X::C, X::D, X::E, X::F];
    for value in values {
        assert_eq!(to_vec(&value).unwrap(), [value as u8]);
    }
}

#[test]
fn test_discriminant_deserialization() {
    let values = vec![X::A, X::B, X::C, X::D, X::E, X::F];
    for value in values {
        assert_eq!(from_slice::<X>(&[value as u8]).unwrap(), value,);
    }
}

#[test]
#[should_panic = "Unexpected variant tag: 2"]
fn test_deserialize_invalid_discriminant() {
    from_slice::<X>(&[2]).unwrap();
}

#[test]
fn test_discriminant_serde() {
    let values = vec![X::A, X::B, X::C, X::D, X::E, X::F];
    let expected_discriminants = [0u8, 20, 21, 22, 10, 11];
    for (index, value) in values.iter().enumerate() {
        let data = to_vec(value).unwrap();
        assert_eq!(data[0], expected_discriminants[index]);
        assert_eq!(from_slice::<X>(&data).unwrap(), values[index]);
    }
}

#[cfg(feature = "unstable__schema")]
mod schema {
    #[cfg(not(feature = "std"))]
    use alloc::{collections::BTreeMap, string::ToString, vec};

    #[cfg(feature = "std")]
    use std::collections::BTreeMap;

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

    use borsh::{
        schema::{Definition, Fields},
        BorshSchema,
    };

    #[allow(unused)]
    #[derive(BorshSchema)]
    #[borsh(use_discriminant = true)]
    #[repr(i16)]
    enum XY {
        A,
        B = 20,
        C,
        D(u32, u32),
        E = 10,
        F(u64),
    }

    #[test]
    fn test_schema_discriminant_no_unit_type() {
        assert_eq!("XY".to_string(), XY::declaration());
        let mut defs = Default::default();
        XY::add_definitions_recursively(&mut defs);
        assert_eq!(
            map! {
                "XY" => Definition::Enum {
                    tag_width: 1,
                    variants: vec![
                         (0, "A".to_string(), "XYA".to_string()),
                         (20, "B".to_string(), "XYB".to_string()),
                         (21, "C".to_string(), "XYC".to_string()),
                         (22, "D".to_string(), "XYD".to_string()),
                         (10, "E".to_string(), "XYE".to_string()),
                         (11, "F".to_string(), "XYF".to_string())
                    ]
                },
                "XYA" => Definition::Struct{ fields: Fields::Empty },
                "XYB" => Definition::Struct{ fields: Fields::Empty },
                "XYC" => Definition::Struct{ fields: Fields::Empty },
                "XYD" => Definition::Struct{ fields: Fields::UnnamedFields(
                    vec!["u32".to_string(), "u32".to_string()]
                )},
                "XYE" => Definition::Struct{ fields: Fields::Empty },
                "XYF" => Definition::Struct{ fields: Fields::UnnamedFields(
                    vec!["u64".to_string()]

                )},
                "u32" => Definition::Primitive(4),
                "u64" => Definition::Primitive(8)
            },
            defs
        );
    }

    #[allow(unused)]
    #[derive(BorshSchema)]
    #[borsh(use_discriminant = false)]
    #[repr(i16)]
    enum XYNoDiscriminant {
        A,
        B = 20,
        C,
        D(u32, u32),
        E = 10,
        F(u64),
    }

    #[test]
    fn test_schema_discriminant_no_unit_type_no_use_discriminant() {
        assert_eq!(
            "XYNoDiscriminant".to_string(),
            XYNoDiscriminant::declaration()
        );
        let mut defs = Default::default();
        XYNoDiscriminant::add_definitions_recursively(&mut defs);
        assert_eq!(
            map! {
                "XYNoDiscriminant" => Definition::Enum {
                    tag_width: 1,
                    variants: vec![
                         (0, "A".to_string(), "XYNoDiscriminantA".to_string()),
                         (1, "B".to_string(), "XYNoDiscriminantB".to_string()),
                         (2, "C".to_string(), "XYNoDiscriminantC".to_string()),
                         (3, "D".to_string(), "XYNoDiscriminantD".to_string()),
                         (4, "E".to_string(), "XYNoDiscriminantE".to_string()),
                         (5, "F".to_string(), "XYNoDiscriminantF".to_string())
                    ]
                },
                "XYNoDiscriminantA" => Definition::Struct{ fields: Fields::Empty },
                "XYNoDiscriminantB" => Definition::Struct{ fields: Fields::Empty },
                "XYNoDiscriminantC" => Definition::Struct{ fields: Fields::Empty },
                "XYNoDiscriminantD" => Definition::Struct{ fields: Fields::UnnamedFields(
                    vec!["u32".to_string(), "u32".to_string()]
                )},
                "XYNoDiscriminantE" => Definition::Struct{ fields: Fields::Empty },
                "XYNoDiscriminantF" => Definition::Struct{ fields: Fields::UnnamedFields(
                    vec!["u64".to_string()]

                )},
                "u32" => Definition::Primitive(4),
                "u64" => Definition::Primitive(8)
            },
            defs
        );
    }
}
