use core::convert::TryFrom;
use core::hash::BuildHasher;
use core::marker::PhantomData;

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::maybestd::{
    borrow::{Cow, ToOwned},
    boxed::Box,
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque},
    io::{ErrorKind, Result},
    string::String,
    vec::Vec,
};

#[cfg(feature = "rc")]
use crate::maybestd::sync::Arc;

pub(crate) mod helpers;

const DEFAULT_SERIALIZER_CAPACITY: usize = 1024;

pub trait AsyncWriter: AsyncWrite + Send + Unpin {}
impl<W: AsyncWrite + Send + Unpin> AsyncWriter for W {}

/// A data-structure that can be serialized into binary format by NBOR.
///
/// ```
/// use borsh::BorshSerialize;
///
/// #[derive(BorshSerialize)]
/// struct MyBorshSerializableStruct {
///     value: String,
/// }
///
/// let x = MyBorshSerializableStruct { value: "hello".to_owned() };
/// let mut buffer: Vec<u8> = Vec::new();
/// x.serialize(&mut buffer).unwrap();
/// let single_serialized_buffer_len = buffer.len();
///
/// x.serialize(&mut buffer).unwrap();
/// assert_eq!(buffer.len(), single_serialized_buffer_len * 2);
///
/// let mut buffer: Vec<u8> = vec![0; 1024 + single_serialized_buffer_len];
/// let mut buffer_slice_enough_for_the_data = &mut buffer[1024..1024 + single_serialized_buffer_len];
/// x.serialize(&mut buffer_slice_enough_for_the_data).unwrap();
/// ```
#[async_trait::async_trait]
pub trait AsyncBorshSerialize {
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()>;

    /// Serialize this instance into a vector of bytes.
    async fn try_to_vec(&self) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(DEFAULT_SERIALIZER_CAPACITY);
        self.serialize(&mut result).await?;
        Ok(result)
    }

    #[inline]
    #[doc(hidden)]
    fn u8_slice(slice: &[Self]) -> Option<&[u8]>
    where
        Self: Sized,
    {
        let _ = slice;
        None
    }
}

#[async_trait::async_trait]
impl AsyncBorshSerialize for u8 {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(core::slice::from_ref(self)).await
    }

    #[inline]
    fn u8_slice(slice: &[Self]) -> Option<&[u8]> {
        Some(slice)
    }
}

macro_rules! impl_for_integer {
    ($type: ident) => {
        #[async_trait::async_trait]
        impl AsyncBorshSerialize for $type {
            #[inline]
            async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
                let bytes = self.to_le_bytes();
                writer.write_all(&bytes).await
            }
        }
    };
}

impl_for_integer!(i8);
impl_for_integer!(i16);
impl_for_integer!(i32);
impl_for_integer!(i64);
impl_for_integer!(i128);
impl_for_integer!(u16);
impl_for_integer!(u32);
impl_for_integer!(u64);
impl_for_integer!(u128);

macro_rules! impl_for_nonzero_integer {
    ($type: ty) => {
        #[async_trait::async_trait]
        impl AsyncBorshSerialize for $type {
            #[inline]
            async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
                AsyncBorshSerialize::serialize(&self.get(), writer).await
            }
        }
    };
}

impl_for_nonzero_integer!(core::num::NonZeroI8);
impl_for_nonzero_integer!(core::num::NonZeroI16);
impl_for_nonzero_integer!(core::num::NonZeroI32);
impl_for_nonzero_integer!(core::num::NonZeroI64);
impl_for_nonzero_integer!(core::num::NonZeroI128);
impl_for_nonzero_integer!(core::num::NonZeroU8);
impl_for_nonzero_integer!(core::num::NonZeroU16);
impl_for_nonzero_integer!(core::num::NonZeroU32);
impl_for_nonzero_integer!(core::num::NonZeroU64);
impl_for_nonzero_integer!(core::num::NonZeroU128);
impl_for_nonzero_integer!(core::num::NonZeroUsize);

#[async_trait::async_trait]
impl AsyncBorshSerialize for isize {
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        AsyncBorshSerialize::serialize(&(*self as i64), writer).await
    }
}

#[async_trait::async_trait]
impl AsyncBorshSerialize for usize {
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        AsyncBorshSerialize::serialize(&(*self as u64), writer).await
    }
}

// Note NaNs have a portability issue. Specifically, signalling NaNs on MIPS are quiet NaNs on x86,
// and vice-versa. We disallow NaNs to avoid this issue.
macro_rules! impl_for_float {
    ($type: ident) => {
        #[async_trait::async_trait]
        impl AsyncBorshSerialize for $type {
            #[inline]
            async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
                assert!(
                    !self.is_nan(),
                    "For portability reasons we do not allow to serialize NaNs."
                );
                writer.write_all(&self.to_bits().to_le_bytes()).await
            }
        }
    };
}

