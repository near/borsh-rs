//! The `extra(n)` charges for `decode_seq`: what a collection allocates besides the decoded
//! vector, for `n > 0` elements. `None` means an overflow or an invalid layout.

/// For collections that allocate nothing besides that vector.
#[inline]
pub(super) fn none(_n: usize) -> Option<usize> {
    Some(0)
}
