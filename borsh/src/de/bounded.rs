//! Bounded decoding: each impl decodes like its `BorshDeserialize` twin in `de.rs`, plus a
//! charge before each allocation. Collections decode through the same `read_seq` as plain
//! ones, scalar twins come from `de.rs`'s own type macros, and the reader entry point ends with
//! the same `expect_end`. `assert_bounded_parity` in the tests catches drift.

use core::convert::TryFrom;
use core::mem::size_of;

use crate::__private::maybestd::{
    borrow::{Borrow, ToOwned},
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use crate::io::{Read, Result};

use super::{expect_end, read_seq, BorshDeserialize, Elements, RecursionGuard};
use super::{invalid, ERROR_NOT_ALL_BYTES_READ};

mod budget;
mod size;

pub use budget::{Budget, LimitExceeded};

#[doc = include_str!("../../docs/rustdoc_include/borsh_bounded_charges.md")]
pub trait BorshDeserializeBounded: Sized {
    /// Deserializes this instance from a reader, charging `budget` before every allocation.
    /// `size_of::<Self>()` is charged by the entry points, or by the allocation that holds it.
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self>;

    /// Same as [`from_slice_bounded`].
    fn try_from_slice_bounded(v: &[u8], budget: &mut Budget) -> Result<Self> {
        from_slice_bounded(v, budget)
    }

    /// Same as [`from_reader_bounded`].
    fn try_from_reader_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        from_reader_bounded(reader, budget)
    }

    /// Reads `len` elements in one go into a vector the caller has already charged for.
    /// `None` must leave the reader untouched.
    #[inline]
    #[doc(hidden)]
    fn vec_from_reader_bounded<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>> {
        let _ = (len, reader);
        Ok(None)
    }

    /// Whether every value encodes to zero bytes, as in
    /// [`BorshDeserialize`](crate::BorshDeserialize). Collections reject such element types.
    #[doc(hidden)]
    const ENCODES_TO_ZERO_BYTES: bool = false;
}

/// Bounded counterpart of [`EnumExt`](crate::de::EnumExt).
pub trait EnumExtBounded: BorshDeserializeBounded {
    /// Decodes the variant for an already read `tag`, charging `budget` like
    /// [`deserialize_bounded_alloc`](BorshDeserializeBounded::deserialize_bounded_alloc). Like
    /// [`EnumExt::deserialize_variant`](crate::de::EnumExt::deserialize_variant), it lets an
    /// impl check the tag before the fields are decoded.
    fn deserialize_variant_bounded_alloc<R: Read>(
        reader: &mut R,
        tag: u8,
        budget: &mut Budget,
    ) -> Result<Self>;
}

/// Bounded decoding: the vector and `extra(n)` are charged before any element is read.
struct Charged<'a> {
    budget: &'a mut Budget,
    extra: fn(usize) -> Option<usize>,
}

impl<T: BorshDeserializeBounded> Elements<T> for Charged<'_> {
    const ENCODES_TO_ZERO_BYTES: bool = T::ENCODES_TO_ZERO_BYTES;
    #[inline]
    fn start(&mut self, len: u32) -> Result<()> {
        // Only fails on 16-bit targets.
        let n = usize::try_from(len).map_err(|_| self.budget.exceeded(usize::MAX))?;
        self.budget.charge_array::<T>(n)?;
        self.budget.charge_checked((self.extra)(n))
    }
    #[inline]
    fn bulk<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<T>>> {
        T::vec_from_reader_bounded(len, reader)
    }
    #[inline]
    fn next<R: Read>(&mut self, reader: &mut R) -> Result<T> {
        T::deserialize_bounded_alloc(reader, self.budget)
    }
}

/// A collection's bounded decode: `read_seq`, charging the vector plus `extra(n)` up front.
fn decode_seq<T: BorshDeserializeBounded, R: Read>(
    reader: &mut R,
    budget: &mut Budget,
    extra: fn(usize) -> Option<usize>,
) -> Result<Vec<T>> {
    read_seq(reader, &mut Charged { budget, extra })
}

impl BorshDeserializeBounded for u8 {
    #[inline]
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, _budget: &mut Budget) -> Result<Self> {
        <u8 as BorshDeserialize>::deserialize_reader(reader)
    }

    #[inline]
    #[doc(hidden)]
    fn vec_from_reader_bounded<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>> {
        <u8 as BorshDeserialize>::vec_from_reader(len, reader)
    }
}

