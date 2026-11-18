//! `BorshDeserialize` and its impls for std types.
use core::convert::{TryFrom, TryInto};
use core::iter::FromIterator;
use core::marker::PhantomData;
use core::mem::{size_of, MaybeUninit};

use crate::__private::encodes_to_zero_bytes_de as zero_bytes;
use crate::__private::maybestd::{
    borrow::{Borrow, Cow, ToOwned},
    boxed::Box,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use crate::io::{Error, ErrorKind, Read, Result};

#[cfg(feature = "unstable__bounded")]
pub(crate) mod bounded;
#[cfg(feature = "unstable__bounded")]
pub use bounded::{
    from_reader_bounded, from_slice_bounded, BorshDeserializeBounded, Budget, EnumExtBounded,
};

const ERROR_NOT_ALL_BYTES_READ: &str = "Not all bytes read";
const ERROR_UNEXPECTED_LENGTH_OF_INPUT: &str = "Unexpected length of input";

/// A data-structure that can be de-serialized from binary format by NBOR.
pub trait BorshDeserialize: Sized {
    /// Deserializes this instance from a given slice of bytes, advancing it past them.
    ///
    /// If this returns an error, the position of `buf` is unspecified: it
    /// differs between `std` and `no_std` builds and between Rust versions.
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        Self::deserialize_reader(&mut *buf)
    }

    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self>;

    /// Deserialize this instance from a slice of bytes, rejecting trailing bytes.
    fn try_from_slice(v: &[u8]) -> Result<Self> {
        from_slice(v)
    }

    fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        from_reader(reader)
    }

    #[doc(hidden)]
    fn vec_from_reader<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>> {
        let _ = (len, reader);
        Ok(None)
    }

    #[doc(hidden)]
    fn array_from_reader<R: Read, const N: usize>(reader: &mut R) -> Result<Option<[Self; N]>> {
        let _ = reader;
        Ok(None)
    }

    /// Whether every value encodes to zero bytes. Collections reject such element types.
    #[doc(hidden)]
    const ENCODES_TO_ZERO_BYTES: bool = false;
}

/// Additional methods offered on enums which is used by `[derive(BorshDeserialize)]`.
pub trait EnumExt: BorshDeserialize {
    /// Deserialises given variant of an enum from the reader.
    ///
    /// This may be used to perform validation or filtering based on what
    /// variant is being deserialised.
    ///
    /// ```
    /// use borsh::BorshDeserialize;
    /// use borsh::de::EnumExt as _;
    ///
    /// /// derive is only available if borsh is built with `features = ["derive"]`
    /// # #[cfg(feature = "derive")]
    /// #[derive(Debug, PartialEq, Eq, BorshDeserialize)]
    /// enum MyEnum {
    ///     Zero,
    ///     One(u8),
    ///     Many(Vec<u8>)
    /// }
    ///
    /// # #[cfg(feature = "derive")]
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct OneOrZero(MyEnum);
    ///
    /// # #[cfg(feature = "derive")]
    /// impl borsh::de::BorshDeserialize for OneOrZero {
    ///     fn deserialize_reader<R: borsh::io::Read>(
    ///         reader: &mut R,
    ///     ) -> borsh::io::Result<Self> {
    ///         use borsh::de::EnumExt;
    ///         let tag = u8::deserialize_reader(reader)?;
    ///         if tag == 2 {
    ///             Err(borsh::io::Error::new(
    ///                 borsh::io::ErrorKind::InvalidData,
    ///                 "MyEnum::Many not allowed here",
    ///             ))
    ///         } else {
    ///             MyEnum::deserialize_variant(reader, tag).map(Self)
    ///         }
    ///     }
    /// }
    ///
    /// use borsh::from_slice;
    /// let data = b"\0";
    /// # #[cfg(feature = "derive")]
    /// assert_eq!(MyEnum::Zero, from_slice::<MyEnum>(&data[..]).unwrap());
    /// # #[cfg(feature = "derive")]
    /// assert_eq!(MyEnum::Zero, from_slice::<OneOrZero>(&data[..]).unwrap().0);
    ///
    /// let data = b"\x02\0\0\0\0";
    /// # #[cfg(feature = "derive")]
    /// assert_eq!(MyEnum::Many(Vec::new()), from_slice::<MyEnum>(&data[..]).unwrap());
    /// # #[cfg(feature = "derive")]
    /// assert!(from_slice::<OneOrZero>(&data[..]).is_err());
    /// ```
    fn deserialize_variant<R: Read>(reader: &mut R, tag: u8) -> Result<Self>;
}

fn invalid<M: Into<String>>(msg: M) -> Error {
    Error::new(ErrorKind::InvalidData, msg.into())
}

