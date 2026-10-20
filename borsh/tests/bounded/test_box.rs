use alloc::{boxed::Box, vec, vec::Vec};

use borsh::to_vec;

use crate::common_macro::assert_bounded_parity;

#[test]
fn test_box_parity() {
    assert_bounded_parity::<Box<u64>>(&to_vec(&Box::new(7u64)).unwrap());
    assert_bounded_parity::<Box<str>>(&to_vec(&Box::<str>::from("borsh")).unwrap());
    assert_bounded_parity::<Box<[u64]>>(&to_vec(&Box::<[u64]>::from(&[1, 2, 3][..])).unwrap());
    assert_bounded_parity::<Box<Vec<u64>>>(&to_vec(&Box::new(vec![1u64, 2, 3])).unwrap());
    assert_bounded_parity::<Vec<Box<u64>>>(&to_vec(&vec![Box::new(1u64), Box::new(2)]).unwrap());
}