impl_for_float!(f32);
impl_for_float!(f64);

#[async_trait::async_trait]
impl AsyncBorshSerialize for bool {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        (u8::from(*self)).serialize(writer).await
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for core::ops::Range<T>
where
    T: AsyncBorshSerialize + Send + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.start.serialize(writer).await?;
        self.end.serialize(writer).await
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for Option<T>
where
    T: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        match self {
            None => 0u8.serialize(writer).await,
            Some(value) => {
                1u8.serialize(writer).await?;
                value.serialize(writer).await
            }
        }
    }
}

#[async_trait::async_trait]
impl<T, E> AsyncBorshSerialize for core::result::Result<T, E>
where
    T: AsyncBorshSerialize + Sync,
    E: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        match self {
            Err(e) => {
                0u8.serialize(writer).await?;
                e.serialize(writer).await
            }
            Ok(v) => {
                1u8.serialize(writer).await?;
                v.serialize(writer).await
            }
        }
    }
}

#[async_trait::async_trait]
impl AsyncBorshSerialize for str {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.as_bytes().serialize(writer).await
    }
}

#[async_trait::async_trait]
impl AsyncBorshSerialize for String {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.as_bytes().serialize(writer).await
    }
}

/// Helper method that is used to serialize a slice of data (without the length marker).
#[inline]
async fn serialize_slice<T: AsyncBorshSerialize, W: AsyncWriter>(
    data: &[T],
    writer: &mut W,
) -> Result<()> {
    if let Some(u8_slice) = T::u8_slice(data) {
        writer.write_all(u8_slice).await?;
    } else {
        for item in data {
            item.serialize(writer).await?;
        }
    }
    Ok(())
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for [T]
where
    T: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        writer
            .write_all(
                &(u32::try_from(self.len()).map_err(|_| ErrorKind::InvalidInput)?).to_le_bytes(),
            )
            .await?;
        serialize_slice(self, writer).await
    }
}

#[async_trait::async_trait]
impl<T: AsyncBorshSerialize + ?Sized + Sync> AsyncBorshSerialize for &T {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        (*self).serialize(writer).await
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for Cow<'_, T>
where
    T: AsyncBorshSerialize + ToOwned + ?Sized + Sync,
    <T as ToOwned>::Owned: Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.as_ref().serialize(writer).await
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for Vec<T>
where
    T: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.as_slice().serialize(writer).await
    }
}

#[cfg(any(test, feature = "bytes"))]
#[async_trait::async_trait]
impl AsyncBorshSerialize for bytes::Bytes {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.as_ref().serialize(writer).await
    }
}

#[cfg(any(test, feature = "bytes"))]
#[async_trait::async_trait]
impl AsyncBorshSerialize for bytes::BytesMut {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.as_ref().serialize(writer).await
    }
}

