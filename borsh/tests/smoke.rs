#![cfg_attr(not(feature = "std"), no_std)]
// Smoke tests that ensure that we don't accidentally remove top-level
// re-exports in a minor release.

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec;

use borsh::{self, from_slice};
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

#[cfg(feature = "unstable__bounded")]
#[test]
fn test_from_slice_bounded() {
    use borsh::{from_reader_bounded, from_slice_bounded, BorshDeserializeBounded, Budget};
    // Only checks that these paths exist.
    #[allow(unused_imports)]
    use borsh::{de::EnumExtBounded, error::LimitExceeded};

    let serialized = borsh::to_vec(&42u64).unwrap();
    // The entry points charge the `u64` itself; `deserialize_bounded_alloc` does not.
    let deserialized = from_slice_bounded::<u64>(&serialized, &mut Budget::new(8)).unwrap();
    assert_eq!(deserialized, 42u64);
    let deserialized =
        from_reader_bounded::<_, u64>(&mut serialized.as_slice(), &mut Budget::new(8)).unwrap();
    assert_eq!(deserialized, 42u64);
    let deserialized = <u64 as BorshDeserializeBounded>::deserialize_bounded_alloc(
        &mut &serialized[..],
        &mut Budget::new(0),
    )
    .unwrap();
    assert_eq!(deserialized, 42u64);
}
