use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::{
    convert::{TryFrom, TryInto},
    hash::{BuildHasher, Hash},
    mem::{forget, size_of},
};

#[cfg(any(test, feature = "bytes"))]
use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::maybestd::{
    borrow::{Borrow, Cow, ToOwned},
    boxed::Box,
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque},
    format,
    io::{Error, ErrorKind, Read, Result},
    string::{String, ToString},
    vec,
    vec::Vec,
};

#[cfg(feature = "rc")]
use crate::maybestd::{rc::Rc, sync::Arc};

mod hint;

const ERROR_NOT_ALL_BYTES_READ: &str = "Not all bytes read";
const ERROR_UNEXPECTED_LENGTH_OF_INPUT: &str = "Unexpected length of input";
const ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_ISIZE: &str = "Overflow on machine with 32 bit isize";
const ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_USIZE: &str = "Overflow on machine with 32 bit usize";
const ERROR_INVALID_ZERO_VALUE: &str = "Expected a non-zero value";

pub trait AsyncReader: AsyncRead + Send + Unpin {}
impl<R: AsyncRead + Send + Unpin> AsyncReader for R {}

/// A data-structure that can be de-serialized from binary format by NBOR.
#[async_trait::async_trait]
pub trait AsyncBorshDeserialize: Sized {
    /// Deserializes this instance from a given slice of bytes.
    /// Updates the buffer to point at the remaining bytes.
    async fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        Self::deserialize_reader(&mut *buf).await
    }

    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self>;

    /// Deserialize this instance from a slice of bytes.
    async fn try_from_slice(v: &[u8]) -> Result<Self> {
        let mut v_mut = v;
        let result = Self::deserialize(&mut v_mut).await?;
        if !v_mut.is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ));
        }
        Ok(result)
    }

    //async fn try_from_reader<R: Reader>(reader: &mut R) -> Result<Self> {
    //    let result = Self::deserialize_reader(reader).await?;
    //    let mut buf = [0u8; 1];
    //    match reader.read_exact(&mut buf) {
    //        Err(f) if f.kind() == ErrorKind::UnexpectedEof => Ok(result),
    //        _ => Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ)),
    //    }
    //}

    #[inline]
    #[doc(hidden)]
    async fn vec_from_reader<R: AsyncReader>(
        len: u32,
        reader: &mut R,
    ) -> Result<Option<Vec<Self>>> {
        let _ = len;
        let _ = reader;
        Ok(None)
    }

    #[inline]
    #[doc(hidden)]
    async fn array_from_reader<R: AsyncReader, const N: usize>(
        reader: &mut R,
    ) -> Result<Option<[Self; N]>> {
        let _ = reader;
        Ok(None)
    }
}

/// Additional methods offered on enums which uses `[derive(BorshDeserialize)]`.
pub trait EnumExt: AsyncBorshDeserialize {
    /// Deserialises given variant of an enum from the reader.
    ///
    /// This may be used to perform validation or filtering based on what
    /// variant is being deserialised.
    ///
    /// ```
    /// use borsh::BorshDeserialize;
    /// use borsh::de::EnumExt as _;
    ///
    /// #[derive(Debug, PartialEq, Eq, BorshDeserialize)]
    /// enum MyEnum {
    ///     Zero,
    ///     One(u8),
    ///     Many(Vec<u8>)
    /// }
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct OneOrZero(MyEnum);
    ///
    /// impl borsh::de::BorshDeserialize for OneOrZero {
    ///     fn deserialize_reader<R: borsh::maybestd::io::Read>(
    ///         reader: &mut R,
    ///     ) -> borsh::maybestd::io::Result<Self> {
    ///         use borsh::de::EnumExt;
    ///         let tag = u8::deserialize_reader(reader)?;
    ///         if tag == 2 {
    ///             Err(borsh::maybestd::io::Error::new(
    ///                 borsh::maybestd::io::ErrorKind::InvalidInput,
    ///                 "MyEnum::Many not allowed here",
    ///             ))
    ///         } else {
    ///             MyEnum::deserialize_variant(reader, tag).map(Self)
    ///         }
    ///     }
    /// }
    ///
    /// let data = b"\0";
    /// assert_eq!(MyEnum::Zero, MyEnum::try_from_slice(&data[..]).unwrap());
    /// assert_eq!(MyEnum::Zero, OneOrZero::try_from_slice(&data[..]).unwrap().0);
    ///
    /// let data = b"\x02\0\0\0\0";
    /// assert_eq!(MyEnum::Many(Vec::new()), MyEnum::try_from_slice(&data[..]).unwrap());
    /// assert!(OneOrZero::try_from_slice(&data[..]).is_err());
    /// ```
    fn deserialize_variant<R: Read>(reader: &mut R, tag: u8) -> Result<Self>;
}