fn eof_to_invalid(e: Error) -> Error {
    if e.kind() == ErrorKind::UnexpectedEof {
        invalid(ERROR_UNEXPECTED_LENGTH_OF_INPUT)
    } else {
        e
    }
}

fn bytes<R: Read, const N: usize>(reader: &mut R) -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    reader.read_exact(&mut buf).map_err(eof_to_invalid)?;
    Ok(buf)
}

/// Preallocate at most 4 KiB (and at least one element) from an untrusted length.
fn cautious<T>(hint: u32) -> usize {
    core::cmp::max(core::cmp::min(hint as usize, 4096 / size_of::<T>()), 1)
}

/// Maps and sets decode as a `Vec` of entries. A key type that encodes to zero bytes is
/// rejected before anything is read, and under `de_strict_order` the keys must be strictly
/// ascending: sorted, no duplicates.
fn sorted<R, K, T, C>(reader: &mut R, key: impl Fn(&T) -> &K) -> Result<C>
where
    R: Read,
    K: BorshDeserialize + Ord,
    T: BorshDeserialize,
    C: FromIterator<T>,
{
    crate::error::check_zst::<K>(K::ENCODES_TO_ZERO_BYTES)?;
    let vec = Vec::<T>::deserialize_reader(reader)?;
    if cfg!(feature = "de_strict_order")
        && vec.windows(2).any(|w| !key(&w[0]).cmp(key(&w[1])).is_lt())
    {
        return Err(invalid("keys were not serialized in ascending order"));
    }
    Ok(vec.into_iter().collect())
}

/// Under `de_strict_order`, collecting into a map or set must not drop duplicates.
#[cfg(feature = "indexmap")]
fn check_len(len: usize, collected: usize, what: &str) -> Result<()> {
    if cfg!(feature = "de_strict_order") && len != collected {
        return Err(invalid(format!(
            "duplicate {} were encountered during deserialization",
            what
        )));
    }
    Ok(())
}

impl BorshDeserialize for u8 {
    #[inline]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(bytes::<_, 1>(reader)?[0])
    }

    /// Bulk read: start at up to 1 MiB and double only as bytes arrive.
    #[doc(hidden)]
    fn vec_from_reader<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>> {
        let len: usize = len.try_into().map_err(|_| ErrorKind::InvalidData)?;
        let mut vec = vec![0u8; len.min(1024 * 1024)];
        let mut pos = 0;
        while pos < len {
            if pos == vec.len() {
                let new_len = vec.len().saturating_mul(2).min(len);
                vec.reserve_exact(new_len - vec.len());
                vec.resize(new_len, 0)
            }
            match reader.read(&mut vec[pos..]) {
                Ok(0) => return Err(invalid(ERROR_UNEXPECTED_LENGTH_OF_INPUT)),
                Ok(read) => pos += read,
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(Some(vec))
    }

    #[doc(hidden)]
    fn array_from_reader<R: Read, const N: usize>(reader: &mut R) -> Result<Option<[Self; N]>> {
        bytes(reader).map(Some)
    }
}

/// The bounded twin of a type whose plain decode never allocates: the same decode, with
/// nothing to charge.
macro_rules! bounded_as_plain {
    ($($t:ty),*) => {$(
        #[cfg(feature = "unstable__bounded")]
        impl bounded::BorshDeserializeBounded for $t {
            const ENCODES_TO_ZERO_BYTES: bool = <$t as BorshDeserialize>::ENCODES_TO_ZERO_BYTES;
            #[inline]
            fn deserialize_bounded_alloc<R: Read>(
                reader: &mut R,
                _budget: &mut bounded::Budget,
            ) -> Result<Self> {
                <$t as BorshDeserialize>::deserialize_reader(reader)
            }
        }
    )*};
}
bounded_as_plain!(
    bool,
    core::net::SocketAddrV4,
    core::net::SocketAddrV6,
    core::net::Ipv4Addr,
    core::net::Ipv6Addr
);

macro_rules! int {
    ($($t:ident)*) => {$(
        bounded_as_plain!($t);
        impl BorshDeserialize for $t {
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                Ok($t::from_le_bytes(bytes(reader)?))
            }
        }
    )*};
}
int!(i8 i16 i32 i64 i128 u16 u32 u64 u128);

macro_rules! nonzero {
    ($($t:ident)*) => {$(
        bounded_as_plain!(core::num::$t);
        impl BorshDeserialize for core::num::$t {
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                Self::new(BorshDeserialize::deserialize_reader(reader)?)
                    .ok_or_else(|| invalid("Expected a non-zero value"))
            }
        }
    )*};
}
nonzero! {
    NonZeroI8 NonZeroI16 NonZeroI32 NonZeroI64 NonZeroI128
    NonZeroU8 NonZeroU16 NonZeroU32 NonZeroU64 NonZeroU128 NonZeroUsize
}

