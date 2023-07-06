#![cfg(feature = "derive")]
use borsh::{from_slice, BorshDeserialize, BorshSerialize};

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;

#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(hash_collections)]
use core::{cmp::Eq, hash::Hash};

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};

#[cfg(hash_collections)]
#[derive(BorshSerialize, BorshDeserialize)]
struct CRec<U: PartialOrd + Hash + Eq> {
    a: String,
    b: HashMap<U, CRec<U>>,
}

//  `impl<T, U> BorshDeserialize for Box<T>` pulls in => `ToOwned`
// => pulls in at least `Clone`
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecA {
    a: String,
    b: Box<CRecA>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
struct CRecB {
    a: String,
    b: Vec<CRecB>,
}

#[cfg(hash_collections)]
#[derive(BorshSerialize, BorshDeserialize)]
struct CRecC {
    a: String,
    b: HashMap<String, CRecC>,
}

#[test]
fn test_recursive_struct() {
    let one = CRecB {
        a: "one".to_string(),
        b: vec![],
    };
    let two = CRecB {
        a: "two".to_string(),
        b: vec![],
    };

    let three = CRecB {
        a: "three".to_string(),
        b: vec![one, two],
    };
    let data = three.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_three = from_slice::<CRecB>(&data).unwrap();
    assert_eq!(three, actual_three);
}
