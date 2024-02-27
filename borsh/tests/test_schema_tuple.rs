#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "unstable__schema")]

#[macro_use]
mod common_macro;
use common_macro::schema_imports::*;

#[test]
fn test_unary_tuple_schema() {
    assert_eq!("(bool,)", <(bool,)>::declaration());
    let mut defs = Default::default();
    <(bool,)>::add_definitions_recursively(&mut defs);
    assert_eq!(
        schema_map! {
        "(bool,)" => Definition::Tuple { elements: vec!["bool".to_string()] },
        "bool" => Definition::Primitive(1)
        },
        defs
    );
}