/// `isize`/`usize` travel as 64 bits and must fit the machine's width.
macro_rules! size {
    ($($t:ident $wire:ident $bits:literal,)*) => {$(
        bounded_as_plain!($t);
        impl BorshDeserialize for $t {
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                let v: $wire = BorshDeserialize::deserialize_reader(reader)?;
                let msg = concat!("Overflow on machine with 32 bit ", $bits);
                $t::try_from(v).map_err(|_| invalid(msg))
            }
        }
    )*};
}
size!(isize i64 "isize", usize u64 "usize",);

macro_rules! float {
    ($($t:ident $bits:ident)*) => {$(
        bounded_as_plain!($t);
        impl BorshDeserialize for $t {
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                let v = $t::from_bits($bits::from_le_bytes(bytes(reader)?));
                if v.is_nan() {
                    let msg = "For portability reasons we do not allow to deserialize NaNs.";
                    return Err(invalid(msg));
                }
                Ok(v)
            }
        }
    )*};
}
float!(f32 u32 f64 u64);

impl BorshDeserialize for bool {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        match u8::deserialize_reader(reader)? {
            0 => Ok(false),
            1 => Ok(true),
            b => Err(invalid(format!("Invalid bool representation: {}", b))),
        }
    }
}

impl<T: BorshDeserialize> BorshDeserialize for Option<T> {
    #[inline]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        match u8::deserialize_reader(reader)? {
            0 => Ok(None),
            1 => Ok(Some(T::deserialize_reader(reader)?)),
            flag => Err(invalid(format!(
                "Invalid Option representation: {}. The first byte must be 0 or 1",
                flag
            ))),
        }
    }
}

impl<T: BorshDeserialize, E: BorshDeserialize> BorshDeserialize for core::result::Result<T, E> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        match u8::deserialize_reader(reader)? {
            0 => Ok(Err(E::deserialize_reader(reader)?)),
            1 => Ok(Ok(T::deserialize_reader(reader)?)),
            flag => Err(invalid(format!(
                "Invalid Result representation: {}. The first byte must be 0 or 1",
                flag
            ))),
        }
    }
}

impl BorshDeserialize for String {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        String::from_utf8(Vec::<u8>::deserialize_reader(reader)?)
            .map_err(|err| invalid(err.to_string()))
    }
}

#[cfg(feature = "ascii")]
pub mod ascii {
    use super::*;

    impl BorshDeserialize for ::ascii::AsciiString {
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            ::ascii::AsciiString::from_ascii(Vec::<u8>::deserialize_reader(reader)?)
                .map_err(|err| invalid(err.to_string()))
        }
    }

    impl BorshDeserialize for ::ascii::AsciiChar {
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            ::ascii::AsciiChar::from_ascii(u8::deserialize_reader(reader)?)
                .map_err(|err| invalid(err.to_string()))
        }
    }
}

/// How [`read_seq`] decodes elements: [`Plain`], or charged to a budget in bounded decoding.
trait Elements<T> {
    /// Whether every element encodes to zero bytes; such collections are rejected.
    const ENCODES_TO_ZERO_BYTES: bool;
    /// Runs once the length is known, before any element is read.
    fn start(&mut self, len: u32) -> Result<()>;
    /// Reads all `len` elements in one go, or `None` (leaving the reader untouched).
    fn bulk<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<T>>>;
    fn next<R: Read>(&mut self, reader: &mut R) -> Result<T>;
}

/// Plain decoding: nothing to charge.
struct Plain;

impl<T: BorshDeserialize> Elements<T> for Plain {
    const ENCODES_TO_ZERO_BYTES: bool = T::ENCODES_TO_ZERO_BYTES;
    #[inline]
    fn start(&mut self, _len: u32) -> Result<()> {
        Ok(())
    }
    #[inline]
    fn bulk<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<T>>> {
        T::vec_from_reader(len, reader)
    }
    #[inline]
    fn next<R: Read>(&mut self, reader: &mut R) -> Result<T> {
        T::deserialize_reader(reader)
    }
}

