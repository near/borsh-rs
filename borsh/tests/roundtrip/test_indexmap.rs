use alloc::{collections::BTreeMap, vec::Vec};
#[cfg(feature = "de_strict_order")]
use alloc::string::ToString;
use borsh::{from_slice, BorshDeserialize};
use indexmap::{IndexMap, IndexSet};

#[test]
// Taken from https://github.com/indexmap-rs/indexmap/blob/dd06e5773e4f91748396c67d00c83637f5c0dd49/src/borsh.rs#L100
// license: MIT OR Apache-2.0
fn test_indexmap_roundtrip() {
    let original_map: IndexMap<i32, i32> = {
        let mut map = IndexMap::new();
        map.insert(1, 2);
        map.insert(3, 4);
        map.insert(5, 6);
        map
    };
    let serialized_map = borsh::to_vec(&original_map).unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(serialized_map);

    let deserialized_map: IndexMap<i32, i32> =
        BorshDeserialize::try_from_slice(&serialized_map).unwrap();
    assert_eq!(original_map, deserialized_map);
}

#[test]
// Taken from https://github.com/indexmap-rs/indexmap/blob/dd06e5773e4f91748396c67d00c83637f5c0dd49/src/borsh.rs#L115
// license: MIT OR Apache-2.0
fn test_indexset_roundtrip() {
    let mut original_set = IndexSet::new();
    [1, 2, 3, 4, 5, 6].iter().for_each(|&i| {
        original_set.insert(i);
    });

    let serialized_set = borsh::to_vec(&original_set).unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(serialized_set);

    let deserialized_set: IndexSet<i32> =
        BorshDeserialize::try_from_slice(&serialized_set).unwrap();
    assert_eq!(original_set, deserialized_set);
}

// length 2, then (1, 2), (1, 3): the key `1` occurs twice
const DUPLICATE_KEY_MAP_BYTES: [u8; 20] = [
    2, 0, 0, 0, //
    1, 0, 0, 0, 2, 0, 0, 0, //
    1, 0, 0, 0, 3, 0, 0, 0,
];

// length 3, then 1, 2, 1: the item `1` occurs twice
const DUPLICATE_ITEM_SET_BYTES: [u8; 16] = [
    3, 0, 0, 0, //
    1, 0, 0, 0, //
    2, 0, 0, 0, //
    1, 0, 0, 0,
];

#[cfg(feature = "de_strict_order")]
const ERROR_DUPLICATE_KEYS: &str = "duplicate keys were encountered during deserialization";

#[test]
fn test_indexmap_serializes_in_insertion_order() {
    let mut a: IndexMap<i32, i32> = IndexMap::new();
    a.insert(1, 10);
    a.insert(2, 20);
    let mut b: IndexMap<i32, i32> = IndexMap::new();
    b.insert(2, 20);
    b.insert(1, 10);

    // `IndexMap`'s `PartialEq` ignores order, but its encoding does not
    assert_eq!(a, b);
    let a_bytes = borsh::to_vec(&a).unwrap();
    assert_ne!(borsh::to_vec(&b).unwrap(), a_bytes);

    // sorting first yields the same bytes as a `BTreeMap` with the same entries
    b.sort_keys();
    let btree: BTreeMap<i32, i32> = a.clone().into_iter().collect();
    assert_eq!(borsh::to_vec(&b).unwrap(), a_bytes);
    assert_eq!(borsh::to_vec(&btree).unwrap(), a_bytes);
}

#[test]
fn test_indexmap_roundtrip_preserves_unsorted_order() {
    let mut original: IndexMap<i32, i32> = IndexMap::new();
    original.insert(3, 30);
    original.insert(1, 10);
    original.insert(2, 20);

    let bytes = borsh::to_vec(&original).unwrap();
    // accepted with and without `de_strict_order`
    let decoded: IndexMap<i32, i32> = from_slice(&bytes).unwrap();
    assert_eq!(
        decoded.keys().copied().collect::<Vec<_>>(),
        original.keys().copied().collect::<Vec<_>>()
    );
    assert_eq!(borsh::to_vec(&decoded).unwrap(), bytes);
}

#[test]
fn test_indexset_roundtrip_preserves_unsorted_order() {
    let mut original: IndexSet<i32> = IndexSet::new();
    original.insert(3);
    original.insert(1);
    original.insert(2);

    let bytes = borsh::to_vec(&original).unwrap();
    // accepted with and without `de_strict_order`
    let decoded: IndexSet<i32> = from_slice(&bytes).unwrap();
    assert_eq!(
        decoded.iter().copied().collect::<Vec<_>>(),
        original.iter().copied().collect::<Vec<_>>()
    );
    assert_eq!(borsh::to_vec(&decoded).unwrap(), bytes);
}

#[cfg(feature = "de_strict_order")]
#[test]
fn test_indexmap_deser_err_duplicate_keys() {
    let err = from_slice::<IndexMap<i32, i32>>(&DUPLICATE_KEY_MAP_BYTES).unwrap_err();
    assert_eq!(err.kind(), borsh::io::ErrorKind::InvalidData);
    assert_eq!(err.to_string(), ERROR_DUPLICATE_KEYS);
}

#[cfg(feature = "de_strict_order")]
#[test]
fn test_indexset_deser_err_duplicate_items() {
    let err = from_slice::<IndexSet<i32>>(&DUPLICATE_ITEM_SET_BYTES).unwrap_err();
    assert_eq!(err.kind(), borsh::io::ErrorKind::InvalidData);
    assert_eq!(err.to_string(), ERROR_DUPLICATE_KEYS);
}

// Without `de_strict_order`, duplicates are still accepted (lossily), as before.
#[cfg(not(feature = "de_strict_order"))]
#[test]
fn test_indexmap_deser_duplicate_keys_last_value_wins() {
    let decoded: IndexMap<i32, i32> = from_slice(&DUPLICATE_KEY_MAP_BYTES).unwrap();
    assert_eq!(decoded.into_iter().collect::<Vec<_>>(), [(1, 3)]);
}

#[cfg(not(feature = "de_strict_order"))]
#[test]
fn test_indexset_deser_duplicate_items_ignored() {
    let decoded: IndexSet<i32> = from_slice(&DUPLICATE_ITEM_SET_BYTES).unwrap();
    assert_eq!(decoded.into_iter().collect::<Vec<_>>(), [1, 2]);
}
