#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "rc")]

#[cfg(feature = "std")]
pub use std::{rc, sync};

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
pub use alloc::{rc, sync};

use borsh::{from_slice, BorshSerialize};

#[test]
fn test_rc_roundtrip() {
    let value = rc::Rc::new(8u8);
    let serialized = value.try_to_vec().unwrap();
    let deserialized = from_slice::<rc::Rc<u8>>(&serialized).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_slice_rc() {
    let original: &[i32] = &[1, 2, 3, 4, 6, 9, 10];
    let shared: rc::Rc<[i32]> = rc::Rc::from(original);
    let serialized = shared.try_to_vec().unwrap();
    let deserialized = from_slice::<rc::Rc<[i32]>>(&serialized).unwrap();
    assert_eq!(original, &*deserialized);
}

#[test]
fn test_arc_roundtrip() {
    let value = sync::Arc::new(8u8);
    let serialized = value.try_to_vec().unwrap();
    let deserialized = from_slice::<sync::Arc<u8>>(&serialized).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_slice_arc() {
    let original: &[i32] = &[1, 2, 3, 4, 6, 9, 10];
    let shared: sync::Arc<[i32]> = sync::Arc::from(original);
    let serialized = shared.try_to_vec().unwrap();
    let deserialized = from_slice::<sync::Arc<[i32]>>(&serialized).unwrap();
    assert_eq!(original, &*deserialized);
}
