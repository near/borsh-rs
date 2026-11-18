use alloc::vec;

use borsh::{from_slice, to_vec, BorshDeserialize, BorshSerialize};

// sequence, no unit enums
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
#[repr(u16)]
enum XY {
    A,
    B = 20,
    C,
    D(u32, u32),
    E = 10,
    F(u64),
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = false)]
#[repr(u16)]
enum XYNoDiscriminant {
    A,
    B = 20,
    C,
    D(u32, u32),
    E = 10,
    F(u64),
}

#[test]
fn test_discriminant_serde_no_unit_type() {
    let values = vec![XY::A, XY::B, XY::C, XY::E, XY::D(12, 14), XY::F(35325423)];
    let expected_discriminants = [0u8, 20, 21, 10, 22, 11];

    for (ind, value) in values.iter().enumerate() {
        let data = to_vec(value).unwrap();
        assert_eq!(data[0], expected_discriminants[ind]);
        assert_eq!(from_slice::<XY>(&data).unwrap(), values[ind]);
    }
}

#[test]
fn test_discriminant_serde_no_unit_type_no_use_discriminant() {
    let values = vec![
        XYNoDiscriminant::A,
        XYNoDiscriminant::B,
        XYNoDiscriminant::C,
        XYNoDiscriminant::D(12, 14),
        XYNoDiscriminant::E,
        XYNoDiscriminant::F(35325423),
    ];
    let expected_discriminants = [0u8, 1, 2, 3, 4, 5];

    for (ind, value) in values.iter().enumerate() {
        let data = to_vec(value).unwrap();
        assert_eq!(data[0], expected_discriminants[ind]);
        assert_eq!(from_slice::<XYNoDiscriminant>(&data).unwrap(), values[ind]);
    }
}

// minimal
#[derive(BorshSerialize)]
#[borsh(use_discriminant = true)]
enum MyDiscriminantEnum {
    A = 20,
}

#[derive(BorshSerialize)]
#[borsh(use_discriminant = false)]
enum MyDiscriminantEnumFalse {
    A = 20,
}

#[derive(BorshSerialize)]
enum MyEnumNoDiscriminant {
    A,
}
#[test]
fn test_discriminant_minimal_true() {
    assert_eq!(MyDiscriminantEnum::A as u8, 20);
    assert_eq!(to_vec(&MyDiscriminantEnum::A).unwrap(), vec![20]);
}

#[test]
fn test_discriminant_minimal_false() {
    assert_eq!(MyDiscriminantEnumFalse::A as u8, 20);
    assert_eq!(
        to_vec(&MyEnumNoDiscriminant::A).unwrap(),
        to_vec(&MyDiscriminantEnumFalse::A).unwrap(),
    );
    assert_eq!(to_vec(&MyDiscriminantEnumFalse::A).unwrap(), vec![0]);
}

// sequence
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = false)]
enum XNoDiscriminant {
    A,
    B = 20,
    C,
    D,
    E = 10,
    F,
}