#[cfg(any(test, feature = "bson"))]
#[async_trait::async_trait]
impl AsyncBorshSerialize for bson::oid::ObjectId {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.bytes().serialize(writer).await
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for VecDeque<T>
where
    T: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        writer
            .write_all(
                &(u32::try_from(self.len()).map_err(|_| ErrorKind::InvalidInput)?).to_le_bytes(),
            )
            .await?;
        let slices = self.as_slices();
        serialize_slice(slices.0, writer).await?;
        serialize_slice(slices.1, writer).await
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for LinkedList<T>
where
    T: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        writer
            .write_all(
                &(u32::try_from(self.len()).map_err(|_| ErrorKind::InvalidInput)?).to_le_bytes(),
            )
            .await?;
        for item in self {
            item.serialize(writer).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for BinaryHeap<T>
where
    T: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        // It could have been just `self.as_slice().serialize(writer)`, but there is no
        // `as_slice()` method:
        // https://internals.rust-lang.org/t/should-i-add-as-slice-method-to-binaryheap/13816
        writer
            .write_all(
                &(u32::try_from(self.len()).map_err(|_| ErrorKind::InvalidInput)?).to_le_bytes(),
            )
            .await?;
        for item in self {
            item.serialize(writer).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl<K, V, H> AsyncBorshSerialize for HashMap<K, V, H>
where
    K: AsyncBorshSerialize + PartialOrd + Sync,
    V: AsyncBorshSerialize + Sync,
    H: BuildHasher + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        let mut vec = self.iter().collect::<Vec<_>>();
        vec.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap());
        u32::try_from(vec.len())
            .map_err(|_| ErrorKind::InvalidInput)?
            .serialize(writer)
            .await?;
        for (key, value) in vec {
            key.serialize(writer).await?;
            value.serialize(writer).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl<T, H> AsyncBorshSerialize for HashSet<T, H>
where
    T: AsyncBorshSerialize + PartialOrd + Sync,
    H: BuildHasher + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        let mut vec = self.iter().collect::<Vec<_>>();
        vec.sort_by(|a, b| a.partial_cmp(b).unwrap());
        u32::try_from(vec.len())
            .map_err(|_| ErrorKind::InvalidInput)?
            .serialize(writer)
            .await?;
        for item in vec {
            item.serialize(writer).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl<K, V> AsyncBorshSerialize for BTreeMap<K, V>
where
    K: AsyncBorshSerialize + Sync,
    V: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        // NOTE: BTreeMap iterates over the entries that are sorted by key, so the serialization
        // result will be consistent without a need to sort the entries as we do for HashMap
        // serialization.
        u32::try_from(self.len())
            .map_err(|_| ErrorKind::InvalidInput)?
            .serialize(writer)
            .await?;
        for (key, value) in self {
            key.serialize(writer).await?;
            value.serialize(writer).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshSerialize for BTreeSet<T>
where
    T: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        // NOTE: BTreeSet iterates over the items that are sorted, so the serialization result will
        // be consistent without a need to sort the entries as we do for HashSet serialization.
        u32::try_from(self.len())
            .map_err(|_| ErrorKind::InvalidInput)?
            .serialize(writer)
            .await?;
        for item in self {
            item.serialize(writer).await?;
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshSerialize for std::net::SocketAddr {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        match *self {
            std::net::SocketAddr::V4(ref addr) => {
                0u8.serialize(writer).await?;
                addr.serialize(writer).await
            }
            std::net::SocketAddr::V6(ref addr) => {
                1u8.serialize(writer).await?;
                addr.serialize(writer).await
            }
        }
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshSerialize for std::net::SocketAddrV4 {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.ip().serialize(writer).await?;
        self.port().serialize(writer).await
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshSerialize for std::net::SocketAddrV6 {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.ip().serialize(writer).await?;
        self.port().serialize(writer).await
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshSerialize for std::net::Ipv4Addr {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.octets()).await
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshSerialize for std::net::Ipv6Addr {
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.octets()).await
    }
}

#[async_trait::async_trait]
impl<T: AsyncBorshSerialize + ?Sized + Sync> AsyncBorshSerialize for Box<T> {
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        self.as_ref().serialize(writer).await
    }
}

#[async_trait::async_trait]
impl<T, const N: usize> AsyncBorshSerialize for [T; N]
where
    T: AsyncBorshSerialize + Sync,
{
    #[inline]
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        if N == 0 {
            return Ok(());
        } else if let Some(u8_slice) = T::u8_slice(self) {
            writer.write_all(u8_slice).await?;
        } else {
            for el in self.iter() {
                el.serialize(writer).await?;
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AsyncBorshSerialize for () {
    async fn serialize<W: AsyncWriter>(&self, _writer: &mut W) -> Result<()> {
        Ok(())
    }
}

macro_rules! impl_tuple {
    ($($idx:tt $name:ident)+) => {
      #[async_trait::async_trait]
      impl<$($name),+> AsyncBorshSerialize for ($($name,)+)
      where $($name: AsyncBorshSerialize + Sync + Send,)+
      {
        #[inline]
        async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
            $(self.$idx.serialize(writer).await?;)+
            Ok(())
        }
      }
    };
}

impl_tuple!(0 T0);
impl_tuple!(0 T0 1 T1);
impl_tuple!(0 T0 1 T1 2 T2);
impl_tuple!(0 T0 1 T1 2 T2 3 T3);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15 16 T16);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15 16 T16 17 T17);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15 16 T16 17 T17 18 T18);
impl_tuple!(0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15 16 T16 17 T17 18 T18 19 T19);

#[cfg(feature = "rc")]
#[async_trait::async_trait]
impl<T: AsyncBorshSerialize + ?Sized + Sync + Send> AsyncBorshSerialize for Arc<T> {
    async fn serialize<W: AsyncWriter>(&self, writer: &mut W) -> Result<()> {
        (**self).serialize(writer).await
    }
}

#[async_trait::async_trait]
impl<T: ?Sized + Sync> AsyncBorshSerialize for PhantomData<T> {
    async fn serialize<W: AsyncWriter>(&self, _: &mut W) -> Result<()> {
        Ok(())
    }
}