fn unexpected_eof_to_unexpected_length_of_input(e: Error) -> Error {
    if e.kind() == ErrorKind::UnexpectedEof {
        Error::new(ErrorKind::InvalidInput, ERROR_UNEXPECTED_LENGTH_OF_INPUT)
    } else {
        e
    }
}

#[async_trait::async_trait]
impl AsyncBorshDeserialize for u8 {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        reader
            .read_u8()
            .await
            .map_err(unexpected_eof_to_unexpected_length_of_input)
    }

    #[inline]
    #[doc(hidden)]
    async fn vec_from_reader<R: AsyncReader>(
        len: u32,
        reader: &mut R,
    ) -> Result<Option<Vec<Self>>> {
        let len: usize = len.try_into().map_err(|_| ErrorKind::InvalidInput)?;
        // Avoid OOM by limiting the size of allocation.  This makes the read
        // less efficient (since we need to loop and reallocate) but it protects
        // us from someone sending us [0xff, 0xff, 0xff, 0xff] and forcing us to
        // allocate 4GiB of memory.
        let mut vec = vec![0u8; len.min(1024 * 1024)];
        let mut pos = 0;
        while pos < len {
            if pos == vec.len() {
                vec.resize(vec.len().saturating_mul(2).min(len), 0)
            }
            // TODO(mina86): Convert this to read_buf once that stabilises.
            match reader.read(&mut vec.as_mut_slice()[pos..]).await? {
                0 => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
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
    async fn array_from_reader<R: AsyncReader, const N: usize>(
        reader: &mut R,
    ) -> Result<Option<[Self; N]>> {
        let mut arr = [0u8; N];
        reader
            .read_exact(&mut arr)
            .await
            .map_err(unexpected_eof_to_unexpected_length_of_input)?;
        Ok(Some(arr))
    }
}

macro_rules! impl_for_integer {
    ($type: ident) => {
        #[async_trait::async_trait]
        impl AsyncBorshDeserialize for $type {
            #[inline]
            async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
                let mut buf = [0u8; size_of::<$type>()];
                reader
                    .read_exact(&mut buf)
                    .await
                    .map_err(unexpected_eof_to_unexpected_length_of_input)?;
                let res = $type::from_le_bytes(buf.try_into().unwrap());
                Ok(res)
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
        impl AsyncBorshDeserialize for $type {
            #[inline]
            async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
                <$type>::new(AsyncBorshDeserialize::deserialize_reader(reader).await?)
                    .ok_or_else(|| Error::new(ErrorKind::InvalidData, ERROR_INVALID_ZERO_VALUE))
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
impl AsyncBorshDeserialize for isize {
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let i: i64 = AsyncBorshDeserialize::deserialize_reader(reader).await?;
        let i = isize::try_from(i).map_err(|_| {
            Error::new(
                ErrorKind::InvalidInput,
                ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_ISIZE,
            )
        })?;
        Ok(i)
    }
}

#[async_trait::async_trait]
impl AsyncBorshDeserialize for usize {
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let u: u64 = AsyncBorshDeserialize::deserialize_reader(reader).await?;
        let u = usize::try_from(u).map_err(|_| {
            Error::new(
                ErrorKind::InvalidInput,
                ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_USIZE,
            )
        })?;
        Ok(u)
    }
}

// Note NaNs have a portability issue. Specifically, signalling NaNs on MIPS are quiet NaNs on x86,
// and vice-versa. We disallow NaNs to avoid this issue.
macro_rules! impl_for_float {
    ($type: ident, $int_type: ident) => {
        #[async_trait::async_trait]
        impl AsyncBorshDeserialize for $type {
            #[inline]
            async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
                let mut buf = [0u8; size_of::<$type>()];
                reader
                    .read_exact(&mut buf)
                    .await
                    .map_err(unexpected_eof_to_unexpected_length_of_input)?;
                let res = $type::from_bits($int_type::from_le_bytes(buf.try_into().unwrap()));
                if res.is_nan() {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "For portability reasons we do not allow to deserialize NaNs.",
                    ));
                }
                Ok(res)
            }
        }
    };
}

impl_for_float!(f32, u32);
impl_for_float!(f64, u64);

#[async_trait::async_trait]
impl AsyncBorshDeserialize for bool {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let b: u8 = AsyncBorshDeserialize::deserialize_reader(reader).await?;
        if b == 0 {
            Ok(false)
        } else if b == 1 {
            Ok(true)
        } else {
            let msg = format!("Invalid bool representation: {}", b);

            Err(Error::new(ErrorKind::InvalidInput, msg))
        }
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshDeserialize for core::ops::Range<T>
where
    T: AsyncBorshDeserialize + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        Ok(Self {
            start: T::deserialize_reader(reader).await?,
            end: T::deserialize_reader(reader).await?,
        })
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshDeserialize for Option<T>
where
    T: AsyncBorshDeserialize,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let flag: u8 = AsyncBorshDeserialize::deserialize_reader(reader).await?;
        if flag == 0 {
            Ok(None)
        } else if flag == 1 {
            Ok(Some(T::deserialize_reader(reader).await?))
        } else {
            let msg = format!(
                "Invalid Option representation: {}. The first byte must be 0 or 1",
                flag
            );

            Err(Error::new(ErrorKind::InvalidInput, msg))
        }
    }
}

