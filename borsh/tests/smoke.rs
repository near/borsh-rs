#![cfg_attr(not(feature = "std"), no_std)]
// Smoke tests that ensure that we don't accidentally remove top-level
// re-exports in a minor release.

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec;

#[allow(unused_imports)]
use borsh::de::EnumExtBounded as _;
use borsh::{self, from_reader_bounded, from_slice, from_slice_bounded, BorshDeserializeBounded};
#[cfg(feature = "unstable__schema")]
use borsh::{schema_container_of, try_from_slice_with_schema};

#[cfg(feature = "unstable__schema")]
#[test]
fn test_to_vec() {
    let value = 42u8;

    let seriazeble = (schema_container_of::<u8>(), value);
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

#[test]
fn test_from_slice_bounded() {
    let serialized = borsh::to_vec(&42u64).unwrap();
    let deserialized = from_slice_bounded::<u64>(&serialized, 0).unwrap();
    assert_eq!(deserialized, 42u64);
    let deserialized = from_reader_bounded::<_, u64>(&mut serialized.as_slice(), 0).unwrap();
    assert_eq!(deserialized, 42u64);
    let mut budget = borsh::de::Budget::new(0);
    let deserialized =
        <u64 as BorshDeserializeBounded>::deserialize_bounded(&mut &serialized[..], &mut budget)
            .unwrap();
    assert_eq!(deserialized, 42u64);
    let _: Option<borsh::error::LimitExceeded> = None;
}
