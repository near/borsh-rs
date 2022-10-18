#![cfg(feature = "num-bigint")]

use borsh::{BorshDeserialize, BorshSerialize};
use quickcheck::quickcheck;

#[track_caller]
fn assert_encoding<T>(val: T, vector: &[u8])
where
    T: BorshDeserialize + BorshSerialize + PartialEq + core::fmt::Debug,
{
    let serialized = val.try_to_vec().unwrap();
    assert_eq!(T::try_from_slice(&serialized).unwrap(), val);
    assert_eq!(&serialized, vector);
}

#[test]
fn test_bigint_vectors() {
    use num_bigint_dep::{BigInt, BigUint};

    assert_encoding(BigInt::from(0), &[1]);
    assert_encoding(BigInt::from(-1), &[0, 1, 1]);
    assert_encoding(BigInt::from(1), &[2, 1, 1]);
    assert_encoding(BigUint::from(1u32), &[1, 1]);
    assert_encoding(BigInt::from(257), &[2, 2, 1, 1]);
    assert_encoding(BigUint::new(vec![]), &[0]);
}

#[test]
fn test_qc_bigint() {
    use num_bigint_dep::{BigInt, Sign};

    fn prop(a: Option<bool>, value: Vec<u32>) -> bool {
        let sign = match a {
            Some(true) => Sign::Plus,
            Some(false) => Sign::Minus,
            None => Sign::NoSign,
        };
        let value = BigInt::new(sign, value);
        let serialized = value.try_to_vec().unwrap();

        let deserialized = <BigInt>::try_from_slice(&serialized)
            .map_err(|e| format!("failed to deserialize BigInt {value}: {e}"))
            .unwrap();
        deserialized == value
    }

    quickcheck(prop as fn(Option<bool>, Vec<u32>) -> bool);
}

#[test]
fn test_qc_biguint() {
    use num_bigint_dep::BigUint;

    fn prop(value: Vec<u32>) -> bool {
        let value = BigUint::new(value);
        let serialized = value.try_to_vec().unwrap();

        let deserialized = <BigUint>::try_from_slice(&serialized)
            .map_err(|e| format!("failed to deserialize BigUint {value}: {e}"))
            .unwrap();
        deserialized == value
    }

    quickcheck(prop as fn(Vec<u32>) -> bool);
}