/// Every length-prefixed collection decodes through here, plain and bounded alike. The element
/// type must not encode to zero bytes, and a nonempty collection is one level of the recursion
/// limit. The vector starts small and grows as elements arrive, never past the length, so a
/// false length costs little.
#[inline]
fn read_seq<T, E: Elements<T>, R: Read>(reader: &mut R, elements: &mut E) -> Result<Vec<T>> {
    crate::error::check_zst::<T>(E::ENCODES_TO_ZERO_BYTES)?;
    let len = u32::deserialize_reader(reader)?;
    if len == 0 {
        return Ok(Vec::new());
    }
    let _guard = RecursionGuard::enter()?;
    elements.start(len)?;
    if let Some(vec) = E::bulk(len, reader)? {
        return Ok(vec);
    }
    let n = usize::try_from(len).unwrap_or(usize::MAX);
    let mut result = Vec::with_capacity(cautious::<T>(len));
    for _ in 0..len {
        let value = elements.next(reader)?;
        // Double, but never past the length: a decoded `Vec` ends with capacity == len, which
        // bounded decoding's charges rely on.
        if result.len() == result.capacity() {
            result.reserve_exact(result.capacity().min(n - result.len()));
        }
        result.push(value);
    }
    Ok(result)
}

impl<T: BorshDeserialize> BorshDeserialize for Vec<T> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        read_seq(reader, &mut Plain)
    }
}

#[cfg(feature = "bytes")]
impl BorshDeserialize for bytes::Bytes {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Vec::<u8>::deserialize_reader(reader)?.into())
    }
}

#[cfg(feature = "bytes")]
impl BorshDeserialize for bytes::BytesMut {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        // Its own loop, not `Vec<u8>`: that path's bulk read reserves up to 1 MiB before any
        // byte arrives, and the result would then be copied.
        use bytes::BufMut;
        let len = u32::deserialize_reader(reader)?;
        let mut out = bytes::BytesMut::with_capacity(cautious::<u8>(len));
        for _ in 0..len {
            out.put_u8(u8::deserialize_reader(reader)?);
        }
        Ok(out)
    }
}

#[cfg(feature = "bson")]
impl BorshDeserialize for bson::oid::ObjectId {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(bson::oid::ObjectId::from_bytes(bytes(reader)?))
    }
}

/// Entries are inserted in the order they appear in the input, so the
/// insertion (iteration) order written by [`BorshSerialize`](crate::BorshSerialize)
/// is restored on deserialization.
///
/// Unlike `HashMap`/`BTreeMap`, the order of keys is **not** checked, even with the
/// **de_strict_order** feature: any order is a valid encoding, and the
/// encoding of `IndexMap` is not canonical (see its `BorshSerialize` impl).
///
/// With **de_strict_order** enabled, input containing a duplicate key is
/// rejected with [`ErrorKind::InvalidData`]. Without it, a later duplicate
/// overwrites the value of the earlier one, which keeps its original position.
#[cfg(feature = "indexmap")]
impl<K, V, S> BorshDeserialize for indexmap::IndexMap<K, V, S>
where
    K: BorshDeserialize + Eq + core::hash::Hash,
    V: BorshDeserialize,
    S: core::hash::BuildHasher + Default,
{
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        crate::error::check_zst::<K>(K::ENCODES_TO_ZERO_BYTES)?;
        let vec = Vec::<(K, V)>::deserialize_reader(reader)?;
        let len = vec.len();
        let map: Self = vec.into_iter().collect();
        check_len(len, map.len(), "keys")?;
        Ok(map)
    }
}

/// Items are inserted in the order they appear in the input, so the
/// insertion (iteration) order written by [`BorshSerialize`](crate::BorshSerialize)
/// is restored on deserialization.
///
/// Unlike `HashSet`/`BTreeSet`, the order of items is **not** checked, even with the
/// **de_strict_order** feature: any order is a valid encoding, and the
/// encoding of `IndexSet` is not canonical (see its `BorshSerialize` impl).
///
/// With **de_strict_order** enabled, input containing a duplicate item is
/// rejected with [`ErrorKind::InvalidData`]. Without it, later duplicates are
/// ignored.
#[cfg(feature = "indexmap")]
impl<T, S> BorshDeserialize for indexmap::IndexSet<T, S>
where
    T: BorshDeserialize + Eq + core::hash::Hash,
    S: core::hash::BuildHasher + Default,
{
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        crate::error::check_zst::<T>(T::ENCODES_TO_ZERO_BYTES)?;
        let vec = Vec::<T>::deserialize_reader(reader)?;
        let len = vec.len();
        let set: Self = vec.into_iter().collect();
        check_len(len, set.len(), "items")?;
        Ok(set)
    }
}

#[cfg(feature = "uuid")]
impl BorshDeserialize for uuid::Uuid {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(uuid::Uuid::from_bytes(
            BorshDeserialize::deserialize_reader(reader)?,
        ))
    }
}

