//! Each impl here is its plain twin from `de/mod.rs` plus a charge before each allocation,
//! with the same reads, checks and constructors. Keep the twins in sync;
//! `assert_bounded_parity` in the tests catches drift.

use core::convert::{TryFrom, TryInto};
use core::mem::{size_of, size_of_val};

use crate::__private::maybestd::{
    borrow::{Borrow, ToOwned},
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use crate::error::check_zst;
use crate::io::{Error, ErrorKind, Read, Result};

use super::{BorshDeserialize, ERROR_NOT_ALL_BYTES_READ, ERROR_UNEXPECTED_LENGTH_OF_INPUT};

mod budget;
mod size;

pub use budget::{Budget, LimitExceeded};

#[doc = include_str!("../../../docs/rustdoc_include/borsh_bounded_charges.md")]
pub trait BorshDeserializeBounded: Sized {
    /// Deserializes this instance from a reader, charging `budget` before every allocation.
    /// `size_of::<Self>()` is charged by the entry points, or by the allocation that holds it.
    fn deserialize_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self>;

    /// Reads `len` elements in one go into a vector the caller has already charged for.
    /// `None` must leave the reader untouched.
    #[inline]
    #[doc(hidden)]
    fn vec_from_reader_bounded<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>> {
        let _ = (len, reader);
        Ok(None)
    }
}

/// Bounded counterpart of [`EnumExt`](crate::de::EnumExt).
pub trait EnumExtBounded: BorshDeserializeBounded {
    /// Decodes the variant for an already read `tag`, charging `budget` like
    /// [`deserialize_bounded`](BorshDeserializeBounded::deserialize_bounded). Like
    /// [`EnumExt::deserialize_variant`](crate::de::EnumExt::deserialize_variant), it lets an
    /// impl check the tag before the fields are decoded.
    fn deserialize_variant_bounded<R: Read>(
        reader: &mut R,
        tag: u8,
        budget: &mut Budget,
    ) -> Result<Self>;
}

/// Charges the vector plus `extra(n)` before reading any element.
fn decode_seq<T, R>(
    reader: &mut R,
    budget: &mut Budget,
    extra: impl FnOnce(usize) -> Option<usize>,
) -> Result<Vec<T>>
where
    T: BorshDeserializeBounded,
    R: Read,
{
    check_zst::<T>()?;

    let len = u32::deserialize_bounded(reader, budget)?;
    if len == 0 {
        return Ok(Vec::new());
    }
    // Only fails on 16-bit targets, where the charge below then fails as an overflow.
    let n = usize::try_from(len).unwrap_or(usize::MAX);
    budget.charge_array::<T>(n)?;
    budget.charge_checked(extra(n))?;
    if let Some(vec) = T::vec_from_reader_bounded(len, reader)? {
        return Ok(vec);
    }
    let mut result = Vec::with_capacity(n);
    for _ in 0..len {
        result.push(T::deserialize_bounded(reader, budget)?);
    }
    Ok(result)
}

/// Only for types whose plain decoder never allocates.
macro_rules! impl_bounded_via_plain {
    ($($type:ty),+ $(,)?) => {
        $(
            impl $crate::de::BorshDeserializeBounded for $type {
                #[inline]
                fn deserialize_bounded<R: $crate::io::Read>(
                    reader: &mut R,
                    _budget: &mut $crate::de::Budget,
                ) -> $crate::io::Result<Self> {
                    <$type as $crate::de::BorshDeserialize>::deserialize_reader(reader)
                }
            }
        )+
    };
}

impl_bounded_via_plain!(
    i8,
    i16,
    i32,
    i64,
    i128,
    u16,
    u32,
    u64,
    u128,
    isize,
    usize,
    f32,
    f64,
    bool,
    core::num::NonZeroI8,
    core::num::NonZeroI16,
    core::num::NonZeroI32,
    core::num::NonZeroI64,
    core::num::NonZeroI128,
    core::num::NonZeroU8,
    core::num::NonZeroU16,
    core::num::NonZeroU32,
    core::num::NonZeroU64,
    core::num::NonZeroU128,
    core::num::NonZeroUsize,
    (),
    core::ops::RangeFull,
    core::net::SocketAddr,
    core::net::IpAddr,
    core::net::SocketAddrV4,
    core::net::SocketAddrV6,
    core::net::Ipv4Addr,
    core::net::Ipv6Addr,
);

impl BorshDeserializeBounded for u8 {
    #[inline]
    fn deserialize_bounded<R: Read>(reader: &mut R, _budget: &mut Budget) -> Result<Self> {
        <u8 as BorshDeserialize>::deserialize_reader(reader)
    }

    #[inline]
    #[doc(hidden)]
    fn vec_from_reader_bounded<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>> {
        let len: usize = len.try_into().map_err(|_| ErrorKind::InvalidData)?;
        // Same reads as `<u8 as BorshDeserialize>::vec_from_reader`, but the vector has the
        // charged capacity from the start.
        let mut vec = Vec::with_capacity(len);
        vec.resize(len.min(1024 * 1024), 0);
        let mut pos = 0;
        while pos < len {
            if pos == vec.len() {
                vec.resize(vec.len().saturating_mul(2).min(len), 0)
            }
            match reader.read(&mut vec.as_mut_slice()[pos..])? {
                0 => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        ERROR_UNEXPECTED_LENGTH_OF_INPUT,
                    ))
                }
                read => {
                    pos += read;
                }
            }
        }
        Ok(Some(vec))
    }
}

