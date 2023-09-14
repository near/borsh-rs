#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "unstable__schema")]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::ToString, vec::Vec};

use borsh::schema::*;
use borsh::BorshSchema;

#[track_caller]
fn test_ok<T: BorshSchema>() {
    let schema = BorshSchemaContainer::for_type::<T>();
    assert_eq!(Ok(()), schema.validate());
}

#[track_caller]
fn test_err<T: BorshSchema>(err: ValidateError) {
    let schema = BorshSchemaContainer::for_type::<T>();
    assert_eq!(Err(err), schema.validate());
}

#[test]
fn validate_for_derived_types() {
    #[derive(BorshSchema)]
    pub struct Empty;

    #[derive(BorshSchema)]
    pub struct Named {
        _foo: usize,
        _bar: [u8; 15],
    }

    #[derive(BorshSchema)]
    pub struct Unnamed(usize, [u8; 15]);

    #[derive(BorshSchema)]
    struct Recursive(Option<Box<Recursive>>);

    #[derive(BorshSchema)]
    struct RecursiveSequence(Vec<RecursiveSequence>);

    // thankfully, this one cannot be constructed
    #[derive(BorshSchema)]
    struct RecursiveArray(Box<[RecursiveArray; 3]>);

    test_ok::<Empty>();
    test_ok::<Named>();
    test_ok::<Unnamed>();
    test_ok::<BorshSchemaContainer>();
    test_ok::<Recursive>();
    test_ok::<RecursiveSequence>();
    test_ok::<RecursiveArray>();
}

#[test]
fn validate_for_zst_sequences() {
    test_err::<Vec<Vec<()>>>(ValidateError::ZSTSequence);
    test_err::<Vec<core::ops::RangeFull>>(ValidateError::ZSTSequence);
}