impl<T> BorshDeserialize for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: BorshDeserialize,
{
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T::Owned>();
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Cow::Owned(BorshDeserialize::deserialize_reader(reader)?))
    }
}

impl<T: BorshDeserialize> BorshDeserialize for VecDeque<T> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Vec::<T>::deserialize_reader(reader)?.into())
    }
}

impl<T: BorshDeserialize> BorshDeserialize for LinkedList<T> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Vec::<T>::deserialize_reader(reader)?.into_iter().collect())
    }
}

#[cfg(any(feature = "std", feature = "hashbrown"))]
pub mod hashes {
    //! Module defines [BorshDeserialize] implementation for [HashMap]/[HashSet].
    use super::*;
    use crate::__private::maybestd::collections::{HashMap, HashSet};
    use core::hash::{BuildHasher, Hash};

    impl<T, H> BorshDeserialize for HashSet<T, H>
    where
        T: BorshDeserialize + Eq + Hash + Ord,
        H: BuildHasher + Default,
    {
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            sorted(reader, |t: &T| t)
        }
    }

    impl<K, V, H> BorshDeserialize for HashMap<K, V, H>
    where
        K: BorshDeserialize + Eq + Hash + Ord,
        V: BorshDeserialize,
        H: BuildHasher + Default,
    {
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            sorted(reader, |(k, _): &(K, V)| k)
        }
    }
}

impl<T: BorshDeserialize + Ord> BorshDeserialize for BTreeSet<T> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        sorted(reader, |t: &T| t)
    }
}

impl<K: BorshDeserialize + Ord, V: BorshDeserialize> BorshDeserialize for BTreeMap<K, V> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        sorted(reader, |(k, _): &(K, V)| k)
    }
}

/// `IpAddr` and `SocketAddr` are enums tagged 0 (v4) and 1 (v6). For `SocketAddr::V6`,
/// `flowinfo` and `scope_id` are always `0`; see the `SocketAddrV6` impl.
macro_rules! net_enum {
    ($($t:ident $v4:ident $v6:ident,)*) => {$(
        bounded_as_plain!(core::net::$t);
        impl BorshDeserialize for core::net::$t {
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                match u8::deserialize_reader(reader)? {
                    0 => Ok(Self::V4(core::net::$v4::deserialize_reader(reader)?)),
                    1 => Ok(Self::V6(core::net::$v6::deserialize_reader(reader)?)),
                    v => Err(invalid(format!(
                        concat!("Invalid ", stringify!($t), " variant: {}"),
                        v
                    ))),
                }
            }
        }
    )*};
}
net_enum!(SocketAddr SocketAddrV4 SocketAddrV6, IpAddr Ipv4Addr Ipv6Addr,);

impl BorshDeserialize for core::net::SocketAddrV4 {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let ip = core::net::Ipv4Addr::deserialize_reader(reader)?;
        Ok(Self::new(ip, u16::deserialize_reader(reader)?))
    }
}

/// Decodes the 16 address octets followed by the port as a little-endian `u16`.
///
/// The encoding does not carry [`flowinfo`](core::net::SocketAddrV6::flowinfo) or
/// [`scope_id`](core::net::SocketAddrV6::scope_id), so the returned value always has both set
/// to `0`. Round-tripping a value with either field non-zero does not reproduce the original.
impl BorshDeserialize for core::net::SocketAddrV6 {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let ip = core::net::Ipv6Addr::deserialize_reader(reader)?;
        Ok(Self::new(ip, u16::deserialize_reader(reader)?, 0, 0))
    }
}

impl BorshDeserialize for core::net::Ipv4Addr {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Self::from(bytes::<_, 4>(reader)?))
    }
}

impl BorshDeserialize for core::net::Ipv6Addr {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Self::from(bytes::<_, 16>(reader)?))
    }
}

/// Each `Box` is one level of the recursion limit. `Box`, `Rc`, `Arc`, `Option` and `u8` decode
/// are inlined: recursive types nest through them, so each saved frame is saved at every level.
impl<T, U> BorshDeserialize for Box<T>
where
    U: Into<Box<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: BorshDeserialize,
{
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T::Owned>();
    #[inline]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let _guard = RecursionGuard::enter()?;
        Ok(T::Owned::deserialize_reader(reader)?.into())
    }
}