#[test]
fn test_discriminant_serde_no_use_discriminant() {
    let values = vec![
        XNoDiscriminant::A,
        XNoDiscriminant::B,
        XNoDiscriminant::C,
        XNoDiscriminant::D,
        XNoDiscriminant::E,
        XNoDiscriminant::F,
    ];
    let expected_discriminants = [0u8, 1, 2, 3, 4, 5];
    for (index, value) in values.iter().enumerate() {
        let data = to_vec(value).unwrap();
        assert_eq!(data[0], expected_discriminants[index]);
        assert_eq!(from_slice::<XNoDiscriminant>(&data).unwrap(), values[index]);
    }
}
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
struct D {
    x: u64,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
enum C {
    C1,
    C2(u64),
    C3(u64, u64),
    C4 { x: u64, y: u64 },
    C5(D),
}

#[test]
fn test_enum_tuples() {
    let values = vec![
        C::C1,
        C::C2(u64::MAX),
        C::C3(1, 2),
        C::C4 { x: 0, y: 100 },
        C::C5(D { x: u64::MAX }),
    ];
    for value in values {
        assert_eq!(from_slice::<C>(&to_vec(&value).unwrap()).unwrap(), value);
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
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
        assert_eq!(to_vec(&value).unwrap(), [value as u8]);
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
fn test_discriminant_serde() {
    let values = vec![X::A, X::B, X::C, X::D, X::E, X::F];
    let expected_discriminants = [0u8, 20, 21, 22, 10, 11];
    for (index, value) in values.iter().enumerate() {
        let data = to_vec(value).unwrap();
        assert_eq!(data[0], expected_discriminants[index]);
        assert_eq!(from_slice::<X>(&data).unwrap(), values[index]);
    }
}

const READ: u8 = 1;
const WRITE: u8 = 2;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
enum Operators {
    A = READ | WRITE,
    B, // 4, not `READ | WRITE + 1 == 3`
    C = 1 << 4,
    D, // 17, not `1 << 5 == 32`
}

#[test]
fn test_discriminant_after_operator_expression() {
    for value in [Operators::A, Operators::B, Operators::C, Operators::D] {
        let tag = value as u8;
        assert_eq!(to_vec(&value).unwrap(), vec![tag]);
        assert_eq!(from_slice::<Operators>(&[tag]).unwrap(), value);
    }
    assert_eq!(Operators::B as u8, 4);
    assert_eq!(Operators::D as u8, 17);
}

// Enums that the check that every tag fits in `u8` must keep accepting.
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
#[repr(u64)]
enum UpToU8Max {
    A = 0xfe,
    B,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
enum SelfReferring {
    A = Self::BASE,
    B,
}

impl SelfReferring {
    const BASE: u8 = 0x10;
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Debug)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
enum Generic<T> {
    A(T) = 0xfe,
    B(T),
}

#[test]
fn test_discriminant_fits_in_u8() {
    assert_eq!(to_vec(&UpToU8Max::B).unwrap(), vec![0xff]);
    assert_eq!(from_slice::<UpToU8Max>(&[0xff]).unwrap(), UpToU8Max::B);
    assert_eq!(to_vec(&SelfReferring::B).unwrap(), vec![0x11]);
    assert_eq!(from_slice::<SelfReferring>(&[0x11]).unwrap(), SelfReferring::B);
    assert_eq!(to_vec(&Generic::B(7u8)).unwrap(), vec![0xff, 7]);
    assert_eq!(from_slice::<Generic<u8>>(&[0xff, 7]).unwrap(), Generic::B(7));
}

#[allow(unused_braces)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
enum Blocks {
    A = { 3 },
    B,
    C = { READ | WRITE } << 4,
    D,
}

#[test]
fn test_discriminant_block_expression() {
    for value in [Blocks::A, Blocks::B, Blocks::C, Blocks::D] {
        let tag = value as u8;
        assert_eq!(to_vec(&value).unwrap(), vec![tag]);
        assert_eq!(from_slice::<Blocks>(&[tag]).unwrap(), value);
    }
    assert_eq!(Blocks::D as u8, 0x31);
}

// Tags are evaluated in the repr type, as rustc does, not in `u8`.
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
enum Shifted {
    A,
    B = 16 << 4 >> 2,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
#[repr(i32)]
enum Negative {
    A = -1 + 1,
    B = -(-128),
}

#[test]
fn test_discriminant_evaluated_in_repr_type() {
    assert_eq!(Shifted::B as u8, 64);
    assert_eq!(to_vec(&Shifted::B).unwrap(), vec![64]);
    assert_eq!(from_slice::<Shifted>(&[64]).unwrap(), Shifted::B);
    assert_eq!(from_slice::<Shifted>(&[0]).unwrap(), Shifted::A);
    assert_eq!(to_vec(&Negative::B).unwrap(), vec![128]);
    assert_eq!(from_slice::<Negative>(&[128]).unwrap(), Negative::B);
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
#[borsh(use_discriminant = true)]
#[repr(r#u8)]
enum RawRepr {
    // Evaluated in `u8`, as rustc does for `repr(u8)`: 0, not 64.
    A = 16 << 4 >> 2,
    B = 64,
}

#[test]
fn test_raw_repr_ident() {
    assert_eq!(RawRepr::A as u8, 0);
    assert_eq!(to_vec(&RawRepr::A).unwrap(), vec![0]);
    assert_eq!(to_vec(&RawRepr::B).unwrap(), vec![64]);
    assert_eq!(from_slice::<RawRepr>(&[64]).unwrap(), RawRepr::B);
}

/// Names in scope at the derive must not change how tags are evaluated or checked.
#[allow(non_camel_case_types, non_upper_case_globals, dead_code)]
mod names_in_scope {
    use borsh::{BorshDeserialize, BorshSerialize};

    type isize = core::primitive::u8;
    type i128 = core::primitive::u8;

    #[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
    #[borsh(use_discriminant = true)]
    pub enum AliasedRepr {
        A,
        B = 16 << 4 >> 2,
    }

    const __borsh_tag: core::primitive::u8 = 3;

    #[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
    #[borsh(use_discriminant = true)]
    #[repr(u8)]
    pub enum NamedConst {
        A = __borsh_tag,
        B,
    }

    #[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
    #[borsh(use_discriminant = true)]
    pub enum BorshTags {
        A,
        B = 5,
    }

    pub trait UserTags {
        const TAGS: core::primitive::isize;
    }

    #[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
    #[borsh(use_discriminant = true)]
    pub enum SelfTags {
        A = Self::TAGS,
        B,
    }

    impl UserTags for SelfTags {
        const TAGS: core::primitive::isize = 7;
    }
}

#[test]
fn test_names_in_scope() {
    use names_in_scope::*;
    assert_eq!(to_vec(&AliasedRepr::B).unwrap(), vec![64]);
    assert_eq!(from_slice::<AliasedRepr>(&[64]).unwrap(), AliasedRepr::B);
    assert_eq!(to_vec(&NamedConst::B).unwrap(), vec![4]);
    assert_eq!(from_slice::<NamedConst>(&[3]).unwrap(), NamedConst::A);
    assert_eq!(to_vec(&BorshTags::B).unwrap(), vec![5]);
    assert_eq!(to_vec(&SelfTags::A).unwrap(), vec![7]);
    assert_eq!(from_slice::<SelfTags>(&[8]).unwrap(), SelfTags::B);
}
