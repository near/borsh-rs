#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "derive")]
use core::marker::PhantomData;

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;

#[cfg(hash_collections)]
use core::{cmp::Eq, hash::Hash};

#[cfg(feature = "std")]
use std::collections::{BTreeMap, HashMap};

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec,
    vec::Vec,
};
#[cfg(not(feature = "std"))]
use core::result::Result;

use borsh::{from_slice, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
struct A<T, F, G> {
    x: Vec<T>,
    y: String,
    b: B<F, G>,
    pd: PhantomData<T>,
    c: Result<T, G>,
    d: [u64; 5],
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
enum B<F, G> {
    X { f: Vec<F> },
    Y(G),
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
struct TupleA<T>(T, u32);

#[derive(BorshSerialize, BorshDeserialize, Debug)]
struct NamedA<T> {
    a: T,
    b: u32,
}

/// `T: PartialOrd` bound is required for `BorshSerialize` derive to be successful
/// `T: Hash + Eq` bound is required for `BorshDeserialize` derive to be successful
#[cfg(hash_collections)]
#[derive(BorshSerialize, BorshDeserialize)]
struct C<T: PartialOrd + Hash + Eq, U> {
    a: String,
    b: HashMap<T, U>,
}

/// `T: Ord` bound is required for `BorshDeserialize` derive to be successful
#[derive(BorshSerialize, BorshDeserialize)]
struct D<T: Ord, U> {
    a: String,
    b: BTreeMap<T, U>,
}

#[cfg(hash_collections)]
#[derive(BorshSerialize)]
struct G<K, V, U>(#[borsh_skip] HashMap<K, V>, U);

#[cfg(hash_collections)]
#[derive(BorshDeserialize)]
struct G1<K, V, U>(#[borsh_skip] HashMap<K, V>, U);

#[cfg(hash_collections)]
#[derive(BorshDeserialize)]
struct G2<K: PartialOrd + Hash + Eq, V, U>(HashMap<K, V>, #[borsh_skip] U);

#[cfg(hash_collections)]
#[derive(BorshSerialize, BorshDeserialize)]
struct H<K: Ord, V, U> {
    x: BTreeMap<K, V>,
    #[allow(unused)]
    #[borsh_skip]
    y: U,
}

/// `T: Ord` bound is required for `BorshDeserialize` derive to be successful
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
enum E<T: Ord, U, G> {
    X { f: BTreeMap<T, U> },
    Y(G),
}

#[cfg(hash_collections)]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
enum I1<K, V, U> {
    B {
        #[allow(unused)]
        #[borsh_skip]
        x: HashMap<K, V>,
        y: String,
    },
    C(K, Vec<U>),
}

#[cfg(hash_collections)]
#[derive(BorshSerialize, BorshDeserialize, Debug)]
enum I2<K: PartialOrd + Eq + Hash, V, U> {
    B { x: HashMap<K, V>, y: String },
    C(K, #[borsh_skip] U),
}

#[test]
fn test_generic_struct() {
    let a = A::<String, u64, String> {
        x: vec!["foo".to_string(), "bar".to_string()],
        pd: Default::default(),
        y: "world".to_string(),
        b: B::X { f: vec![1, 2] },
        c: Err("error".to_string()),
        d: [0, 1, 2, 3, 4],
    };
    let data = a.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_a = from_slice::<A<String, u64, String>>(&data).unwrap();
    assert_eq!(a, actual_a);
}

#[cfg(hash_collections)]
#[test]
fn test_generic_struct_hashmap() {
    let mut hashmap = HashMap::new();
    hashmap.insert(34, "another".to_string());
    hashmap.insert(14, "value".to_string());
    let a = C::<u32, String> {
        a: "field".to_string(),
        b: hashmap,
    };
    let data = a.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_a = from_slice::<C<u32, String>>(&data).unwrap();
    assert_eq!(actual_a.b.get(&14), Some("value".to_string()).as_ref());
    assert_eq!(actual_a.b.get(&34), Some("another".to_string()).as_ref());
}

#[test]
fn test_generic_enum() {
    let b: B<String, u64> = B::X {
        f: vec!["one".to_string(), "two".to_string(), "three".to_string()],
    };
    let c: B<String, u64> = B::Y(656556u64);

    let list = vec![b, c];
    let data = list.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_list = from_slice::<Vec<B<String, u64>>>(&data).unwrap();

    assert_eq!(list, actual_list);
}
