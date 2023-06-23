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

#[test]
fn test_default_hashmap() {
    let mut map = HashMap::new();
    map.insert("foo".to_string(), "bar".to_string());
    map.insert("one".to_string(), "two".to_string());

    let data = map.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_map = from_slice::<HashMap<String, String>>(&data).unwrap();
    assert_eq!(map, actual_map);
}

#[test]
fn test_default_hashset() {
    let mut set = HashSet::new();
    set.insert("foo".to_string());
    set.insert("many".to_string());
    set.insert("various".to_string());
    set.insert("different".to_string());
    set.insert("keys".to_string());
    set.insert("one".to_string());

    let data = set.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_set = from_slice::<HashSet<String>>(&data).unwrap();
    assert_eq!(set, actual_set);
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
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
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
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_set = from_slice::<HashSet<String, NewHasher>>(&data).unwrap();
    assert_eq!(set, actual_set);
}