impl BorshDeserializeBounded for String {
    #[inline]
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        String::from_utf8(Vec::<u8>::deserialize_bounded_alloc(reader, budget)?)
            .map_err(|err| invalid(err.to_string()))
    }
}
impl<T> BorshDeserializeBounded for Vec<T>
where
    T: BorshDeserializeBounded,
{
    #[inline]
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        decode_seq(reader, budget, size::none)
    }
}

impl<T, U> BorshDeserializeBounded for Box<T>
where
    U: Into<Box<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: BorshDeserializeBounded,
{
    const ENCODES_TO_ZERO_BYTES: bool = <U as BorshDeserializeBounded>::ENCODES_TO_ZERO_BYTES;
    #[inline]
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        // Charge first so recursion pays as it goes. On stable Rust only sized types have thin
        // pointers; an unsized `T` reuses the buffer `T::Owned` charged.
        if size_of::<*const T>() == size_of::<*const ()>() {
            budget.charge_bytes(size_of::<U>())?;
        }
        let _guard = RecursionGuard::enter()?;
        Ok(T::Owned::deserialize_bounded_alloc(reader, budget)?.into())
    }
}

/// Deserializes an object from a slice of bytes, charging `budget` as described on
/// [`BorshDeserializeBounded`](crate::BorshDeserializeBounded).
///
/// Charges `size_of::<T>()` first, before reading anything, then the allocations.
///
/// # Example
/// ```
/// use borsh::{from_slice_bounded, to_vec, Budget};
/// use core::mem::size_of;
///
/// let encoded = to_vec(&vec![1u64, 2, 3]).unwrap();
/// // The `Vec` itself, plus 24 bytes for three `u64`s.
/// let mut budget = Budget::from_bytes(size_of::<Vec<u64>>() + 24);
/// let decoded = from_slice_bounded::<Vec<u64>>(&encoded, &mut budget).unwrap();
/// assert_eq!(decoded, [1, 2, 3]);
/// ```
/// # Errors
/// Returns an error if the data is invalid or not fully consumed, and an error of kind
/// `OutOfMemory` (see [`LimitExceeded`](crate::error::LimitExceeded)) if it does not fit
/// `budget`.
pub fn from_slice_bounded<T: BorshDeserializeBounded>(v: &[u8], budget: &mut Budget) -> Result<T> {
    budget.charge_bytes(size_of::<T>())?;
    let mut v_mut = v;
    let object = T::deserialize_bounded_alloc(&mut v_mut, budget)?;
    if !v_mut.is_empty() {
        return Err(invalid(ERROR_NOT_ALL_BYTES_READ));
    }
    Ok(object)
}