#[async_trait::async_trait]
impl<T, E> AsyncBorshDeserialize for core::result::Result<T, E>
where
    T: AsyncBorshDeserialize,
    E: AsyncBorshDeserialize,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let flag: u8 = AsyncBorshDeserialize::deserialize_reader(reader).await?;
        if flag == 0 {
            Ok(Err(E::deserialize_reader(reader).await?))
        } else if flag == 1 {
            Ok(Ok(T::deserialize_reader(reader).await?))
        } else {
            let msg = format!(
                "Invalid Result representation: {}. The first byte must be 0 or 1",
                flag
            );

            Err(Error::new(ErrorKind::InvalidInput, msg))
        }
    }
}

#[async_trait::async_trait]
impl AsyncBorshDeserialize for String {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        String::from_utf8(Vec::<u8>::deserialize_reader(reader).await?).map_err(|err| {
            let msg = err.to_string();
            Error::new(ErrorKind::InvalidData, msg)
        })
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshDeserialize for Vec<T>
where
    T: AsyncBorshDeserialize + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let len = u32::deserialize_reader(reader).await?;
        if len == 0 {
            Ok(Vec::new())
        } else if let Some(vec_bytes) = T::vec_from_reader(len, reader).await? {
            Ok(vec_bytes)
        } else if size_of::<T>() == 0 {
            let mut result = vec![T::deserialize_reader(reader).await?];

            let p = result.as_mut_ptr();
            unsafe {
                forget(result);
                let len = len.try_into().map_err(|_| ErrorKind::InvalidInput)?;
                let result = Vec::from_raw_parts(p, len, len);
                Ok(result)
            }
        } else {
            // TODO(16): return capacity allocation when we can safely do that.
            let mut result = Vec::with_capacity(hint::cautious::<T>(len));
            for _ in 0..len {
                result.push(T::deserialize_reader(reader).await?);
            }
            Ok(result)
        }
    }
}

