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
