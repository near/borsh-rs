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

use borsh::maybestd::collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque};
use borsh::{from_slice, BorshDeserialize, BorshSerialize};
use borsh_derive::borsh;
use bytes::{Bytes, BytesMut};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
#[borsh_init(init)]
struct A<'a> {
    x: u64,
    b: B,
    y: f32,
    z: String,
    t: (String, u64),
    btree_map_string: BTreeMap<String, String>,
    btree_set_u64: BTreeSet<u64>,
    linked_list_string: LinkedList<String>,
    vec_deque_u64: VecDeque<u64>,
    bytes: Bytes,
    bytes_mut: BytesMut,
    v: Vec<String>,
    w: Box<[u8]>,
    box_str: Box<str>,
    i: [u8; 32],
    u: Result<String, String>,
    lazy: Option<u64>,
    c: borrow::Cow<'a, str>,
    cow_arr: borrow::Cow<'a, [borrow::Cow<'a, str>]>,
    range_u32: ops::Range<u32>,
    #[borsh_skip]
    skipped: Option<u64>,
}

impl A<'_> {
    pub fn init(&mut self) {
        if let Some(v) = self.lazy.as_mut() {
            *v *= 10;
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
struct B {
    x: u64,
    y: i32,
    c: C,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
enum C {
    C1,
    C2(u64),
    C3(u64, u64),
    C4 { x: u64, y: u64 },
    C5(D),
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
struct D {
    x: u64,
}

#[derive(BorshSerialize)]
struct E<'a, 'b> {
    a: &'a A<'b>,
}

#[derive(BorshSerialize)]
struct F1<'a, 'b> {
    aa: &'a [&'a A<'b>],
}

#[derive(BorshDeserialize)]
struct F2<'b> {
    aa: Vec<A<'b>>,
}

#[borsh(use_discriminant = true)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
enum X {
    A,
    B = 20,
    C,
    D,
    E = 10,
    F,
}

#[test]
fn test_discriminant_serialization() {
    let values = vec![X::A, X::B, X::C, X::D, X::E, X::F];
    for value in values {
        assert_eq!(value.try_to_vec().unwrap(), [value as u8]);
    }
}

#[test]
fn test_discriminant_deserialization() {
    let values = vec![X::A, X::B, X::C, X::D, X::E, X::F];
    for value in values {
        assert_eq!(from_slice::<X>(&[value as u8]).unwrap(), value,);
    }
}

#[test]
#[should_panic = "Unexpected variant tag: 2"]
fn test_deserialize_invalid_discriminant() {
    from_slice::<X>(&[2]).unwrap();
}

#[test]
fn test_simple_struct() {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    map.insert("test".into(), "test".into());
    let mut set: BTreeSet<u64> = BTreeSet::new();
    set.insert(u64::MAX);
    let cow_arr = [
        borrow::Cow::Borrowed("Hello1"),
        borrow::Cow::Owned("Hello2".to_string()),
    ];
    let a = A {
        x: 1,
        b: B {
            x: 2,
            y: 3,
            c: C::C5(D { x: 1 }),
        },
        y: 4.0,
        z: "123".to_string(),
        t: ("Hello".to_string(), 10),
        btree_map_string: map.clone(),
        btree_set_u64: set.clone(),
        linked_list_string: vec!["a".to_string(), "b".to_string()].into_iter().collect(),
        vec_deque_u64: vec![1, 2, 3].into_iter().collect(),
        bytes: vec![5, 4, 3, 2, 1].into(),
        bytes_mut: BytesMut::from(&[1, 2, 3, 4, 5][..]),
        v: vec!["qwe".to_string(), "zxc".to_string()],
        w: vec![0].into_boxed_slice(),
        box_str: Box::from("asd"),
        i: [4u8; 32],
        u: Ok("Hello".to_string()),
        lazy: Some(5),
        c: borrow::Cow::Borrowed("Hello"),
        cow_arr: borrow::Cow::Borrowed(&cow_arr),
        range_u32: 12..71,
        skipped: Some(6),
    };
    let encoded_a = a.try_to_vec().unwrap();
    let e = E { a: &a };
    let encoded_ref_a = e.try_to_vec().unwrap();
    assert_eq!(encoded_ref_a, encoded_a);
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded_a);

    let decoded_a = from_slice::<A>(&encoded_a).unwrap();
    let expected_a = A {
        x: 1,
        b: B {
            x: 2,
            y: 3,
            c: C::C5(D { x: 1 }),
        },
        y: 4.0,
        z: a.z.clone(),
        t: ("Hello".to_string(), 10),
        btree_map_string: map,
        btree_set_u64: set,
        linked_list_string: vec!["a".to_string(), "b".to_string()].into_iter().collect(),
        vec_deque_u64: vec![1, 2, 3].into_iter().collect(),
        bytes: vec![5, 4, 3, 2, 1].into(),
        bytes_mut: BytesMut::from(&[1, 2, 3, 4, 5][..]),
        v: a.v.clone(),
        w: a.w.clone(),
        box_str: Box::from("asd"),
        i: a.i,
        u: Ok("Hello".to_string()),
        lazy: Some(50),
        c: borrow::Cow::Owned("Hello".to_string()),
        cow_arr: borrow::Cow::Owned(vec![
            borrow::Cow::Borrowed("Hello1"),
            borrow::Cow::Owned("Hello2".to_string()),
        ]),
        range_u32: 12..71,
        skipped: None,
    };

    assert_eq!(expected_a, decoded_a);

    let f1 = F1 { aa: &[&a, &a] };
    let encoded_f1 = f1.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded_f1);
    let decoded_f2 = from_slice::<F2>(&encoded_f1).unwrap();
    assert_eq!(decoded_f2.aa.len(), 2);
    assert!(decoded_f2.aa.iter().all(|f2_a| f2_a == &expected_a));
}