#[cfg(any(test, feature = "bytes"))]
#[async_trait::async_trait]
impl AsyncBorshDeserialize for bytes::Bytes {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let vec = <Vec<u8>>::deserialize_reader(reader).await?;
        Ok(vec.into())
    }
}

#[cfg(any(test, feature = "bytes"))]
#[async_trait::async_trait]
impl AsyncBorshDeserialize for bytes::BytesMut {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let len = u32::deserialize_reader(reader).await?;
        let mut out = BytesMut::with_capacity(hint::cautious::<u8>(len));
        for _ in 0..len {
            out.put_u8(u8::deserialize_reader(reader).await?);
        }
        Ok(out)
    }
}

#[cfg(any(test, feature = "bson"))]
#[async_trait::async_trait]
impl AsyncBorshDeserialize for bson::oid::ObjectId {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 12];
        reader.read_exact(&mut buf).await?;
        Ok(bson::oid::ObjectId::from_bytes(buf))
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshDeserialize for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: AsyncBorshDeserialize,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        Ok(Cow::Owned(
            AsyncBorshDeserialize::deserialize_reader(reader).await?,
        ))
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshDeserialize for VecDeque<T>
where
    T: AsyncBorshDeserialize + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let vec = <Vec<T>>::deserialize_reader(reader).await?;
        Ok(vec.into())
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshDeserialize for LinkedList<T>
where
    T: AsyncBorshDeserialize + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let vec = <Vec<T>>::deserialize_reader(reader).await?;
        Ok(vec.into_iter().collect::<LinkedList<T>>())
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshDeserialize for BinaryHeap<T>
where
    T: AsyncBorshDeserialize + Ord + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let vec = <Vec<T>>::deserialize_reader(reader).await?;
        Ok(vec.into_iter().collect::<BinaryHeap<T>>())
    }
}

#[async_trait::async_trait]
impl<T, H> AsyncBorshDeserialize for HashSet<T, H>
where
    T: AsyncBorshDeserialize + Eq + Hash + Send,
    H: BuildHasher + Default,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let vec = <Vec<T>>::deserialize_reader(reader).await?;
        Ok(vec.into_iter().collect::<HashSet<T, H>>())
    }
}

#[async_trait::async_trait]
impl<K, V, H> AsyncBorshDeserialize for HashMap<K, V, H>
where
    K: AsyncBorshDeserialize + Eq + Hash + Send,
    V: AsyncBorshDeserialize + Send,
    H: BuildHasher + Default + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let len = u32::deserialize_reader(reader).await?;
        // TODO(16): return capacity allocation when we can safely do that.
        let mut result = HashMap::with_hasher(H::default());
        for _ in 0..len {
            let key = K::deserialize_reader(reader).await?;
            let value = V::deserialize_reader(reader).await?;
            result.insert(key, value);
        }
        Ok(result)
    }
}

#[async_trait::async_trait]
impl<T> AsyncBorshDeserialize for BTreeSet<T>
where
    T: AsyncBorshDeserialize + Ord + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let vec = <Vec<T>>::deserialize_reader(reader).await?;
        Ok(vec.into_iter().collect::<BTreeSet<T>>())
    }
}

