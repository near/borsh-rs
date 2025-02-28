use alloc::collections::BTreeMap;
#[allow(unused)]
use alloc::{string::String, vec::Vec};
#[cfg(hash_collections)]
use core::{cmp::Eq, hash::Hash};
#[cfg(feature = "std")]
use std::collections::HashMap;

use borsh::{BorshDeserializeAsync, BorshSerializeAsync};
#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;

/// `T: Ord` bound is required for `BorshDeserialize` derive to be successful
#[derive(BorshSerializeAsync, BorshDeserializeAsync, PartialEq, Debug)]
enum E<T: Ord, U, W> {
    X { f: BTreeMap<T, U> },
    Y(W),
}

#[cfg(hash_collections)]
#[derive(BorshSerializeAsync, BorshDeserializeAsync, Debug)]
enum I1<K, V, R> {
    B {
        #[allow(unused)]
        #[borsh(skip, async_bound(serialize = "V: Sync", deserialize = "V: Send"))]
        x: HashMap<K, V>,
        y: String,
    },
    C(K, Vec<R>),
}

#[cfg(hash_collections)]
#[derive(BorshSerializeAsync, BorshDeserializeAsync, Debug)]
enum I2<K: Ord + Eq + Hash, R, U> {
    B {
        x: HashMap<K, R>,
        y: String,
    },
    C(
        K,
        #[borsh(
            skip,
            async_bound(serialize = "U: Sync", deserialize = "U: Default + Send")
        )]
        U,
    ),
}
