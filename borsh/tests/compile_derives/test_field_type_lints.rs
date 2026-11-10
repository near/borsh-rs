//! The derives paste field types into generated code; lints silenced on the struct or field
//! must stay silenced there.
#![deny(deprecated, unused_parens, unused_qualifications)]
#![allow(dead_code)]

use borsh::{BorshDeserialize, BorshSerialize};

mod old {
    #![allow(deprecated)]

    #[deprecated]
    #[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
    pub struct Old(pub u8);
}

#[allow(deprecated)]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct UsesDeprecated {
    x: old::Old,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct UsesDeprecatedField {
    #[allow(deprecated)]
    x: old::Old,
}

#[allow(unused_parens, unused_qualifications)]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct QualifiedAndParenthesized {
    x: core::primitive::u8,
    y: (u8),
}

/// The generated code must not allow lints either: an `allow` under a `forbid` doesn't compile.
mod forbidden {
    #![forbid(deprecated, unused_parens, unused_qualifications, clippy::all, clippy::use_self)]

    use alloc::{boxed::Box, vec::Vec};
    use borsh::{BorshDeserialize, BorshSerialize};

    #[derive(Clone, BorshSerialize, BorshDeserialize)]
    pub struct Plain(u8, Box<u16>, [u8; 2]);

    #[derive(Clone, BorshSerialize, BorshDeserialize)]
    pub struct Nested<T: Clone> {
        a: Vec<Plain>,
        b: Option<Box<Self>>,
        c: core::marker::PhantomData<T>,
    }
}

/// Names of the caller must not capture the bindings of the generated constant.
mod caller_names {
    #![forbid(dead_code)]
    #![allow(non_upper_case_globals)]

    use borsh::{BorshDeserialize, BorshSerialize};

    pub const __borsh_struct: usize = 0;

    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct UsesConstant(pub u8, pub [u8; __borsh_struct]);

    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct ConstParameter<const __borsh_struct: usize>(pub [u8; __borsh_struct]);

    #[test]
    fn test_caller_names() {
        assert_eq!(borsh::to_vec(&UsesConstant(7, [])).unwrap(), [7]);
        assert_eq!(borsh::to_vec(&ConstParameter::<1>([8])).unwrap(), [8]);
        let decoded = borsh::from_slice::<UsesConstant>(&[9]).unwrap();
        assert_eq!(decoded.0, 9);
        let decoded = borsh::from_slice::<ConstParameter<1>>(&[10]).unwrap();
        assert_eq!(decoded.0, [10]);
    }
}
