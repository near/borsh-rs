//! Inputs with exact limits, shared by the decoding and allocator tests.

use std::io;
use std::mem::size_of;

use borsh::{from_slice, from_slice_with_limit, to_vec, BorshDeserialize};

pub const MIB: usize = 1024 * 1024;

/// Claims `u32::MAX` elements but supplies only four payload bytes.
pub const LYING_LENGTH_PREFIX: [u8; 8] = [0xff, 0xff, 0xff, 0xff, 1, 2, 3, 4];

pub struct Case {
    pub name: &'static str,
    pub input: Vec<u8>,
    /// Exact charged bytes, so one byte less must fail.
    pub limit: usize,
    /// `None` decodes with plain `from_slice`.
    pub decode: fn(&[u8], Option<usize>) -> io::Result<()>,
}

fn decode<T: BorshDeserialize>(input: &[u8], limit: Option<usize>) -> io::Result<()> {
    match limit {
        Some(limit) => from_slice_with_limit::<T>(input, limit).map(drop),
        None => from_slice::<T>(input).map(drop),
    }
}

/// Mirrors the decoder: room for 4096 bytes of `T`, then doubling. Assumes
/// `len > 0` and a nonzero-sized `T` without a byte fast path.
pub fn decoded_capacity<T>(len: usize) -> usize {
    let mut capacity = (4096 / size_of::<T>()).clamp(1, len);
    while capacity < len {
        capacity *= 2;
    }
    capacity
}

pub fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "Vec<u64> of 1000",
            input: to_vec(&vec![7u64; 1000]).unwrap(),
            // The 4096-byte initial buffer doubles to hold 1000 u64s.
            limit: 8192,
            decode: decode::<Vec<u64>>,
        },
        Case {
            name: "Vec<u8> of 3 MiB",
            input: to_vec(&vec![7u8; 3 * MIB]).unwrap(),
            limit: 3 * MIB,
            decode: decode::<Vec<u8>>,
        },
        Case {
            name: "Vec<Vec<u8>> of 1000 empty vectors",
            input: to_vec(&vec![Vec::<u8>::new(); 1000]).unwrap(),
            // On 64-bit targets, 4096 / 24 = 170 slots double to 1360.
            limit: decoded_capacity::<Vec<u8>>(1000) * size_of::<Vec<u8>>(),
            decode: decode::<Vec<Vec<u8>>>,
        },
        Case {
            name: "String of 5000 bytes",
            input: to_vec(&"borsh".repeat(1000)).unwrap(),
            limit: 5000,
            decode: decode::<String>,
        },
    ]
}