impl<T: BorshDeserialize, const N: usize> BorshDeserialize for [T; N] {
    const ENCODES_TO_ZERO_BYTES: bool = N == 0 || zero_bytes::<T>();
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        /// Drops the initialized prefix if decoding fails or panics part way.
        struct Partial<T, const N: usize> {
            buffer: [MaybeUninit<T>; N],
            init: usize,
        }
        impl<T, const N: usize> Drop for Partial<T, N> {
            fn drop(&mut self) {
                // SAFETY: the first `init` elements are initialized.
                unsafe {
                    core::ptr::drop_in_place(&mut self.buffer[..self.init] as *mut _ as *mut [T])
                };
            }
        }
        if let Some(arr) = T::array_from_reader(reader)? {
            return Ok(arr);
        }
        // SAFETY: an array of `MaybeUninit` needs no initialization.
        let mut out = Partial::<T, N> {
            buffer: unsafe { MaybeUninit::uninit().assume_init() },
            init: 0,
        };
        while out.init < N {
            out.buffer[out.init].write(T::deserialize_reader(reader)?);
            out.init += 1;
        }
        out.init = 0;
        // SAFETY: all N elements are initialized, and `init = 0` keeps `drop` from dropping them.
        Ok(unsafe { core::ptr::read(&out.buffer as *const _ as *const [T; N]) })
    }
}

macro_rules! unit {
    ($($t:ty),*) => {$(
        bounded_as_plain!($t);
        impl BorshDeserialize for $t {
            const ENCODES_TO_ZERO_BYTES: bool = true;
            fn deserialize_reader<R: Read>(_reader: &mut R) -> Result<Self> {
                Ok(<$t>::default())
            }
        }
    )*};
}
unit!((), core::ops::RangeFull);

macro_rules! tuple {
    ($($idx:tt $name:ident)+) => {
        impl<$($name: BorshDeserialize),+> BorshDeserialize for ($($name,)+) {
            const ENCODES_TO_ZERO_BYTES: bool = $(zero_bytes::<$name>())&&+;
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                Ok(($($name::deserialize_reader(reader)?,)+))
            }
        }
    };
}
tuples!(tuple []
    0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9
    10 T10 11 T11 12 T12 13 T13 14 T14 15 T15 16 T16 17 T17 18 T18 19 T19
);

macro_rules! range {
    ($($t:ident($($side:ident),*) => $make:expr,)*) => {$(
        impl<T: BorshDeserialize> BorshDeserialize for core::ops::$t<T> {
            const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                let ($($side,)*) = <_>::deserialize_reader(reader)?;
                Ok($make)
            }
        }
    )*};
}
range! {
    Range(start, end) => start..end,
    RangeInclusive(start, end) => start..=end,
    RangeFrom(start) => start..,
    RangeTo(end) => ..end,
    RangeToInclusive(end) => ..=end,
}

/// Module is available if borsh is built with `features = ["rc"]`.
#[cfg(feature = "rc")]
pub mod rc {
    use super::*;
    use crate::__private::maybestd::{rc::Rc, sync::Arc};

    macro_rules! shared {
        ($($t:ident)*) => {$(
            /// Decoding doesn't preserve sharing: each pointer gets its own copy.
            impl<T: ?Sized> BorshDeserialize for $t<T>
            where
                Box<T>: BorshDeserialize,
            {
                const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<Box<T>>();
                #[inline]
                fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                    Ok(Box::<T>::deserialize_reader(reader)?.into())
                }
            }
        )*};
    }
    shared!(Rc Arc);
}

impl<T: ?Sized> BorshDeserialize for PhantomData<T> {
    const ENCODES_TO_ZERO_BYTES: bool = true;
    fn deserialize_reader<R: Read>(_: &mut R) -> Result<Self> {
        Ok(PhantomData)
    }
}

impl<T: BorshDeserialize + Copy> BorshDeserialize for core::cell::Cell<T> {
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        T::deserialize_reader(reader).map(Self::new)
    }
}

impl<T: BorshDeserialize> BorshDeserialize for core::cell::RefCell<T> {
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        T::deserialize_reader(reader).map(Self::new)
    }
}

/// Deserializes an object from a slice of bytes.
/// # Example
/// ```
/// use borsh::{BorshDeserialize, BorshSerialize, from_slice, to_vec};
///
/// /// derive is only available if borsh is built with `features = ["derive"]`
/// # #[cfg(feature = "derive")]
/// #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
/// struct MyStruct {
///    a: u64,
///    b: Vec<u8>,
/// }
///
/// # #[cfg(feature = "derive")]
/// let original = MyStruct { a: 10, b: vec![1, 2, 3] };
/// # #[cfg(feature = "derive")]
/// let encoded = to_vec(&original).unwrap();
/// # #[cfg(feature = "derive")]
/// let decoded = from_slice::<MyStruct>(&encoded).unwrap();
/// # #[cfg(feature = "derive")]
/// assert_eq!(original, decoded);
/// ```
/// # Errors
/// If the data is invalid, this function will return an error.
/// # Note
/// This function will return an error if the data is not fully read.
pub fn from_slice<T: BorshDeserialize>(v: &[u8]) -> Result<T> {
    let mut v_mut = v;
    let object = T::deserialize(&mut v_mut)?;
    if !v_mut.is_empty() {
        return Err(invalid(ERROR_NOT_ALL_BYTES_READ));
    }
    Ok(object)
}

