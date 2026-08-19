use crate::common_macro::schema_imports::*;

#[allow(unused)]
#[derive(borsh::BorshSchema)]
enum ERecD {
    B { x: String, y: i32 },
    C(u8, Vec<ERecD>),
}

#[test]
pub fn recursive_enum_schema() {
    let mut defs = Default::default();
    ERecD::add_definitions_recursively(&mut defs);
    assert_eq!(
        schema_map! {
           "ERecD" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "B".to_string(), "ERecD__B".to_string()),
                    (1, "C".to_string(), "ERecD__C".to_string()),
                ]
            },
            "ERecD__B" => Definition::Struct {
                fields: Fields::NamedFields (
                    vec![
                        ("x".to_string(), "String".to_string()),
                        ("y".to_string(), "i32".to_string()),
                    ]
                )
            },
            "ERecD__C" => Definition::Struct {
                fields: Fields::UnnamedFields( vec![
                    "u8".to_string(),
                    "Vec<ERecD>".to_string(),
                ])
            },
            "Vec<ERecD>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "ERecD".to_string(),
            },
            "i32" => Definition::Primitive(4),
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

// Regression test for https://github.com/near/borsh-rs/issues/377:
// a bare `Self` in an enum variant field must resolve to the enum, not to the
// synthesized per-variant inner struct, so the schema matches the wire format.
#[allow(unused)]
#[derive(borsh::BorshSchema)]
enum ERecDSelf {
    B { x: String, y: i32 },
    C(u8, Vec<Self>),
}

#[test]
pub fn recursive_enum_self_schema() {
    let mut defs = Default::default();
    ERecDSelf::add_definitions_recursively(&mut defs);
    assert_eq!(
        schema_map! {
           "ERecDSelf" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "B".to_string(), "ERecDSelf__B".to_string()),
                    (1, "C".to_string(), "ERecDSelf__C".to_string()),
                ]
            },
            "ERecDSelf__B" => Definition::Struct {
                fields: Fields::NamedFields (
                    vec![
                        ("x".to_string(), "String".to_string()),
                        ("y".to_string(), "i32".to_string()),
                    ]
                )
            },
            // The second field and the sequence element resolve to the enum
            // `ERecDSelf`, not the inner struct `ERecDSelf__C`.
            "ERecDSelf__C" => Definition::Struct {
                fields: Fields::UnnamedFields( vec![
                    "u8".to_string(),
                    "Vec<ERecDSelf>".to_string(),
                ])
            },
            "Vec<ERecDSelf>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "ERecDSelf".to_string(),
            },
            "i32" => Definition::Primitive(4),
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

// The `Vec<Self>` schema must be byte-identical to the spelled-out `Vec<ERecDSelf>`
// schema. Both enums are named `ERecDSelf` (in separate scopes) so the declarations,
// and therefore the whole definition maps, coincide exactly when the fix is in place.
#[test]
pub fn recursive_enum_self_matches_spelled_out() {
    let self_defs = {
        let mut defs = Default::default();
        ERecDSelf::add_definitions_recursively(&mut defs);
        defs
    };
    let spelled_out_defs = {
        #[allow(unused)]
        #[derive(borsh::BorshSchema)]
        enum ERecDSelf {
            B { x: String, y: i32 },
            C(u8, Vec<ERecDSelf>),
        }
        let mut defs = Default::default();
        ERecDSelf::add_definitions_recursively(&mut defs);
        defs
    };
    assert_eq!(self_defs, spelled_out_defs);
}

// Generic recursion: `Vec<Self>` inside `List<T>` must resolve to `List<T>` and keep
// `T` used, so the inner struct stays generic and the schema matches `Vec<List<T>>`.
#[test]
pub fn recursive_generic_enum_self_schema() {
    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    enum List<T> {
        Nil,
        Cons(T, Vec<Self>),
    }

    let mut defs = Default::default();
    <List<u32>>::add_definitions_recursively(&mut defs);
    assert_eq!(
        schema_map! {
            "List<u32>" => Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "Nil".to_string(), "List__Nil".to_string()),
                    (1, "Cons".to_string(), "List__Cons<u32>".to_string()),
                ]
            },
            "List__Nil" => Definition::Struct { fields: Fields::Empty },
            "List__Cons<u32>" => Definition::Struct {
                fields: Fields::UnnamedFields(vec![
                    "u32".to_string(),
                    "Vec<List<u32>>".to_string(),
                ])
            },
            "Vec<List<u32>>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "List<u32>".to_string(),
            },
            "u32" => Definition::Primitive(4)
        },
        defs
    );
}
