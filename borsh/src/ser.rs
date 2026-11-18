//! `BorshSerialize` and its impls for std types.
use core::convert::TryFrom;
use core::marker::PhantomData;

use crate::__private::encodes_to_zero_bytes_ser as zero_bytes;
use crate::__private::maybestd::{
    borrow::{Cow, ToOwned},
    boxed::Box,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    string::String,
    vec::Vec,
};
use crate::error::check_zst;
use crate::io::{Error, ErrorKind, Result, Write};

/// A data-structure that can be serialized into binary format by NBOR.
///
/// ```
/// use borsh::BorshSerialize;
///
/// /// derive is only available if borsh is built with `features = ["derive"]`
/// # #[cfg(feature = "derive")]
/// #[derive(BorshSerialize)]
/// struct MyBorshSerializableStruct {
///     value: String,
/// }
///
///
/// # #[cfg(feature = "derive")]
/// let x = MyBorshSerializableStruct { value: "hello".to_owned() };
/// let mut buffer: Vec<u8> = Vec::new();
/// # #[cfg(feature = "derive")]
/// x.serialize(&mut buffer).unwrap();
/// # #[cfg(feature = "derive")]
/// let single_serialized_buffer_len = buffer.len();
///
/// # #[cfg(feature = "derive")]
/// x.serialize(&mut buffer).unwrap();
/// # #[cfg(feature = "derive")]
/// assert_eq!(buffer.len(), single_serialized_buffer_len * 2);
///
/// # #[cfg(feature = "derive")]
/// let mut buffer: Vec<u8> = vec![0; 1024 + single_serialized_buffer_len];
/// # #[cfg(feature = "derive")]
/// let mut buffer_slice_enough_for_the_data = &mut buffer[1024..1024 + single_serialized_buffer_len];
/// # #[cfg(feature = "derive")]
/// x.serialize(&mut buffer_slice_enough_for_the_data).unwrap();
/// ```
pub trait BorshSerialize {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()>;

    /// The slice's bytes, when `Self` is `u8`, so slices of bytes are written in one call.
    #[inline]
    #[doc(hidden)]
    fn u8_slice(slice: &[Self]) -> Option<&[u8]>
    where
        Self: Sized,
    {
        let _ = slice;
        None
    }

    /// Whether every value encodes to zero bytes. Collections reject such element types.
    #[doc(hidden)]
    const ENCODES_TO_ZERO_BYTES: bool = false;
}

/// A collection length: a `u32`, or `InvalidData` past `u32::MAX`.
fn write_len<W: Write>(len: usize, writer: &mut W) -> Result<()> {
    writer.write_all(
        &u32::try_from(len)
            .map_err(|_| ErrorKind::InvalidData)?
            .to_le_bytes(),
    )
}

/// Write the elements, in one call for bytes.
fn write_items<T: BorshSerialize, W: Write>(data: &[T], writer: &mut W) -> Result<()> {
    match T::u8_slice(data) {
        Some(bytes) => writer.write_all(bytes),
        None => data.iter().try_for_each(|item| item.serialize(writer)),
    }
}

impl BorshSerialize for u8 {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(core::slice::from_ref(self))
    }

    #[inline]
    fn u8_slice(slice: &[Self]) -> Option<&[u8]> {
        Some(slice)
    }
}

macro_rules! int {
    ($($t:ident)*) => {$(
        impl BorshSerialize for $t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                writer.write_all(&self.to_le_bytes())
            }
        }
    )*};
}
int!(i8 i16 i32 i64 i128 u16 u32 u64 u128);

macro_rules! nonzero {
    ($($t:ident)*) => {$(
        impl BorshSerialize for core::num::$t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                BorshSerialize::serialize(&self.get(), writer)
            }
        }
    )*};
}
nonzero! {
    NonZeroI8 NonZeroI16 NonZeroI32 NonZeroI64 NonZeroI128
    NonZeroU8 NonZeroU16 NonZeroU32 NonZeroU64 NonZeroU128 NonZeroUsize
}

/// `isize` and `usize` travel as 64 bits.
macro_rules! size {
    ($($t:ident $wire:ident)*) => {$(
        impl BorshSerialize for $t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                BorshSerialize::serialize(&(*self as $wire), writer)
            }
        }
    )*};
}
size!(isize i64 usize u64);

macro_rules! float {
    ($($t:ident)*) => {$(
        impl BorshSerialize for $t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                if self.is_nan() {
                    let msg = "For portability reasons we do not allow to serialize NaNs.";
                    return Err(Error::new(ErrorKind::InvalidData, msg));
                }
                writer.write_all(&self.to_bits().to_le_bytes())
            }
        }
    )*};
}
float!(f32 f64);

impl BorshSerialize for bool {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        u8::from(*self).serialize(writer)
    }
}