/// Deserializes an object from a reader.
/// # Example
/// ```
/// use borsh::{BorshDeserialize, BorshSerialize, from_reader, to_vec};
///
/// /// derive is only available if borsh is built with `features = ["derive"]`
/// # #[cfg(feature = "derive")]
/// #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
/// struct MyStruct {
///     a: u64,
///     b: Vec<u8>,
/// }
///
/// # #[cfg(feature = "derive")]
/// let original = MyStruct { a: 10, b: vec![1, 2, 3] };
/// # #[cfg(feature = "derive")]
/// let encoded = to_vec(&original).unwrap();
/// # #[cfg(feature = "derive")]
/// let decoded = from_reader::<_, MyStruct>(&mut encoded.as_slice()).unwrap();
/// # #[cfg(feature = "derive")]
/// assert_eq!(original, decoded);
/// ```
///
/// After decoding, the reader must be at its end: trailing bytes are an error. The
/// end-of-input check retries `Interrupted` and returns any other I/O error.
pub fn from_reader<R: Read, T: BorshDeserialize>(reader: &mut R) -> Result<T> {
    let result = T::deserialize_reader(reader)?;
    expect_end(reader)?;
    Ok(result)
}

/// The end-of-input check of `from_reader`, plain and bounded.
fn expect_end<R: Read>(reader: &mut R) -> Result<()> {
    let mut buf = [0u8; 1];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(invalid(ERROR_NOT_ALL_BYTES_READ)),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

/// Deserializes an object from a slice of bytes, allowing values to be nested
/// at most `limit` levels deep.
///
/// Behaves like [`from_slice`], which uses [`DEFAULT_RECURSION_LIMIT`]. See
/// that constant for what counts as a level. Use this to accept data that
/// legitimately nests deeper than the default, or to tighten the limit.
/// A higher limit needs a correspondingly larger stack.
///
/// Only available with the `std` feature: without it nesting depth is not
/// tracked.
/// # Example
/// ```
/// use borsh::de::from_slice_with_recursion_limit;
///
/// // 3 levels of `Box`.
/// let encoded = 7u8.to_le_bytes();
/// let decoded = from_slice_with_recursion_limit::<Box<Box<Box<u8>>>>(&encoded, 3).unwrap();
/// assert_eq!(***decoded, 7);
/// assert!(from_slice_with_recursion_limit::<Box<Box<Box<u8>>>>(&encoded, 2).is_err());
/// ```
#[cfg(feature = "std")]
pub fn from_slice_with_recursion_limit<T: BorshDeserialize>(v: &[u8], limit: usize) -> Result<T> {
    with_recursion_limit(limit, || from_slice(v))
}

/// Deserializes an object from a reader, allowing values to be nested at most
/// `limit` levels deep.
///
/// Behaves like [`from_reader`], which uses [`DEFAULT_RECURSION_LIMIT`]. See
/// [`from_slice_with_recursion_limit`] for details.
///
/// Only available with the `std` feature.
#[cfg(feature = "std")]
pub fn from_reader_with_recursion_limit<R: Read, T: BorshDeserialize>(
    reader: &mut R,
    limit: usize,
) -> Result<T> {
    with_recursion_limit(limit, || from_reader(reader))
}

/// Default limit on nesting depth for [`BorshDeserialize`].
///
/// Every `Box<T>` and every non-empty `Vec<T>` (and the collections built on
/// it, such as `VecDeque`, `BTreeMap` or `HashSet`) that is being
/// deserialized counts as one level, so `Vec<Box<T>>` uses two levels per
/// level of nesting. That includes `Vec<u8>` and the types decoded through
/// it, such as a non-empty `String`, so a value at the limit can be accepted
/// with an empty string in its innermost level and rejected with a non-empty
/// one. Use `from_slice_with_recursion_limit`, `from_reader_with_recursion_limit`
/// or `with_recursion_limit` to deserialize with a different limit.
///
/// The limit counts levels, not bytes of stack. The stack used per level
/// grows with the inline size of the value being built at that level, so a
/// type that is large inline (for example one holding a big array next to its
/// `Box`) can still overflow the stack before it reaches the limit, especially
/// on threads with small stacks or in debug builds. Use a lower limit for such
/// types, or deserialize them on a thread with a larger stack.
///
/// The counter is thread-local, so it needs `std`. Without `std` nesting depth
/// is not tracked.
pub const DEFAULT_RECURSION_LIMIT: usize = 1024;

#[cfg(feature = "std")]
std::thread_local! {
    static DEPTH: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static LIMIT: core::cell::Cell<usize> =
        const { core::cell::Cell::new(DEFAULT_RECURSION_LIMIT) };
}

/// One level of nesting, entered by each `Box` and each nonempty collection, and released
/// on drop, so the depth is restored on errors and panics too. A no-op without `std`.
struct RecursionGuard(PhantomData<*const ()>);

impl RecursionGuard {
    #[inline]
    fn enter() -> Result<Self> {
        #[cfg(feature = "std")]
        {
            let depth = DEPTH.with(|d| d.get());
            if depth >= LIMIT.with(|l| l.get()) {
                return Err(invalid("Recursion limit exceeded"));
            }
            DEPTH.with(|d| d.set(depth + 1));
        }
        Ok(RecursionGuard(PhantomData))
    }
}

#[cfg(feature = "std")]
impl Drop for RecursionGuard {
    #[inline]
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get() - 1));
    }
}

