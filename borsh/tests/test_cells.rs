#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::string::{String, ToString};

#[test]
fn test_cell_roundtrip() {
    let cell = core::cell::Cell::new(42u32);

    let out = borsh::to_vec(&cell).unwrap();

    let cell_round: core::cell::Cell<u32> = borsh::from_slice(&out).unwrap();

    assert_eq!(cell, cell_round);
}

#[test]
fn test_ref_cell_roundtrip() {
    let rcell = core::cell::RefCell::new("str".to_string());

    let out = borsh::to_vec(&rcell).unwrap();

    let rcell_round: core::cell::RefCell<String> = borsh::from_slice(&out).unwrap();

    assert_eq!(rcell, rcell_round);
}

mod de_errors {

    use alloc::string::ToString;

    #[test]
    fn test_ref_cell_try_borrow_error() {
        let rcell = core::cell::RefCell::new("str");

        let _active_borrow = rcell.try_borrow_mut().unwrap();

        assert_eq!(
            borsh::to_vec(&rcell).unwrap_err().to_string(),
            "already mutably borrowed"
        );
    }
}

#[macro_use]
mod common_macro;

#[cfg(feature = "unstable__schema")]
mod schema {

    use super::common_macro::schema_imports::*;
    fn common_map_i32() -> BTreeMap<String, Definition> {
        schema_map! {

            "i32" => Definition::Primitive(4)
        }
    }

    fn common_map_slice_i32() -> BTreeMap<String, Definition> {
        schema_map! {
            "Vec<i32>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "i32".to_string()
            },
            "i32" => Definition::Primitive(4)
        }
    }

    #[test]
    fn test_cell() {
        assert_eq!("i32", <core::cell::Cell<i32> as BorshSchema>::declaration());

        let mut actual_defs = schema_map!();
        <core::cell::Cell<i32> as BorshSchema>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(common_map_i32(), actual_defs);
    }

    #[test]
    fn test_ref_cell_vec() {
        assert_eq!(
            "Vec<i32>",
            <core::cell::RefCell<Vec<i32>> as BorshSchema>::declaration()
        );

        let mut actual_defs = schema_map!();
        <core::cell::RefCell<Vec<i32>> as BorshSchema>::add_definitions_recursively(
            &mut actual_defs,
        );
        assert_eq!(common_map_slice_i32(), actual_defs);
    }
}
