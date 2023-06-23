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

fn default_btreeset() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    set.insert("foo".to_string());
    set.insert("many".to_string());
    set.insert("various".to_string());
    set.insert("different".to_string());
    set.insert("keys".to_string());
    set.insert("one".to_string());
    set
}

fn default_btreemap() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    map.insert("foo".to_string(), "bar".to_string());
    map.insert("one".to_string(), "two".to_string());
    map.insert("great".to_string(), "two".to_string());
    map.insert("variety".to_string(), "seven".to_string());
    map.insert("of".to_string(), "up".to_string());
    map.insert("keys".to_string(), "advertisement".to_string());
    map
}

#[test]
fn test_btreemap() {
    let map = default_btreemap();

    let data = map.try_to_vec().unwrap();
    let actual_map = from_slice::<BTreeMap<String, String>>(&data).unwrap();
    assert_eq!(map, actual_map);
}

#[test]
fn test_btreeset() {
    let set = default_btreeset();

    let data = set.try_to_vec().unwrap();
    let actual_set = from_slice::<BTreeSet<String>>(&data).unwrap();
    assert_eq!(set, actual_set);
}

#[cfg(feature = "de_strict_order")]
const ERROR_WRONG_ORDER_OF_KEYS: &str = "keys were not serialized in ascending order";

#[cfg(feature = "de_strict_order")]
#[test]
fn test_btreeset_deser_err_wrong_order() {
    let mut vec_str = vec![];
    vec_str.push("various".to_string());
    vec_str.push("foo".to_string());
    vec_str.push("many".to_string());

    let data = vec_str.try_to_vec().unwrap();
    let result = from_slice::<BTreeSet<String>>(&data);
    assert!(result.is_err());

    assert_eq!(result.unwrap_err().to_string(), ERROR_WRONG_ORDER_OF_KEYS);
}

#[cfg(feature = "de_strict_order")]
#[test]
fn test_btreemap_deser_err_wrong_order() {
    let val = "val".to_string();
    let mut vec_str = vec![];
    vec_str.push(("various".to_string(), val.clone()));
    vec_str.push(("foo".to_string(), val.clone()));
    vec_str.push(("many".to_string(), val.clone()));

    let data = vec_str.try_to_vec().unwrap();
    let result = from_slice::<BTreeMap<String, String>>(&data);
    assert!(result.is_err());

    assert_eq!(result.unwrap_err().to_string(), ERROR_WRONG_ORDER_OF_KEYS);
}