/// Runs `f` with the nesting limit set to `limit`, and restores the previous
/// limit afterwards, also when `f` fails or panics.
///
/// Use this to change the limit for decoding that [`from_slice_with_recursion_limit`]
/// and [`from_reader_with_recursion_limit`] don't cover, such as decoding one
/// value from the front of a longer buffer with [`BorshDeserialize::deserialize`].
/// Levels already in use by an enclosing call still count towards `limit`.
///
/// Only available with the `std` feature.
/// # Example
/// ```
/// use borsh::{de::with_recursion_limit, BorshDeserialize};
///
/// // 3 levels of `Box`, followed by the start of the next value.
/// let data = [7u8, 1, 2];
/// let mut buf = &data[..];
/// let decoded = with_recursion_limit(3, || Box::<Box<Box<u8>>>::deserialize(&mut buf)).unwrap();
/// assert_eq!(***decoded, 7);
/// assert_eq!(buf, [1, 2]);
/// ```
#[cfg(feature = "std")]
pub fn with_recursion_limit<T>(limit: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            LIMIT.with(|l| l.set(self.0));
        }
    }
    let _restore = Restore(LIMIT.with(|l| l.replace(limit)));
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cautious_bounds() {
        assert_eq!(cautious::<u8>(10), 10);
        assert_eq!(cautious::<u64>(u32::MAX), 512);
        assert_eq!(cautious::<[u8; 4097]>(u32::MAX), 1);
        assert_eq!(cautious::<[u8; 4097]>(0), 1);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn cautious_huge_elements() {
        assert_eq!(cautious::<[u8; 1 << 32]>(u32::MAX), 1);
        assert_eq!(cautious::<[u8; (1 << 32) + 1]>(u32::MAX), 1);
    }

    /// A failed or panicking array decode drops exactly the elements it built.
    #[test]
    fn array_decode_doesnt_leak() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static DECODED: AtomicUsize = AtomicUsize::new(0);
        static DROPPED: AtomicUsize = AtomicUsize::new(0);

        struct Counted(#[allow(dead_code)] u8);
        impl BorshDeserialize for Counted {
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                let val = u8::deserialize_reader(reader)?;
                if DECODED.fetch_add(1, Ordering::SeqCst) >= 7 {
                    panic!("panic in deserialize");
                }
                Ok(Counted(val))
            }
        }
        impl Drop for Counted {
            fn drop(&mut self) {
                DROPPED.fetch_add(1, Ordering::SeqCst);
            }
        }

        assert!(<[Counted; 5]>::deserialize(&mut &[0u8; 3][..]).is_err());
        assert_eq!(
            (
                DECODED.load(Ordering::SeqCst),
                DROPPED.load(Ordering::SeqCst)
            ),
            (3, 3)
        );
        assert!(<[Counted; 2]>::deserialize(&mut &[0u8; 2][..]).is_ok());
        assert_eq!(
            (
                DECODED.load(Ordering::SeqCst),
                DROPPED.load(Ordering::SeqCst)
            ),
            (5, 5)
        );
        #[cfg(feature = "std")]
        {
            let result = std::panic::catch_unwind(|| {
                <[Counted; 3]>::deserialize(&mut &[0u8; 3][..]).unwrap()
            });
            assert!(result.is_err());
            // The third element panicked before it existed.
            assert_eq!(
                (
                    DECODED.load(Ordering::SeqCst),
                    DROPPED.load(Ordering::SeqCst)
                ),
                (8, 7)
            );
        }
    }
}
