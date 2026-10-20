//! Bounded decoding preserves the plain decoder's check order and passes one budget through
//! nested values, charging before allocation.

use core::alloc::Layout;
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
    fn deserialize_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self>;

    /// Reads `len` elements in one go. The caller has already charged the vector; the hook
    /// only allocates and reads. Returning `None` must leave the reader untouched.
    #[inline]
    #[doc(hidden)]
    fn vec_from_reader_bounded<R: Read>(
        len: u32,
        reader: &mut R,
        budget: &mut Budget,
    ) -> Result<Option<Vec<Self>>> {
        let _ = (len, reader, budget);
        Ok(None)
    }

    #[inline]
    #[doc(hidden)]
    fn array_from_reader_bounded<R: Read, const N: usize>(
        reader: &mut R,
        budget: &mut Budget,
    ) -> Result<Option<[Self; N]>> {
        let _ = (reader, budget);
        Ok(None)
    }
}

/// Additional methods offered on enums, the bounded twin of [`EnumExt`](crate::de::EnumExt).
pub trait EnumExtBounded: BorshDeserializeBounded {
    /// Deserializes the given variant, charging `budget` like
    /// [`deserialize_bounded`](BorshDeserializeBounded::deserialize_bounded).
    ///
    /// Allows validation or filtering of the variant before decoding its fields.
    ///
    /// ```
    /// use borsh::de::{Budget, EnumExtBounded as _};
    /// use borsh::BorshDeserializeBounded;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// enum MyEnum {
    ///     Zero,
    ///     One(u8),
    ///     Many(Vec<u8>)
    /// }
    ///
    /// // The two impls an enum needs to dispatch on a tag read ahead.
    /// impl BorshDeserializeBounded for MyEnum {
    ///     fn deserialize_bounded<R: borsh::io::Read>(
    ///         reader: &mut R,
    ///         budget: &mut Budget,
    ///     ) -> borsh::io::Result<Self> {
    ///         use borsh::de::EnumExtBounded;
    ///         let tag = u8::deserialize_bounded(reader, budget)?;
    ///         Self::deserialize_variant_bounded(reader, tag, budget)
    ///     }
    /// }
    ///
    /// impl borsh::de::EnumExtBounded for MyEnum {
    ///     fn deserialize_variant_bounded<R: borsh::io::Read>(
    ///         reader: &mut R,
    ///         tag: u8,
    ///         budget: &mut Budget,
    ///     ) -> borsh::io::Result<Self> {
    ///         match tag {
    ///             0 => Ok(MyEnum::Zero),
    ///             1 => Ok(MyEnum::One(u8::deserialize_bounded(reader, budget)?)),
    ///             2 => Ok(MyEnum::Many(Vec::deserialize_bounded(reader, budget)?)),
    ///             _ => Err(borsh::io::Error::new(
    ///                 borsh::io::ErrorKind::InvalidData,
    ///                 format!("Unexpected variant tag: {:?}", tag),
    ///             )),
    ///         }
    ///     }
    /// }
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct OneOrZero(MyEnum);
    ///
    /// impl BorshDeserializeBounded for OneOrZero {
    ///     fn deserialize_bounded<R: borsh::io::Read>(
    ///         reader: &mut R,
    ///         budget: &mut Budget,
    ///     ) -> borsh::io::Result<Self> {
    ///         use borsh::de::EnumExtBounded;
    ///         let tag = u8::deserialize_bounded(reader, budget)?;
    ///         if tag == 2 {
    ///             Err(borsh::io::Error::new(
    ///                 borsh::io::ErrorKind::InvalidData,
    ///                 "MyEnum::Many not allowed here",
    ///             ))
    ///         } else {
    ///             MyEnum::deserialize_variant_bounded(reader, tag, budget).map(Self)
    ///         }
    ///     }
    /// }
    ///
    /// use borsh::from_slice_bounded;
    /// let data = b"\0";
    /// assert_eq!(
    ///     MyEnum::Zero,
    ///     from_slice_bounded::<MyEnum>(&data[..], &mut Budget::new(0)).unwrap()
    /// );
    /// assert_eq!(
    ///     MyEnum::Zero,
    ///     from_slice_bounded::<OneOrZero>(&data[..], &mut Budget::new(0)).unwrap().0
    /// );
    ///
    /// let data = b"\x02\0\0\0\0";
    /// assert_eq!(
    ///     MyEnum::Many(Vec::new()),
    ///     from_slice_bounded::<MyEnum>(&data[..], &mut Budget::new(0)).unwrap()
    /// );
    /// assert!(from_slice_bounded::<OneOrZero>(&data[..], &mut Budget::new(0)).is_err());
    /// ```
    fn deserialize_variant_bounded<R: Read>(
        reader: &mut R,
        tag: u8,
        budget: &mut Budget,
    ) -> Result<Self>;
}

