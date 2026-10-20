use borsh::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;

#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(hash_collections)]
use core::{cmp::Eq, hash::Hash};
use alloc::{boxed::Box, string::String};


#[cfg(hash_collections)]
#[allow(unused)]
#[derive(BorshSerialize, BorshDeserialize)]
struct CRec<U: Ord + Hash + Eq> {
    a: String,
    b: HashMap<U, CRec<U>>,
}

//  `impl<T, U> BorshDeserialize for Box<T>` pulls in => `ToOwned`
// => pulls in at least `Clone`
#[allow(unused)]
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecA {
    a: String,
    b: Box<CRecA>,
}


#[cfg(hash_collections)]
#[allow(unused)]
#[derive(BorshSerialize, BorshDeserialize)]
struct CRecC {
    a: String,
    b: HashMap<String, CRecC>,
}

// recursion only through types which forward the hidden `ENCODES_TO_ZERO_BYTES` constant
// (`Box`, arrays, ...) must not make the derived constant cyclic; the types without any finite
// value (`CRecOnlyBox`, `CRecMutualA`, ...) must still compile, as long as they are not used
// as the element of a collection
#[allow(unused)]
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecOnlyBox(Box<CRecOnlyBox>);

#[allow(unused)]
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecEmptyArray {
    a: [Box<CRecEmptyArray>; 0],
    b: u8,
}

#[allow(unused)]
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecSelfKeyword(Box<Self>, Box<()>);

#[allow(unused)]
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecGeneric<T: Clone>(Box<CRecGeneric<T>>, T);

#[allow(unused)]
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecMutualA(Box<CRecMutualB>);

#[allow(unused)]
#[derive(Clone, BorshSerialize, BorshDeserialize)]
struct CRecMutualB(Box<CRecMutualA>);
