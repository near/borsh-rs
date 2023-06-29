#![cfg_attr(not(feature = "std"), no_std)]
// Smoke tests that ensure that we don't accidentally remove top-level
// re-exports in a minor release.

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec;

use borsh::{self, from_slice};
#[cfg(feature = "schema")]
use borsh::{try_from_slice_with_schema, BorshSchema};

#[cfg(feature = "schema")]
#[test]
fn test_to_vec() {
    let value = 42u8;
    let seriazeble = (<u8 as BorshSchema>::schema_container(), value);
    let serialized = borsh::to_vec(&seriazeble).unwrap();
    #[cfg(feature = "std")]
    println!("serialized: {:?}", serialized);
    let deserialized = try_from_slice_with_schema::<u8>(&serialized).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_to_writer() {
    let value = 42u8;
    let mut serialized = vec![0; 1];
    // serialized: [2, 0, 0, 0, 117, 56, 0, 0, 0, 0, 42]
    borsh::to_writer(&mut serialized[..], &value).unwrap();
    let deserialized = from_slice::<u8>(&serialized).unwrap();
    assert_eq!(value, deserialized);
}