impl<T: BorshSerialize> BorshSerialize for Option<T> {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            None => 0u8.serialize(writer),
            Some(value) => {
                1u8.serialize(writer)?;
                value.serialize(writer)
            }
        }
    }
}

impl<T: BorshSerialize, E: BorshSerialize> BorshSerialize for core::result::Result<T, E> {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            Err(e) => {
                0u8.serialize(writer)?;
                e.serialize(writer)
            }
            Ok(v) => {
                1u8.serialize(writer)?;
                v.serialize(writer)
            }
        }
    }
}

macro_rules! as_bytes {
    ($($(#[$cfg:meta])* $t:ty => |$s:ident| $bytes:expr,)*) => {$(
        $(#[$cfg])*
        impl BorshSerialize for $t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                let $s = self;
                BorshSerialize::serialize($bytes, writer)
            }
        }
    )*};
}
as_bytes! {
    str => |s| s.as_bytes(),
    String => |s| s.as_bytes(),
    #[cfg(feature = "bytes")] bytes::Bytes => |s| &s[..],
    #[cfg(feature = "bytes")] bytes::BytesMut => |s| &s[..],
    #[cfg(feature = "bson")] bson::oid::ObjectId => |s| &s.bytes(),
    #[cfg(feature = "uuid")] uuid::Uuid => |s| s.as_bytes(),
}

#[cfg(feature = "ascii")]
pub mod ascii {
    use super::*;

    impl BorshSerialize for ::ascii::AsciiChar {
        fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
            self.as_byte().serialize(writer)
        }
    }

    impl BorshSerialize for ::ascii::AsciiStr {
        fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
            self.as_bytes().serialize(writer)
        }
    }

    impl BorshSerialize for ::ascii::AsciiString {
        fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
            self.as_bytes().serialize(writer)
        }
    }
}

impl<T: BorshSerialize> BorshSerialize for [T] {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        check_zst::<T>(T::ENCODES_TO_ZERO_BYTES)?;
        write_len(self.len(), writer)?;
        write_items(self, writer)
    }
}

impl<T: BorshSerialize + ?Sized> BorshSerialize for &T {
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        (*self).serialize(writer)
    }
}

impl<T: BorshSerialize + ToOwned + ?Sized> BorshSerialize for Cow<'_, T> {
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.as_ref().serialize(writer)
    }
}

impl<T: BorshSerialize> BorshSerialize for Vec<T> {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.as_slice().serialize(writer)
    }
}

impl<T: BorshSerialize> BorshSerialize for VecDeque<T> {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        check_zst::<T>(T::ENCODES_TO_ZERO_BYTES)?;
        write_len(self.len(), writer)?;
        let (front, back) = self.as_slices();
        write_items(front, writer)?;
        write_items(back, writer)
    }
}

/// A length, then each item: the impl for collections that iterate in their encoded order.
/// Map entries are `(&K, &V)` tuples, so they write the key, then the value.
macro_rules! seq {
    ($($(#[$attr:meta])* [$($g:tt)*] $t:ty, $key:ident;)*) => {$(
        $(#[$attr])*
        impl<$($g)*> BorshSerialize for $t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                check_zst::<$key>($key::ENCODES_TO_ZERO_BYTES)?;
                write_len(self.len(), writer)?;
                self.iter().try_for_each(|item| item.serialize(writer))
            }
        }
    )*};
}
seq! {
    [T: BorshSerialize] LinkedList<T>, T;
    [T: BorshSerialize] BTreeSet<T>, T;
    [K: BorshSerialize, V: BorshSerialize] BTreeMap<K, V>, K;
    /// Items are written in insertion (iteration) order, **not** sorted.
    ///
    /// This encoding is order-preserving and therefore **not canonical**: two
    /// `IndexSet`s that compare equal with `==` (which ignores order) but were
    /// built in a different order serialize to different bytes. Don't rely on it
    /// for hashing or signing unless the insertion order is itself deterministic.
    /// If canonical bytes are needed, call [`IndexSet::sort`](indexmap::IndexSet::sort)
    /// before serializing, or use [`BTreeSet`] or `HashSet` instead, both of which
    /// are serialized in ascending order.
    #[cfg(feature = "indexmap")]
    [T: BorshSerialize, S] indexmap::IndexSet<T, S>, T;
    /// Entries are written in insertion (iteration) order, **not** sorted by key.
    ///
    /// This encoding is order-preserving and therefore **not canonical**: two
    /// `IndexMap`s that compare equal with `==` (which ignores order) but were
    /// built in a different order serialize to different bytes. Don't rely on it
    /// for hashing or signing unless the insertion order is itself deterministic.
    /// If canonical bytes are needed, call [`IndexMap::sort_keys`](indexmap::IndexMap::sort_keys)
    /// before serializing, or use [`BTreeMap`] or `HashMap` instead, both of which
    /// are serialized in ascending key order.
    #[cfg(feature = "indexmap")]
    [K: BorshSerialize, V: BorshSerialize, S] indexmap::IndexMap<K, V, S>, K;
}

