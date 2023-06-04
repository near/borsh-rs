use borsh::to_vec;
use borsh::BorshDeserialize;
use borsh::BorshSerialize;

#[derive(BorshDeserialize, PartialEq, Debug)]
struct A();

#[test]
fn test_deserialize_zero_size() {
    let v = [0u8, 0u8, 0u8, 64u8];
    let res = Vec::<A>::try_from_slice(&v);
    assert!(res.is_err());
}

#[derive(BorshDeserialize, BorshSerialize, PartialEq, Debug)]
struct B(u32);
#[test]
fn test_deserialize_non_zero_size() {
    let v = [1, 0, 0, 0, 64, 0, 0, 0];
    let res = Vec::<B>::try_from_slice(&v);
    assert!(res.is_ok());
}
