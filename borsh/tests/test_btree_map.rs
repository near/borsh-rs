#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
};

use borsh::{from_slice, BorshSerialize};

#[test]
fn test_btreemap() {
    let mut map = BTreeMap::new();
    map.insert("foo".to_string(), "bar".to_string());
    map.insert("one".to_string(), "two".to_string());
    map.insert("great".to_string(), "two".to_string());
    map.insert("variety".to_string(), "seven".to_string());
    map.insert("of".to_string(), "up".to_string());
    map.insert("keys".to_string(), "advertisement".to_string());

    let data = map.try_to_vec().unwrap();
    let actual_map = from_slice::<BTreeMap<String, String>>(&data).unwrap();
    assert_eq!(map, actual_map);
}

#[test]
fn test_btreeset() {
    let mut set = BTreeSet::new();
    set.insert("foo".to_string());
    set.insert("many".to_string());
    set.insert("various".to_string());
    set.insert("different".to_string());
    set.insert("keys".to_string());
    set.insert("one".to_string());

    let data = set.try_to_vec().unwrap();
    let actual_set = from_slice::<BTreeSet<String>>(&data).unwrap();
    assert_eq!(set, actual_set);
}