/// Charges the vector and `extra(n)` together before decoding any elements.
/// `extra(n)` covers allocations made when converting the vector into a collection;
/// `None` means overflow or an invalid allocation size.
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
    let n = match usize::try_from(len) {
        Ok(n) => n,
        // 16-bit targets only. Charged as an overflow, which always fails.
        Err(_) => return budget.charge_checked(None).map(|()| Vec::new()),
    };
    budget.charge_checked(
        Layout::array::<T>(n)
            .ok()
            .and_then(|vec| vec.size().checked_add(extra(n)?)),
    )?;
    if let Some(vec) = T::vec_from_reader_bounded(len, reader, budget)? {
        Ok(vec)
    } else {
        let mut result = Vec::with_capacity(n);
        for _ in 0..len {
            result.push(T::deserialize_bounded(reader, budget)?);
        }
        Ok(result)
    }
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
    fn vec_from_reader_bounded<R: Read>(
        len: u32,
        reader: &mut R,
        _budget: &mut Budget,
    ) -> Result<Option<Vec<Self>>> {
        let len: usize = len.try_into().map_err(|_| ErrorKind::InvalidData)?;
        // Preallocate the charged size to avoid reallocations, but preserve the plain
        // decoder's read windows for readers that depend on the requested buffer size.
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

    #[inline]
    #[doc(hidden)]
    fn array_from_reader_bounded<R: Read, const N: usize>(
        reader: &mut R,
        _budget: &mut Budget,
    ) -> Result<Option<[Self; N]>> {
        <u8 as BorshDeserialize>::array_from_reader(reader)
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
        // On stable Rust only sized types have thin pointers. The box of an unsized value
        // reuses the buffer `owned` already charged for.
        if size_of::<*const T>() == size_of::<*const ()>() {
            budget.charge(size_of_val(<U as Borrow<T>>::borrow(&owned)))?;
        }
        Ok(owned.into())
    }
}

/// Deserializes an object from a slice of bytes, failing with
/// [`LimitExceeded`](crate::error::LimitExceeded) when decoding would charge more than is
/// left in `budget`.
///
/// The common call is `from_slice_bounded::<T>(&bytes, &mut Budget::new(limit))`. One budget
/// can be passed to several calls to share one limit across several values, each still
/// checked for trailing bytes. On error, the budget keeps the charges made before the error.
/// Afterwards, [`budget.used()`](Budget::used) tells how much has been charged.
///
/// What is left in the budget is also the largest buffer a single false length prefix can
/// make the decoder allocate before the input runs out, so pick a limit the process can
/// actually provide. What is charged is documented on
/// [`BorshDeserializeBounded`](crate::BorshDeserializeBounded).
///
/// # Example
/// ```
/// use borsh::de::Budget;
/// use borsh::{from_slice_bounded, to_vec};
///
/// let numbers = to_vec(&vec![1u64, 2, 3]).unwrap();
/// let text = to_vec(&String::from("abc")).unwrap();
/// // One limit for both values: three `u64`s need 24 bytes, the string 3.
/// let mut budget = Budget::new(30);
/// assert_eq!(from_slice_bounded::<Vec<u64>>(&numbers, &mut budget).unwrap(), vec![1, 2, 3]);
/// assert_eq!(from_slice_bounded::<String>(&text, &mut budget).unwrap(), "abc");
/// assert_eq!(budget.used(), 27);
/// // The 3 bytes left are too few for the numbers again.
/// let err = from_slice_bounded::<Vec<u64>>(&numbers, &mut budget).unwrap_err();
/// assert_eq!(err.kind(), borsh::io::ErrorKind::OutOfMemory);
/// assert_eq!(budget.used(), 27);
/// ```
/// # Errors
/// Returns an error if the data is invalid, does not fit the budget, or is not fully consumed.
pub fn from_slice_bounded<T: BorshDeserializeBounded>(v: &[u8], budget: &mut Budget) -> Result<T> {
    let mut v_mut = v;
    let object = T::deserialize_bounded(&mut v_mut, budget)?;
    if !v_mut.is_empty() {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ));
    }
    Ok(object)
}

/// Deserializes an object from a reader, failing with
/// [`LimitExceeded`](crate::error::LimitExceeded) when decoding would charge more than is
/// left in `budget`. See [`from_slice_bounded`].
///
/// # Example
/// ```
/// use borsh::de::Budget;
/// use borsh::{from_reader_bounded, to_vec};
///
/// let encoded = to_vec(&String::from("hello")).unwrap();
/// let decoded =
///     from_reader_bounded::<_, String>(&mut encoded.as_slice(), &mut Budget::new(5)).unwrap();
/// assert_eq!(decoded, "hello");
/// ```
pub fn from_reader_bounded<R: Read, T: BorshDeserializeBounded>(
    reader: &mut R,
    budget: &mut Budget,
) -> Result<T> {
    let result = T::deserialize_bounded(reader, budget)?;
    let mut buf = [0u8; 1];
    match reader.read_exact(&mut buf) {
        Err(f) if f.kind() == ErrorKind::UnexpectedEof => Ok(result),
        _ => Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ)),
    }
}
