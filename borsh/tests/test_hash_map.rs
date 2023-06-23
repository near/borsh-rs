#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(hash_collections)]

#[cfg(feature = "std")]
use core::hash::BuildHasher;

#[cfg(feature = "hashbrown")]
use hashbrown::{HashMap, HashSet};
#[cfg(feature = "std")]
use std::collections::{
    hash_map::{DefaultHasher, RandomState},
    HashMap, HashSet,
};
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use borsh::{from_slice, BorshSerialize};

fn default_hashset() -> HashSet<String> {
    let mut set = HashSet::new();
    set.insert("foo".to_string());
    set.insert("many".to_string());
    set.insert("various".to_string());
    set.insert("different".to_string());
    set.insert("keys".to_string());
    set.insert("one".to_string());
    set
}

fn default_hashmap() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("foo".to_string(), "bar".to_string());
    map.insert("one".to_string(), "two".to_string());
    map.insert("123mayn".to_string(), "two".to_string());
    map.insert("_more".to_string(), "two".to_string());
    map.insert("diefferent".to_string(), "two".to_string());
    map
}

#[test]
fn test_default_hashmap() {
    let map = default_hashmap();

    let data = map.try_to_vec().unwrap();
    let actual_map = from_slice::<HashMap<String, String>>(&data).unwrap();
    assert_eq!(map, actual_map);
}

#[test]
fn test_default_hashset() {
    let set = default_hashset();

    let data = set.try_to_vec().unwrap();
    let actual_set = from_slice::<HashSet<String>>(&data).unwrap();
    assert_eq!(set, actual_set);
}

#[cfg(feature = "de_strict_order")]
const ERROR_WRONG_ORDER_OF_KEYS: &str = "keys were not serialized in ascending order";

#[cfg(feature = "de_strict_order")]
#[test]
fn test_hashset_deser_err_wrong_order() {
    let mut vec_str = vec![];
    vec_str.push("various".to_string());
    vec_str.push("foo".to_string());
    vec_str.push("many".to_string());

    let data = vec_str.try_to_vec().unwrap();
    let result = from_slice::<HashSet<String>>(&data);
    assert!(result.is_err());

    assert_eq!(result.unwrap_err().to_string(), ERROR_WRONG_ORDER_OF_KEYS);
}

#[cfg(feature = "de_strict_order")]
#[test]
fn test_hashmap_deser_err_wrong_order() {
    let val = "val".to_string();
    let mut vec_str = vec![];
    vec_str.push(("various".to_string(), val.clone()));
    vec_str.push(("foo".to_string(), val.clone()));
    vec_str.push(("many".to_string(), val.clone()));

    let data = vec_str.try_to_vec().unwrap();
    let result = from_slice::<HashMap<String, String>>(&data);
    assert!(result.is_err());

    assert_eq!(result.unwrap_err().to_string(), ERROR_WRONG_ORDER_OF_KEYS);
}

#[derive(Default)]
#[cfg(feature = "std")]
struct NewHasher(RandomState);

#[cfg(feature = "std")]
impl BuildHasher for NewHasher {
    type Hasher = DefaultHasher;
    fn build_hasher(&self) -> DefaultHasher {
        self.0.build_hasher()
    }
}

#[test]
#[cfg(feature = "std")]
fn test_generic_hash_hashmap() {
    let mut map = HashMap::with_hasher(NewHasher::default());
    map.insert("foo".to_string(), "bar".to_string());
    map.insert("one".to_string(), "two".to_string());

    let data = map.try_to_vec().unwrap();
    let actual_map = from_slice::<HashMap<String, String, NewHasher>>(&data).unwrap();
    assert_eq!(map, actual_map);
}

#[test]
#[cfg(feature = "std")]
fn test_generic_hashset() {
    let mut set = HashSet::with_hasher(NewHasher::default());
    set.insert("foo".to_string());
    set.insert("many".to_string());
    set.insert("various".to_string());
    set.insert("different".to_string());
    set.insert("keys".to_string());
    set.insert("one".to_string());

    let data = set.try_to_vec().unwrap();
    let actual_set = from_slice::<HashSet<String, NewHasher>>(&data).unwrap();
    assert_eq!(set, actual_set);
}
