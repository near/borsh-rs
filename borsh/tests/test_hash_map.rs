use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{
    hash_map::{DefaultHasher, RandomState},
    HashMap,
};
use std::hash::BuildHasher;

#[test]
fn test_default_hashmap() {
    let mut map = HashMap::new();
    map.insert("foo".to_string(), "bar".to_string());
    map.insert("one".to_string(), "two".to_string());

    let data = map.try_to_vec().unwrap();
    let actual_map = HashMap::<String, String>::try_from_slice(&data).unwrap();
    assert_eq!(map, actual_map);
}

#[derive(Default)]
struct NewHasher(RandomState);

impl BuildHasher for NewHasher {
    type Hasher = DefaultHasher;
    fn build_hasher(&self) -> DefaultHasher {
        self.0.build_hasher()
    }
}

#[test]
fn test_generic_hash_hashmap() {
    let mut map = HashMap::with_hasher(NewHasher::default());
    map.insert("foo".to_string(), "bar".to_string());
    map.insert("one".to_string(), "two".to_string());

    let data = map.try_to_vec().unwrap();
    let actual_map = HashMap::<String, String, NewHasher>::try_from_slice(&data).unwrap();
    assert_eq!(map, actual_map);
}
