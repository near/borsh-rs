//! Charge formulas for the allocations collections make while they are built from a decoded
//! vector. The rules and their exactness are documented in `borsh_bounded_charges.md`.
//!
//! Each function returns the bytes for `n > 0` elements, or `None` when the arithmetic
//! overflows or one of the allocations would not have a valid `Layout`. The temporary
//! `Vec` the elements are decoded into is charged separately, by `decode_seq`.

/// Used by the collections that keep no allocation besides the decoded vector.
#[inline]
pub(super) fn none(_n: usize) -> Option<usize> {
    Some(0)
}
