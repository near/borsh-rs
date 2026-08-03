use borsh::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;

#[cfg(feature = "std")]
use std::collections::HashMap;

use alloc::{boxed::Box, string::String};
#[cfg(hash_collections)]
use core::{cmp::Eq, hash::Hash};

#[cfg(hash_collections)]
#[allow(unused)]
#[derive(BorshSerialize, BorshDeserialize)]
struct CRec<U: Ord + Hash + Eq> {
    a: String,
    b: HashMap<U, Self>,
}

//  `impl<T, U> BorshDeserialize for Box<T>` pulls in => `ToOwned`
// => pulls in at least `Clone`
#[allow(unused)]
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecA {
    a: String,
    b: Box<Self>,
}

#[cfg(hash_collections)]
#[allow(unused)]
#[derive(BorshSerialize, BorshDeserialize)]
struct CRecC {
    a: String,
    b: HashMap<String, Self>,
}