impl BorshDeserializeBounded for String {
    #[inline]
    fn deserialize_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        String::from_utf8(Vec::<u8>::deserialize_bounded(reader, budget)?).map_err(|err| {
            let msg = err.to_string();
            Error::new(ErrorKind::InvalidData, msg)
        })
    }
}
impl<T> BorshDeserializeBounded for Vec<T>
where
    T: BorshDeserializeBounded,
{
    #[inline]
    fn deserialize_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        decode_seq(reader, budget, size::none)
    }
}

impl<T, U> BorshDeserializeBounded for Box<T>
where
    U: Into<Box<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: BorshDeserializeBounded,
{
    fn deserialize_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let owned = T::Owned::deserialize_bounded(reader, budget)?;
        // Charges the box of a sized `T`; the box of an unsized one reuses the buffer `owned`
        // was charged for. On stable Rust only sized types have thin pointers.
        if size_of::<*const T>() == size_of::<*const ()>() {
            budget.charge(size_of_val(<U as Borrow<T>>::borrow(&owned)))?;
        }
        Ok(owned.into())
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
/// let mut budget = Budget::new(size_of::<Vec<u64>>() + 24);
/// let decoded = from_slice_bounded::<Vec<u64>>(&encoded, &mut budget).unwrap();
/// assert_eq!(decoded, [1, 2, 3]);
/// ```
/// # Errors
/// Returns an error if the data is invalid or not fully consumed, and an error of kind
/// `OutOfMemory` (see [`LimitExceeded`](crate::error::LimitExceeded)) if it does not fit
/// `budget`.
pub fn from_slice_bounded<T: BorshDeserializeBounded>(v: &[u8], budget: &mut Budget) -> Result<T> {
    budget.charge(size_of::<T>())?;
    let mut v_mut = v;
    let object = T::deserialize_bounded(&mut v_mut, budget)?;
    if !v_mut.is_empty() {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ));
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
/// let mut budget = Budget::new(size_of::<String>() + 5);
/// let decoded = from_reader_bounded::<_, String>(&mut encoded.as_slice(), &mut budget).unwrap();
/// assert_eq!(decoded, "hello");
/// ```
pub fn from_reader_bounded<R: Read, T: BorshDeserializeBounded>(
    reader: &mut R,
    budget: &mut Budget,
) -> Result<T> {
    budget.charge(size_of::<T>())?;
    let result = T::deserialize_bounded(reader, budget)?;
    let mut buf = [0u8; 1];
    match reader.read_exact(&mut buf) {
        Err(f) if f.kind() == ErrorKind::UnexpectedEof => Ok(result),
        _ => Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            err.get_ref()
                .and_then(|inner| inner.downcast_ref::<LimitExceeded>()),
            Some(&expected)
        );
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
        let mut budget = Budget::new(limit);
        assert_eq!(
            from_slice_bounded::<Vec<u8>>(exact, &mut budget).unwrap(),
            [1, 2, 3]
        );
        assert_eq!(
            from_reader_bounded::<_, Vec<u8>>(&mut &exact[..], &mut budget).unwrap(),
            [1, 2, 3]
        );
        assert_eq!(budget.used(), 2 * (root + 3));
        // Each call still rejects trailing bytes, and keeps the charge made before.
        let err = from_slice_bounded::<Vec<u8>>(trailing, &mut budget).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        let err = from_reader_bounded::<_, Vec<u8>>(&mut &trailing[..], &mut budget).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert_eq!(budget.used(), 4 * (root + 3));
        let claims_100 = 100u32.to_le_bytes();
        let result = from_slice_bounded::<Vec<u8>>(&claims_100, &mut budget);
        assert_exceeded(result.map(drop), limit, 100, 8);
        assert_eq!(budget.used(), 4 * (root + 3) + root);
    }

    #[test]
    fn root_charge_fails_before_anything_is_read() {
        let root = size_of::<Vec<u8>>();
        let input = [3, 0, 0, 0, 1, 2, 3];
        let mut reader = &input[..];
        let result = from_reader_bounded::<_, Vec<u8>>(&mut reader, &mut Budget::new(root - 1));
        assert_exceeded(result.map(drop), root - 1, root, root - 1);
        assert_eq!(reader.len(), input.len());
        // The limit error wins over malformed input.
        let result = from_slice_bounded::<Vec<u8>>(&[], &mut Budget::new(root - 1));
        assert_exceeded(result.map(drop), root - 1, root, root - 1);
    }
}
