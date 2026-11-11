use alloc::{boxed::Box, vec, vec::Vec};
use core::mem::size_of;

use borsh::de::Budget;
use borsh::io::{Read, Result};
use borsh::{from_reader_bounded, from_slice_bounded, to_vec, BorshDeserializeBounded};

use crate::common_macro::{assert_bounded_parity, limit_exceeded};

#[test]
fn test_box_parity() {
    assert_bounded_parity::<Box<u64>>(&to_vec(&Box::new(7u64)).unwrap());
    assert_bounded_parity::<Box<str>>(&to_vec(&Box::<str>::from("borsh")).unwrap());
    assert_bounded_parity::<Box<[u64]>>(&to_vec(&Box::<[u64]>::from(&[1, 2, 3][..])).unwrap());
    assert_bounded_parity::<Box<Vec<u64>>>(&to_vec(&Box::new(vec![1u64, 2, 3])).unwrap());
    assert_bounded_parity::<Vec<Box<u64>>>(&to_vec(&vec![Box::new(1u64), Box::new(2)]).unwrap());
}

#[test]
fn test_vec_of_boxed_units_fails_at_its_length() {
    // Claims 2^27 boxes, which take no input bytes.
    let input = [0, 0, 0, 8, 1, 2, 3];
    let mut reader = &input[..];
    let err = from_reader_bounded::<_, Vec<Box<()>>>(&mut reader, &mut Budget::from_bytes(64))
        .unwrap_err();
    let requested = (1 << 27) * size_of::<Box<()>>();
    let remaining = 64 - size_of::<Vec<Box<()>>>();
    assert_eq!(limit_exceeded(&err), Some((64, requested, remaining)));
    assert_eq!(reader.len(), 3);
}

#[allow(unused)]
#[derive(Clone)]
enum Chain {
    End,
    Link(Box<Chain>),
}

impl BorshDeserializeBounded for Chain {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        match u8::deserialize_bounded_alloc(reader, budget)? {
            0 => Ok(Chain::End),
            _ => Ok(Chain::Link(Box::deserialize_bounded_alloc(reader, budget)?)),
        }
    }
}

#[test]
fn test_deep_box_chain_fails_on_the_way_down() {
    let limit = 10 * size_of::<Chain>();
    let input = vec![1u8; 1_000_000];
    let err = from_slice_bounded::<Chain>(&input, &mut Budget::from_bytes(limit))
        .err()
        .expect("the chain never ends");
    // The root and 9 boxes fit.
    assert_eq!(limit_exceeded(&err), Some((limit, size_of::<Chain>(), 0)));
}