#[cfg(any(feature = "std", feature = "hashbrown"))]
pub mod hashes {
    //! Module defines [BorshSerialize] implementation for [HashMap]/[HashSet].
    use super::*;
    use crate::__private::maybestd::collections::{HashMap, HashSet};
    use core::hash::BuildHasher;

    /// Hash collections are written sorted, so equal collections encode the same.
    impl<K: BorshSerialize + Ord, V: BorshSerialize, H: BuildHasher> BorshSerialize
        for HashMap<K, V, H>
    {
        fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
            check_zst::<K>(K::ENCODES_TO_ZERO_BYTES)?;
            let mut vec = self.iter().collect::<Vec<_>>();
            vec.sort_by_key(|(k, _)| *k);
            write_len(vec.len(), writer)?;
            vec.into_iter().try_for_each(|kv| kv.serialize(writer))
        }
    }

    impl<T: BorshSerialize + Ord, H: BuildHasher> BorshSerialize for HashSet<T, H> {
        fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
            check_zst::<T>(T::ENCODES_TO_ZERO_BYTES)?;
            let mut vec = self.iter().collect::<Vec<_>>();
            vec.sort();
            write_len(vec.len(), writer)?;
            vec.into_iter().try_for_each(|item| item.serialize(writer))
        }
    }
}

/// `IpAddr` and `SocketAddr` are enums tagged 0 (v4) and 1 (v6). **Lossy for
/// `SocketAddr::V6`:** see the `SocketAddrV6` impl. The `flowinfo` and `scope_id` fields are
/// not encoded.
macro_rules! net_enum {
    ($($t:ident)*) => {$(
        impl BorshSerialize for core::net::$t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                match self {
                    Self::V4(addr) => {
                        0u8.serialize(writer)?;
                        addr.serialize(writer)
                    }
                    Self::V6(addr) => {
                        1u8.serialize(writer)?;
                        addr.serialize(writer)
                    }
                }
            }
        }
    )*};
}
net_enum!(SocketAddr IpAddr);

macro_rules! socket {
    ($($(#[$doc:meta])* $t:ident)*) => {$(
        $(#[$doc])*
        impl BorshSerialize for core::net::$t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                self.ip().serialize(writer)?;
                self.port().serialize(writer)
            }
        }
    )*};
}
socket! {
    SocketAddrV4
    /// Encoded as the 16 address octets followed by the port as a little-endian `u16`
    /// (18 bytes total).
    ///
    /// **This encoding is lossy.** [`flowinfo`](core::net::SocketAddrV6::flowinfo) and
    /// [`scope_id`](core::net::SocketAddrV6::scope_id) are *not* encoded, and deserialization
    /// always sets both to `0`. For any value where either field is non-zero (for example a
    /// link-local address such as `[fe80::1%2]:8080`, or an address returned by
    /// `TcpStream::peer_addr` for a link-local peer), `from_slice(&to_vec(&x)?)? != x`, and two
    /// values that differ only in those fields serialize to identical bytes.
    ///
    /// `scope_id` is an interface index local to one host, and `flowinfo` (the traffic class and
    /// flow label) is rarely meaningful in a stored address. If you need to preserve them,
    /// serialize them alongside the address explicitly.
    SocketAddrV6
}

macro_rules! ip {
    ($($t:ident)*) => {$(
        impl BorshSerialize for core::net::$t {
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                writer.write_all(&self.octets())
            }
        }
    )*};
}
ip!(Ipv4Addr Ipv6Addr);

impl<T: BorshSerialize + ?Sized> BorshSerialize for Box<T> {
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.as_ref().serialize(writer)
    }
}

impl<T: BorshSerialize, const N: usize> BorshSerialize for [T; N] {
    const ENCODES_TO_ZERO_BYTES: bool = N == 0 || zero_bytes::<T>();
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        if N == 0 {
            return Ok(());
        }
        write_items(self, writer)
    }
}

macro_rules! unit {
    ($($t:ty),*) => {$(
        impl BorshSerialize for $t {
            const ENCODES_TO_ZERO_BYTES: bool = true;
            fn serialize<W: Write>(&self, _writer: &mut W) -> Result<()> {
                Ok(())
            }
        }
    )*};
}
unit!((), core::ops::RangeFull);

macro_rules! tuple {
    ($($idx:tt $name:ident)+) => {
        impl<$($name: BorshSerialize),+> BorshSerialize for ($($name,)+) {
            const ENCODES_TO_ZERO_BYTES: bool = $(zero_bytes::<$name>())&&+;
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                $(self.$idx.serialize(writer)?;)+
                Ok(())
            }
        }
    };
}
tuples!(tuple []
    0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9
    10 T10 11 T11 12 T12 13 T13 14 T14 15 T15 16 T16 17 T17 18 T18 19 T19
);