/// Deserializes an object from a reader, charging `budget`; see [`from_slice_bounded`].
///
/// # Example
/// ```
/// use borsh::{from_reader_bounded, to_vec, Budget};
/// use core::mem::size_of;
///
/// let encoded = to_vec(&String::from("hello")).unwrap();
/// // The `String` itself, plus its 5 bytes.
/// let mut budget = Budget::from_bytes(size_of::<String>() + 5);
/// let decoded = from_reader_bounded::<_, String>(&mut encoded.as_slice(), &mut budget).unwrap();
/// assert_eq!(decoded, "hello");
/// ```
pub fn from_reader_bounded<R: Read, T: BorshDeserializeBounded>(
    reader: &mut R,
    budget: &mut Budget,
) -> Result<T> {
    budget.charge_bytes(size_of::<T>())?;
    let result = T::deserialize_bounded_alloc(reader, budget)?;
    expect_end(reader)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::ErrorKind;

    #[track_caller]
    pub(super) fn assert_exceeded(
        result: Result<()>,
        limit: usize,
        requested: usize,
        remaining: usize,
    ) {
        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::OutOfMemory);
        let expected = LimitExceeded {
            limit,
            requested,
            remaining,
        };
        #[cfg(feature = "std")]
        assert_eq!(LimitExceeded::from_io_error(&err), Some(&expected));
        #[cfg(not(feature = "std"))]
        assert_eq!(err.to_string(), expected.to_string());
    }

    #[test]
    fn entry_points_charge_the_budget_they_are_given() {
        let input = [3, 0, 0, 0, 1, 2, 3, 0];
        let (exact, trailing) = (&input[..7], &input[..]);
        // Every call charges `root` first.
        let root = size_of::<Vec<u8>>();
        let limit = 20 + 5 * root;
        let mut budget = Budget::from_bytes(limit);
        assert_eq!(
            from_slice_bounded::<Vec<u8>>(exact, &mut budget).unwrap(),
            [1, 2, 3]
        );
        assert_eq!(
            from_reader_bounded::<_, Vec<u8>>(&mut &exact[..], &mut budget).unwrap(),
            [1, 2, 3]
        );
        assert_eq!(budget.used_bytes(), 2 * (root + 3));
        // Each call still rejects trailing bytes, and keeps the charge made before.
        let err = from_slice_bounded::<Vec<u8>>(trailing, &mut budget).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        let err = from_reader_bounded::<_, Vec<u8>>(&mut &trailing[..], &mut budget).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert_eq!(budget.used_bytes(), 4 * (root + 3));
        let claims_100 = 100u32.to_le_bytes();
        let result = from_slice_bounded::<Vec<u8>>(&claims_100, &mut budget);
        assert_exceeded(result.map(drop), limit, 100, 8);
        assert_eq!(budget.used_bytes(), 4 * (root + 3) + root);
    }

    #[test]
    fn trait_methods_match_the_entry_points() {
        type Outcome = (core::result::Result<Vec<u8>, (ErrorKind, String)>, usize);
        fn run(decode: impl FnOnce(&mut Budget) -> Result<Vec<u8>>) -> Outcome {
            let mut budget = Budget::from_bytes(size_of::<Vec<u8>>() + 10);
            let result = decode(&mut budget).map_err(|err| (err.kind(), err.to_string()));
            (result, budget.used_bytes())
        }
        let input = [3, 0, 0, 0, 1, 2, 3, 0];
        let claims_100 = 100u32.to_le_bytes();
        // Exact input, trailing bytes, and a length over the limit.
        for input in [&input[..7], &input[..], &claims_100[..]] {
            assert_eq!(
                run(|budget| Vec::<u8>::try_from_slice_bounded(input, budget)),
                run(|budget| from_slice_bounded::<Vec<u8>>(input, budget))
            );
            assert_eq!(
                run(|budget| Vec::<u8>::try_from_reader_bounded(&mut &input[..], budget)),
                run(|budget| from_reader_bounded::<_, Vec<u8>>(&mut &input[..], budget))
            );
        }
    }

    #[test]
    fn vectors_end_at_their_charged_capacity() {
        fn encoded(len: usize, element_size: usize) -> Vec<u8> {
            let mut input = Vec::from((len as u32).to_le_bytes());
            input.resize(4 + len * element_size, 7);
            input
        }
        // Past the cautious start of 512 `u64`s, and past two 1 MiB steps of bytes.
        let mut budget = Budget::from_bytes(usize::MAX);
        let numbers = from_slice_bounded::<Vec<u64>>(&encoded(1000, 8), &mut budget).unwrap();
        assert_eq!((numbers.len(), numbers.capacity()), (1000, 1000));
        assert_eq!(budget.used_bytes(), size_of::<Vec<u64>>() + 8000);

        let len = 2 * 1024 * 1024 + 1;
        let mut budget = Budget::from_bytes(usize::MAX);
        let bytes = from_slice_bounded::<Vec<u8>>(&encoded(len, 1), &mut budget).unwrap();
        assert_eq!((bytes.len(), bytes.capacity()), (len, len));
        assert_eq!(budget.used_bytes(), size_of::<Vec<u8>>() + len);
    }

    #[test]
    fn root_charge_fails_before_anything_is_read() {
        let root = size_of::<Vec<u8>>();
        let input = [3, 0, 0, 0, 1, 2, 3];
        let mut reader = &input[..];
        let result =
            from_reader_bounded::<_, Vec<u8>>(&mut reader, &mut Budget::from_bytes(root - 1));
        assert_exceeded(result.map(drop), root - 1, root, root - 1);
        assert_eq!(reader.len(), input.len());
        // The limit error wins over malformed input.
        let result = from_slice_bounded::<Vec<u8>>(&[], &mut Budget::from_bytes(root - 1));
        assert_exceeded(result.map(drop), root - 1, root, root - 1);
    }
}
