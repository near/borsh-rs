#![cfg(feature = "derive")]
use borsh::BorshSerialize;

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;

#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};

/// strangely enough, this worked before current commit
#[cfg(hash_collections)]
#[derive(BorshSerialize)]
struct CRec<U: PartialOrd> {
    a: String,
    b: HashMap<U, CRec<U>>,
}

#[derive(BorshSerialize)]
struct CRecA {
    a: String,
    b: Box<CRecA>,
}

#[derive(BorshSerialize, PartialEq, Eq)]
struct CRecB {
    a: String,
    b: Vec<CRecB>,
}

#[cfg(hash_collections)]
#[derive(BorshSerialize)]
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
    let _data = three.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(_data);
    // let actual_three = from_slice::<CRecB>(&data).unwrap();
}