#[async_trait::async_trait]
impl<K, V> AsyncBorshDeserialize for BTreeMap<K, V>
where
    K: AsyncBorshDeserialize + Ord + core::hash::Hash + Send,
    V: AsyncBorshDeserialize + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let len = u32::deserialize_reader(reader).await?;
        let mut result = BTreeMap::new();
        for _ in 0..len {
            let key = K::deserialize_reader(reader).await?;
            let value = V::deserialize_reader(reader).await?;
            result.insert(key, value);
        }
        Ok(result)
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshDeserialize for std::net::SocketAddr {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let kind = u8::deserialize_reader(reader).await?;
        match kind {
            0 => std::net::SocketAddrV4::deserialize_reader(reader)
                .await
                .map(std::net::SocketAddr::V4),
            1 => std::net::SocketAddrV6::deserialize_reader(reader)
                .await
                .map(std::net::SocketAddr::V6),
            value => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Invalid SocketAddr variant: {}", value),
            )),
        }
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshDeserialize for std::net::SocketAddrV4 {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let ip = std::net::Ipv4Addr::deserialize_reader(reader).await?;
        let port = u16::deserialize_reader(reader).await?;
        Ok(std::net::SocketAddrV4::new(ip, port))
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshDeserialize for std::net::SocketAddrV6 {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let ip = std::net::Ipv6Addr::deserialize_reader(reader).await?;
        let port = u16::deserialize_reader(reader).await?;
        Ok(std::net::SocketAddrV6::new(ip, port, 0, 0))
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshDeserialize for std::net::Ipv4Addr {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 4];
        reader
            .read_exact(&mut buf)
            .await
            .map_err(unexpected_eof_to_unexpected_length_of_input)?;
        Ok(std::net::Ipv4Addr::from(buf))
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl AsyncBorshDeserialize for std::net::Ipv6Addr {
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 16];
        reader
            .read_exact(&mut buf)
            .await
            .map_err(unexpected_eof_to_unexpected_length_of_input)?;
        Ok(std::net::Ipv6Addr::from(buf))
    }
}

#[async_trait::async_trait]
impl<T, U> AsyncBorshDeserialize for Box<T>
where
    U: Into<Box<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: AsyncBorshDeserialize,
{
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        Ok(T::Owned::deserialize_reader(reader).await?.into())
    }
}

#[async_trait::async_trait]
impl<T, const N: usize> AsyncBorshDeserialize for [T; N]
where
    T: AsyncBorshDeserialize + Send,
{
    #[inline]
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        struct ArrayDropGuard<'r, T: AsyncBorshDeserialize, const N: usize, R: AsyncReader> {
            buffer: [MaybeUninit<T>; N],
            init_count: usize,
            reader: &'r mut R,
        }
        impl<'r, T: AsyncBorshDeserialize, const N: usize, R: AsyncReader> Drop
            for ArrayDropGuard<'r, T, N, R>
        {
            fn drop(&mut self) {
                let init_range = &mut self.buffer[..self.init_count];
                // SAFETY: Elements up to self.init_count have been initialized. Assumes this value
                //         is only incremented in `fill_buffer`, which writes the element before
                //         increasing the init_count.
                unsafe {
                    core::ptr::drop_in_place(init_range as *mut _ as *mut [T]);
                };
            }
        }

        impl<'r, T: AsyncBorshDeserialize, const N: usize, R: AsyncReader> ArrayDropGuard<'r, T, N, R> {
            unsafe fn transmute_to_array(mut self) -> [T; N] {
                debug_assert_eq!(self.init_count, N);
                // Set init_count to 0 so that the values do not get dropped twice.
                self.init_count = 0;
                // SAFETY: This cast is required because `mem::transmute` does not work with
                //         const generics https://github.com/rust-lang/rust/issues/61956. This
                //         array is guaranteed to be initialized by this point.
                core::ptr::read(&self.buffer as *const _ as *const [T; N])
            }
            async fn fill_buffer(&mut self) -> Result<()> {
                // TODO: replace with `core::array::try_from_fn` when stabilized to avoid manually
                // dropping uninitialized values through the guard drop.
                for elem in self.buffer.iter_mut() {
                    elem.write(T::deserialize_reader(self.reader).await?);
                    self.init_count += 1;
                }
                Ok(())
            }
        }

        if let Some(arr) = T::array_from_reader(reader).await? {
            Ok(arr)
        } else {
            let mut result = ArrayDropGuard {
                buffer: unsafe { MaybeUninit::uninit().assume_init() },
                init_count: 0,
                reader,
            };

            result.fill_buffer().await?;

            // SAFETY: The elements up to `i` have been initialized in `fill_buffer`.
            Ok(unsafe { result.transmute_to_array() })
        }
    }
}

