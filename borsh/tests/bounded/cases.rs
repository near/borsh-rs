//! Inputs with known charges, shared by the decoding tests.
//!
//! The formulas below are written out independently of the implementation, from the
//! charging table in `docs/rustdoc_include/borsh_bounded_charges.md`.

#![allow(dead_code)]

use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::mem::size_of;

use borsh::de::Budget;
use borsh::io::Result;
use borsh::{from_slice_bounded, to_vec, BorshDeserializeBounded, BorshSerialize};

pub const MIB: usize = 1024 * 1024;

pub struct Case {
    pub name: String,
    pub input: Vec<u8>,
    /// `size_of::<T>()` plus the allocations.
    pub charge: usize,
    /// `(requested, remaining)` of the limit error at limit `charge - 1`. `None` when the
    /// charge is 0.
    pub below: Option<(usize, usize)>,
    /// Decodes `input` against `budget`, failing on trailing bytes.
    pub decode: fn(&[u8], &mut Budget) -> Result<()>,
}

pub fn decode<T: BorshDeserializeBounded>(input: &[u8], budget: &mut Budget) -> Result<()> {
    from_slice_bounded::<T>(input, budget).map(drop)
}

fn case<T: BorshSerialize + BorshDeserializeBounded>(
    name: String,
    value: &T,
    allocations: usize,
    below: Option<(usize, usize)>,
) -> Case {
    Case {
        name,
        input: to_vec(value).unwrap(),
        charge: size_of::<T>() + allocations,
        below,
        decode: decode::<T>,
    }
}

/// The failure of a case whose last charge, of `last` bytes, does not fit.
fn single(last: usize) -> Option<(usize, usize)> {
    if last == 0 {
        None
    } else {
        Some((last, last - 1))
    }
}

pub fn cases() -> Vec<Case> {
    vec![
        case("()".into(), &(), 0, None),
        case("u64".into(), &7u64, 0, single(size_of::<u64>())),
        case(
            "empty Vec<u64>".into(),
            &Vec::<u64>::new(),
            0,
            single(size_of::<Vec<u64>>()),
        ),
        case(
            "Box<()>".into(),
            &Box::new(()),
            0,
            single(size_of::<Box<()>>()),
        ),
        case("Box<u64>".into(), &Box::new(7u64), 8, single(8)),
        case(
            "Vec<u64> of 1000".into(),
            &vec![7u64; 1000],
            8000,
            single(8000),
        ),
        case("Vec<u8> of 1".into(), &vec![7u8], 1, single(1)),
        case("Vec<u8> of 1 MiB".into(), &vec![7u8; MIB], MIB, single(MIB)),
        case(
            "Vec<u8> of 1 MiB + 1".into(),
            &vec![7u8; MIB + 1],
            MIB + 1,
            single(MIB + 1),
        ),
        case(
            "Vec<u8> of 2 MiB + 1".into(),
            &vec![7u8; 2 * MIB + 1],
            2 * MIB + 1,
            single(2 * MIB + 1),
        ),
        case(
            "String of 5000 bytes".into(),
            &"borsh".repeat(1000),
            5000,
            single(5000),
        ),
        case(
            "Vec<Vec<u8>> of 3 x 10 bytes".into(),
            &vec![vec![7u8; 10]; 3],
            3 * size_of::<Vec<u8>>() + 30,
            // The outer vector and two inner ones fit; the third does not.
            Some((10, 9)),
        ),
        // A box of an unsized value reuses the buffer that was already charged.
        case(
            "Box<[u8]> of 5000".into(),
            &vec![7u8; 5000].into_boxed_slice(),
            5000,
            single(5000),
        ),
        // The box is charged after its contents, so the box charge is the one that fails.
        case(
            "Box<Vec<u8>> of 10".into(),
            &Box::new(vec![7u8; 10]),
            10 + size_of::<Vec<u8>>(),
            Some((size_of::<Vec<u8>>(), size_of::<Vec<u8>>() - 1)),
        ),
        case(
            "empty Box<Vec<u8>>".into(),
            &Box::new(Vec::<u8>::new()),
            size_of::<Vec<u8>>(),
            single(size_of::<Vec<u8>>()),
        ),
    ]
}
