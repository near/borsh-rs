#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::float_cmp)]
#![cfg(feature = "derive")]

use borsh::{from_slice, BorshDeserialize, BorshSerialize};
use bson::oid::ObjectId;

#[derive(BorshDeserialize, BorshSerialize, PartialEq, Debug)]
struct StructWithObjectId(i32, ObjectId, u8);

#[test]
fn test_object_id() {
    let obj = StructWithObjectId(
        123,
        ObjectId::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
        33,
    );
    let serialized = obj.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(serialized);
    let deserialized: StructWithObjectId = from_slice(&serialized).unwrap();
    assert_eq!(obj, deserialized);
}
