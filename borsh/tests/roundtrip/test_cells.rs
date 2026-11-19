use alloc::string::{String, ToString};

use crate::common_macro::assert_bounded_parity;

#[test]
fn test_cell_roundtrip() {
    let cell = core::cell::Cell::new(42u32);

    let out = borsh::to_vec(&cell).unwrap();

    let cell_round: core::cell::Cell<u32> = borsh::from_slice(&out).unwrap();
    assert_bounded_parity::<core::cell::Cell<u32>>(&out);

    assert_eq!(cell, cell_round);
}

#[test]
fn test_ref_cell_roundtrip() {
    let rcell = core::cell::RefCell::new("str".to_string());

    let out = borsh::to_vec(&rcell).unwrap();

    let rcell_round: core::cell::RefCell<String> = borsh::from_slice(&out).unwrap();
    assert_bounded_parity::<core::cell::RefCell<String>>(&out);

    assert_eq!(rcell, rcell_round);
}