#[cfg(test)]
#[tokio::test]
async fn array_deserialization_doesnt_leak() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static DESERIALIZE_COUNT: AtomicUsize = AtomicUsize::new(0);
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct MyType(u8);
    #[async_trait::async_trait]
    impl AsyncBorshDeserialize for MyType {
        async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
            let val = u8::deserialize_reader(reader).await?;
            let v = DESERIALIZE_COUNT.fetch_add(1, Ordering::SeqCst);
            if v >= 7 {
                panic!("panic in deserialize");
            }
            Ok(MyType(val))
        }
    }
    impl Drop for MyType {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    assert!(
        <[MyType; 5] as AsyncBorshDeserialize>::deserialize(&mut &[0u8; 3][..])
            .await
            .is_err()
    );
    assert_eq!(DESERIALIZE_COUNT.load(Ordering::SeqCst), 3);
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 3);

    assert!(
        <[MyType; 2] as AsyncBorshDeserialize>::deserialize(&mut &[0u8; 2][..])
            .await
            .is_ok()
    );
    assert_eq!(DESERIALIZE_COUNT.load(Ordering::SeqCst), 5);
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 5);

    /* TODO: Find a way to catch panic from async functions
    #[cfg(feature = "std")]
    {
        // Test that during a panic in deserialize, the values are still dropped.
        let result = std::panic::catch_unwind(|| {
            <[MyType; 3] as AsyncBorshDeserialize>::deserialize(&mut &[0u8; 3][..])
                .await
                .unwrap();
        });
        assert!(result.is_err());
        assert_eq!(DESERIALIZE_COUNT.load(Ordering::SeqCst), 8);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 7); // 5 because 6 panicked and was not init
    } */
}

#[async_trait::async_trait]
impl AsyncBorshDeserialize for () {
    async fn deserialize_reader<R: AsyncReader>(_reader: &mut R) -> Result<Self> {
        Ok(())
    }
}

macro_rules! impl_tuple {
    ($($name:ident)+) => {
      #[async_trait::async_trait]
      impl<$($name),+> AsyncBorshDeserialize for ($($name,)+)
      where $($name: AsyncBorshDeserialize + Send,)+
      {
        #[inline]
        async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {

            Ok(($($name::deserialize_reader(reader).await?,)+))
        }
      }
    };
}

impl_tuple!(T0);
impl_tuple!(T0 T1);
impl_tuple!(T0 T1 T2);
impl_tuple!(T0 T1 T2 T3);
impl_tuple!(T0 T1 T2 T3 T4);
impl_tuple!(T0 T1 T2 T3 T4 T5);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16 T17);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16 T17 T18);
impl_tuple!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15 T16 T17 T18 T19);

#[cfg(feature = "rc")]
#[async_trait::async_trait]
impl<T, U> AsyncBorshDeserialize for Rc<T>
where
    U: Into<Rc<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: AsyncBorshDeserialize,
{
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        Ok(T::Owned::deserialize_reader(reader).await?.into())
    }
}

#[cfg(feature = "rc")]
#[async_trait::async_trait]
impl<T, U> AsyncBorshDeserialize for Arc<T>
where
    U: Into<Arc<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: AsyncBorshDeserialize,
{
    async fn deserialize_reader<R: AsyncReader>(reader: &mut R) -> Result<Self> {
        Ok(T::Owned::deserialize_reader(reader).await?.into())
    }
}

#[async_trait::async_trait]
impl<T: ?Sized> AsyncBorshDeserialize for PhantomData<T> {
    async fn deserialize_reader<R: AsyncReader>(_: &mut R) -> Result<Self> {
        Ok(Self::default())
    }
}