macro_rules! range {
    ($($(#[$doc:meta])* $t:ident |$s:ident| $($side:expr),*;)*) => {$(
        $(#[$doc])*
        impl<T: BorshSerialize> BorshSerialize for core::ops::$t<T> {
            const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
            fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                let $s = self;
                $($side.serialize(writer)?;)*
                Ok(())
            }
        }
    )*};
}
range! {
    Range |s| &s.start, &s.end;
    /// Writes `start()` then `end()`. A range iterated to exhaustion can be
    /// marked empty by a flag that is not encoded, so it can decode as a
    /// non-empty range (for example `255u8..=255` after one `next()`). The
    /// format is frozen; check `is_empty()` before serializing if that matters.
    RangeInclusive |s| s.start(), s.end();
    RangeFrom |s| &s.start;
    RangeTo |s| &s.end;
    RangeToInclusive |s| &s.end;
}

/// Module is available if borsh is built with `features = ["rc"]`.
#[cfg(feature = "rc")]
pub mod rc {
    use super::*;
    use crate::__private::maybestd::{rc::Rc, sync::Arc};

    macro_rules! shared {
        ($($t:ident)*) => {$(
            /// Encodes the pointee; sharing isn't preserved.
            impl<T: BorshSerialize + ?Sized> BorshSerialize for $t<T> {
                const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
                fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
                    (**self).serialize(writer)
                }
            }
        )*};
    }
    shared!(Rc Arc);
}

impl<T: ?Sized> BorshSerialize for PhantomData<T> {
    const ENCODES_TO_ZERO_BYTES: bool = true;
    fn serialize<W: Write>(&self, _: &mut W) -> Result<()> {
        Ok(())
    }
}

impl<T: BorshSerialize + Copy> BorshSerialize for core::cell::Cell<T> {
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        T::serialize(&self.get(), writer)
    }
}

impl<T: BorshSerialize> BorshSerialize for core::cell::RefCell<T> {
    const ENCODES_TO_ZERO_BYTES: bool = zero_bytes::<T>();
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self.try_borrow() {
            Ok(value) => value.serialize(writer),
            Err(_) => Err(Error::other("already mutably borrowed")),
        }
    }
}

pub(crate) mod helpers {
    use crate::__private::maybestd::vec::Vec;
    use crate::io::{ErrorKind, Result, Write};
    use crate::BorshSerialize;

    /// Serialize an object into a vector of bytes.
    /// # Example
    ///
    /// ```
    /// assert_eq!(vec![12, 0, 0, 0, 0, 0, 0, 0], borsh::to_vec(&12u64).unwrap());
    /// ```
    pub fn to_vec<T: BorshSerialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(1024);
        value.serialize(&mut result)?;
        Ok(result)
    }

    /// Serializes an object directly into a `Writer`.
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "std")]
    /// let stderr = std::io::stderr();
    /// # #[cfg(feature = "std")]
    /// assert_eq!((), borsh::to_writer(&stderr, "hello_0x0a").unwrap());
    /// ```
    pub fn to_writer<T: BorshSerialize + ?Sized, W: Write>(mut writer: W, value: &T) -> Result<()> {
        value.serialize(&mut writer)
    }

    /// Serializes an object without allocation to compute and return its length
    /// # Example
    ///
    /// ```
    /// use borsh::BorshSerialize;
    ///
    /// /// derive is only available if borsh is built with `features = ["derive"]`
    /// # #[cfg(feature = "derive")]
    /// #[derive(BorshSerialize)]
    /// struct A {
    ///     tag: String,
    ///     value: u64,
    /// };
    ///
    /// # #[cfg(feature = "derive")]
    /// let a = A { tag: "hello".to_owned(), value: 42 };
    ///
    /// assert_eq!(8, borsh::object_length(&12u64).unwrap());
    /// # #[cfg(feature = "derive")]
    /// assert_eq!(17, borsh::object_length(&a).unwrap());
    /// ```
    pub fn object_length<T: BorshSerialize + ?Sized>(value: &T) -> Result<usize> {
        struct Length(usize);
        impl Write for Length {
            #[inline]
            fn write(&mut self, buf: &[u8]) -> Result<usize> {
                self.0 = self
                    .0
                    .checked_add(buf.len())
                    .ok_or(ErrorKind::OutOfMemory)?;
                Ok(buf.len())
            }
            #[inline]
            fn flush(&mut self) -> Result<()> {
                Ok(())
            }
        }
        let mut w = Length(0);
        value.serialize(&mut w)?;
        Ok(w.0)
    }
}
