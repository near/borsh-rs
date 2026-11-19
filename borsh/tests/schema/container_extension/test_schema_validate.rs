use crate::common_macro::schema_imports::*;

use alloc::{boxed::Box, collections::BTreeMap, format, string::ToString, vec::Vec};


#[track_caller]
fn test_ok<T: BorshSchema>() {
    let schema = BorshSchemaContainer::for_type::<T>();
    assert_eq!(Ok(()), schema.validate());
}

#[track_caller]
fn test_err<T: BorshSchema>(err: SchemaContainerValidateError) {
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
    #[allow(unused)]
    pub struct Unnamed(usize, [u8; 15]);

    #[derive(BorshSchema)]
    #[allow(unused)]
    struct Recursive(Option<Box<Recursive>>);

    #[derive(BorshSchema)]
    #[allow(unused)]
    struct RecursiveSequence(Vec<RecursiveSequence>);

    // thankfully, this one cannot be constructed
    #[derive(BorshSchema)]
    #[allow(unused)]
    struct RecursiveArray(Box<[RecursiveArray; 3]>);

    test_ok::<Empty>();
    test_ok::<Named>();
    test_ok::<Unnamed>();
    test_ok::<BorshSchemaContainer>();
    test_ok::<Recursive>();
    test_ok::<RecursiveSequence>();
    test_ok::<RecursiveArray>();

    test_ok::<[(); 300]>();
}

#[test]
fn validate_for_zst_sequences() {
    test_err::<Vec<Vec<()>>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<()>".to_string(),
    ));
    test_err::<Vec<core::ops::RangeFull>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<RangeFull>".to_string(),
    ));
}

/// The schema describes the encoding, and transparent wrappers declare their inner type,
/// so it already agrees with the runtime guard on types which encode to zero bytes,
/// although they are not zero-sized in memory.
#[test]
fn validate_for_zero_encoded_sequences() {
    #[derive(BorshSchema)]
    #[allow(unused)]
    struct Wrapper(Box<()>);

    #[derive(BorshSchema)]
    #[allow(unused)]
    struct Skipped {
        #[borsh(skip)]
        x: u64,
    }

    test_err::<Vec<Box<()>>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<()>".to_string(),
    ));
    test_err::<Vec<Box<[u8; 0]>>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<[u8; 0]>".to_string(),
    ));
    test_err::<Vec<core::cell::RefCell<()>>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<()>".to_string(),
    ));
    test_err::<Vec<core::ops::RangeInclusive<()>>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<RangeInclusive<()>>".to_string(),
    ));
    test_err::<Vec<(Box<()>, ())>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<((), ())>".to_string(),
    ));
    test_err::<Vec<Wrapper>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<Wrapper>".to_string(),
    ));
    test_err::<Vec<Skipped>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<Skipped>".to_string(),
    ));
    // the same zero-sized array type twice must not be mistaken for recursion
    #[derive(BorshSchema)]
    #[allow(unused)]
    struct RepeatedArrays([u8; 0], [u8; 0], Box<()>);

    test_err::<Vec<RepeatedArrays>>(SchemaContainerValidateError::ZSTSequence(
        "Vec<RepeatedArrays>".to_string(),
    ));
    test_ok::<Vec<Box<u8>>>();
    test_ok::<Vec<(Box<()>, u8)>>();
    test_ok::<Vec<Option<Box<()>>>>();
}

#[test]
fn validate_bound_vec() {
    #[allow(dead_code)]
    struct BoundVec<const W: u8, const N: u64>;

    impl<const W: u8, const N: u64> BorshSchema for BoundVec<W, N> {
        fn declaration() -> Declaration {
            format!("BoundVec<{}, {}>", W, N)
        }
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            let definition = Definition::Sequence {
                length_width: W,
                length_range: 0..=N,
                elements: "u8".to_string(),
            };
            add_definition(Self::declaration(), definition, definitions);
            u8::add_definitions_recursively(definitions);
        }
    }

    test_ok::<BoundVec<4, { u16::MAX as u64 }>>();
    test_err::<BoundVec<1, { u16::MAX as u64 }>>(SchemaContainerValidateError::TagTooNarrow(
        "BoundVec<1, 65535>".to_string(),
    ));

    test_ok::<BoundVec<1, { u8::MAX as u64 }>>();
    test_ok::<BoundVec<0, { u16::MAX as u64 }>>();
}
