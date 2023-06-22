#![cfg_attr(not(feature = "std"), no_std)]
use borsh::{from_slice, BorshSerialize};

#[test]
fn test_unary_tuple() {
    let expected = (true,);
    let buf = expected.try_to_vec().unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(buf);
    let actual = from_slice::<(bool,)>(&buf).expect("failed to deserialize");
    assert_eq!(actual, expected);
}
