#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "derive")]

#[cfg(feature = "std")]
use std::{
    borrow,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    ops,
};

#[cfg(not(feature = "std"))]
use core::{ops, result::Result};

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{
    borrow,
    boxed::Box,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    string::{String, ToString},
    vec,
    vec::Vec,
};

use bytes::{Bytes, BytesMut};

use borsh::{from_slice, BorshDeserialize, BorshSerialize};
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
#[borsh(init=init)]
struct A {
    lazy: Option<u64>,
}

impl A {
    pub fn init(&mut self) {
        if let Some(v) = self.lazy.as_mut() {
            *v *= 10;
        }
    }
}
#[test]
fn test_simple_struct() {
    let a = A { lazy: Some(5) };
    let encoded_a = a.try_to_vec().unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded_a);

    let decoded_a = from_slice::<A>(&encoded_a).unwrap();
    let expected_a = A { lazy: Some(50) };
    assert_eq!(expected_a, decoded_a);
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
#[borsh(init=initializon_method)]
enum AEnum {
    A,
    B,
    C,
}

impl AEnum {
    pub fn initializon_method(&mut self) {
        *self = AEnum::C;
    }
}

#[test]
fn test_simple_enum() {
    let a = AEnum::B;
    let encoded_a = a.try_to_vec().unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded_a);

    let decoded_a = from_slice::<AEnum>(&encoded_a).unwrap();
    assert_eq!(AEnum::C, decoded_a);
}
