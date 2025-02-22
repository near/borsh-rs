#[cfg(feature = "unstable__async")]
use core::future::Future;
use core::{
    convert::{TryFrom, TryInto},
    marker::PhantomData,
    mem::{size_of, MaybeUninit},
};

use async_generic::async_generic;
#[cfg(feature = "bytes")]
use bytes::{BufMut, BytesMut};

#[cfg(feature = "unstable__async")]
use crate::async_io::AsyncRead;
use crate::{
    __private::maybestd::{
        borrow::{Borrow, Cow, ToOwned},
        boxed::Box,
        collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
        format,
        string::String,
        vec,
        vec::Vec,
    },
    error::check_zst,
    io::{Error, ErrorKind, Read, Result},
};

mod hint;

const ERROR_NOT_ALL_BYTES_READ: &str = "Not all bytes read";
const ERROR_UNEXPECTED_LENGTH_OF_INPUT: &str = "Unexpected length of input";
const ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_ISIZE: &str = "Overflow on machine with 32 bit isize";
const ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_USIZE: &str = "Overflow on machine with 32 bit usize";
const ERROR_INVALID_ZERO_VALUE: &str = "Expected a non-zero value";

#[cfg(feature = "de_strict_order")]
const ERROR_WRONG_ORDER_OF_KEYS: &str = "keys were not serialized in ascending order";

/// A data-structure that can be de-serialized from binary format by NBOR.
pub trait BorshDeserialize: Sized {
    /// Deserializes this instance from a given slice of bytes.
    /// Updates the buffer to point at the remaining bytes.
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        Self::deserialize_reader(&mut *buf)
    }

    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self>;

    /// Deserialize this instance from a slice of bytes.
    fn try_from_slice(v: &[u8]) -> Result<Self> {
        let mut v_mut = v;
        let result = Self::deserialize(&mut v_mut)?;
        if !v_mut.is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ));
        }
        Ok(result)
    }

    fn try_from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let result = Self::deserialize_reader(reader)?;

        let mut buf = [0u8; 1];
        match reader.read_exact(&mut buf) {
            Err(f) if f.kind() == ErrorKind::UnexpectedEof => Ok(result),
            _ => Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ)),
        }
    }

    #[inline]
    #[doc(hidden)]
    fn vec_from_reader<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>> {
        let _ = len;
        let _ = reader;
        Ok(None)
    }

    #[inline]
    #[doc(hidden)]
    fn array_from_reader<R: Read, const N: usize>(reader: &mut R) -> Result<Option<[Self; N]>> {
        let _ = reader;
        Ok(None)
    }
}

/// A data-structure that can be asynchronously de-serialized from binary format by NBOR.
#[cfg(feature = "unstable__async")]
pub trait BorshDeserializeAsync: Sized + Send {
    fn deserialize_reader<R: AsyncRead>(
        reader: &mut R,
    ) -> impl Future<Output = Result<Self>> + Send;

    fn try_from_reader<R: AsyncRead>(reader: &mut R) -> impl Future<Output = Result<Self>> + Send {
        async move {
            let result = { Self::deserialize_reader(reader).await }?;

            let mut buf = [0u8; 1];
            let res = reader.read_exact(&mut buf);
            let res = { res.await };
            match res {
                Err(f) if f.kind() == ErrorKind::UnexpectedEof => Ok(result),
                _ => Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ)),
            }
        }
    }

    #[inline]
    #[doc(hidden)]
    fn vec_from_reader<R: AsyncRead>(
        len: u32,
        reader: &mut R,
    ) -> impl Future<Output = Result<Option<Vec<Self>>>> + Send {
        let _ = len;
        let _ = reader;
        core::future::ready(Ok(None))
    }

    #[inline]
    #[doc(hidden)]
    fn array_from_reader<R: AsyncRead, const N: usize>(
        reader: &mut R,
    ) -> impl Future<Output = Result<Option<[Self; N]>>> + Send {
        let _ = reader;
        core::future::ready(Ok(None))
    }
}

/// Additional methods offered on enums which is used by `[derive(BorshDeserialize)]`.
pub trait EnumExt: BorshDeserialize {
    /// Deserialises given variant of an enum from the reader.
    ///
    /// This may be used to perform validation or filtering based on what
    /// variant is being deserialized.
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

/// Additional methods offered on enums which is used by `[derive(BorshDeserializeAsync)]`.
#[cfg(feature = "unstable__async")]
pub trait EnumExtAsync: BorshDeserializeAsync {
    /// Deserialises given variant of an enum from the async reader.
    ///
    /// This may be used to perform validation or filtering based on what
    /// variant is being deserialized.
    ///
    /// ```
    /// # tokio_test::block_on(async {
    /// use borsh::BorshDeserializeAsync;
    /// use borsh::de::EnumExtAsync as _;
    ///
    /// /// derive is only available if borsh is built with `features = ["derive"]`
    /// # #[cfg(feature = "derive")]
    /// #[derive(Debug, PartialEq, Eq, BorshDeserializeAsync)]
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
    /// impl borsh::de::BorshDeserializeAsync for OneOrZero {
    ///     async fn deserialize_reader<R: borsh::async_io::AsyncRead>(
    ///         reader: &mut R,
    ///     ) -> borsh::io::Result<Self> {
    ///         use borsh::de::EnumExtAsync;
    ///         let tag = u8::deserialize_reader(reader).await?;
    ///         if tag == 2 {
    ///             Err(borsh::io::Error::new(
    ///                 borsh::io::ErrorKind::InvalidData,
    ///                 "MyEnum::Many not allowed here",
    ///             ))
    ///         } else {
    ///             MyEnum::deserialize_variant(reader, tag).await.map(Self)
    ///         }
    ///     }
    /// }
    ///
    /// use borsh::from_reader_async;
    /// let data = b"\0";
    /// # #[cfg(all(feature = "derive", any(feature = "unstable__tokio", feature = "unstable__async-std")))]
    /// assert_eq!(MyEnum::Zero, from_reader_async::<_, MyEnum>(&mut &data[..]).await.unwrap());
    /// # #[cfg(all(feature = "derive", any(feature = "unstable__tokio", feature = "unstable__async-std")))]
    /// assert_eq!(MyEnum::Zero, from_reader_async::<_, OneOrZero>(&mut &data[..]).await.unwrap().0);
    ///
    /// let data = b"\x02\0\0\0\0";
    /// # #[cfg(all(feature = "derive", any(feature = "unstable__tokio", feature = "unstable__async-std")))]
    /// assert_eq!(MyEnum::Many(Vec::new()), from_reader_async::<_, MyEnum>(&mut &data[..]).await.unwrap());
    /// # #[cfg(all(feature = "derive", any(feature = "unstable__tokio", feature = "unstable__async-std")))]
    /// assert!(from_reader_async::<_, OneOrZero>(&mut &data[..]).await.is_err());
    /// # });
    /// ```
    fn deserialize_variant<R: AsyncRead>(
        reader: &mut R,
        tag: u8,
    ) -> impl Future<Output = Result<Self>> + Send;
}

fn unexpected_eof_to_unexpected_length_of_input(e: Error) -> Error {
    if e.kind() == ErrorKind::UnexpectedEof {
        Error::new(ErrorKind::InvalidData, ERROR_UNEXPECTED_LENGTH_OF_INPUT)
    } else {
        e
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for u8 {
    #[inline]
    #[async_generic(
        async_signature[impl_fut]<R: AsyncRead>(reader: &mut R) -> impl Future<Output=Result<Self>> + Send
    )]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        if _sync {
            let mut buf = [0u8; 1];
            reader
                .read_exact(&mut buf)
                .map_err(unexpected_eof_to_unexpected_length_of_input)?;
            Ok(buf[0])
        } else {
            reader.read_u8()
        }
    }

    #[inline]
    #[doc(hidden)]
    #[async_generic(
        async_signature<R: AsyncRead>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>>
    )]
    fn vec_from_reader<R: Read>(len: u32, reader: &mut R) -> Result<Option<Vec<Self>>> {
        let len: usize = len.try_into().map_err(|_| ErrorKind::InvalidData)?;
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
            let res = {
                let res = reader.read(&mut vec.as_mut_slice()[pos..]);
                if _sync { res } else { res.await }?
            };
            match res {
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
    #[async_generic(
        async_signature<R: AsyncRead, const N: usize>(reader: &mut R) -> Result<Option<[Self; N]>>
    )]
    fn array_from_reader<R: Read, const N: usize>(reader: &mut R) -> Result<Option<[Self; N]>> {
        let mut arr = [0u8; N];
        let res = reader.read_exact(&mut arr);
        if _sync { res } else { res.await }
            .map_err(unexpected_eof_to_unexpected_length_of_input)?;
        Ok(Some(arr))
    }
}

macro_rules! impl_for_integer {
    ($type: ident, $method: ident) => {
        #[async_generic(
            #[cfg(feature = "unstable__async")]
            async_variant
        )]
        impl BorshDeserialize for $type {
            #[inline]
            #[async_generic(
                async_signature[impl_fut]<R: AsyncRead>(reader: &mut R) -> impl Future<Output=Result<Self>> + Send
            )]
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                if _sync {
                    let mut buf = [0u8; size_of::<$type>()];
                    reader
                        .read_exact(&mut buf)
                        .map_err(unexpected_eof_to_unexpected_length_of_input)?;
                    let res = $type::from_le_bytes(buf.try_into().unwrap());
                    Ok(res)
                } else {
                    reader.$method()
                }
            }
        }
    };
}

impl_for_integer!(i8, read_i8);
impl_for_integer!(i16, read_i16);
impl_for_integer!(i32, read_i32);
impl_for_integer!(i64, read_i64);
impl_for_integer!(i128, read_i128);
impl_for_integer!(u16, read_u16);
impl_for_integer!(u32, read_u32);
impl_for_integer!(u64, read_u64);
impl_for_integer!(u128, read_u128);

#[rustfmt::skip]
macro_rules! impl_for_nonzero_integer {
    ($type: ty, $repr: ty) => {
        #[async_generic(
            #[cfg(feature = "unstable__async")]
            async_variant
        )]
        impl BorshDeserialize for $type {
            #[inline]
            #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                <$type>::new(if _sync {
                    <$repr as BorshDeserialize>::deserialize_reader(reader)
                } else {
                    <$repr as BorshDeserializeAsync>::deserialize_reader(reader).await
                }?)
                .ok_or_else(|| Error::new(ErrorKind::InvalidData, ERROR_INVALID_ZERO_VALUE))
            }
        }
    };
}

impl_for_nonzero_integer!(core::num::NonZeroI8, i8);
impl_for_nonzero_integer!(core::num::NonZeroI16, i16);
impl_for_nonzero_integer!(core::num::NonZeroI32, i32);
impl_for_nonzero_integer!(core::num::NonZeroI64, i64);
impl_for_nonzero_integer!(core::num::NonZeroI128, i128);
impl_for_nonzero_integer!(core::num::NonZeroIsize, isize);
impl_for_nonzero_integer!(core::num::NonZeroU8, u8);
impl_for_nonzero_integer!(core::num::NonZeroU16, u16);
impl_for_nonzero_integer!(core::num::NonZeroU32, u32);
impl_for_nonzero_integer!(core::num::NonZeroU64, u64);
impl_for_nonzero_integer!(core::num::NonZeroU128, u128);
impl_for_nonzero_integer!(core::num::NonZeroUsize, usize);

#[rustfmt::skip]
macro_rules! impl_for_size_integer {
    ($type: ty: $temp_type: ty, $msg: expr) => {
        #[async_generic(
            #[cfg(feature = "unstable__async")]
            async_variant
        )]
        impl BorshDeserialize for $type {
            #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                let i = if _sync {
                    <$temp_type as BorshDeserialize>::deserialize_reader(reader)
                } else {
                    <$temp_type as BorshDeserializeAsync>::deserialize_reader(reader).await
                }?;
                let i =
                    <$type>::try_from(i).map_err(|_| Error::new(ErrorKind::InvalidData, $msg))?;
                Ok(i)
            }
        }
    };
}

impl_for_size_integer!(isize: i64, ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_ISIZE);
impl_for_size_integer!(usize: u64, ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_USIZE);

// Note NaNs have a portability issue. Specifically, signalling NaNs on MIPS are quiet NaNs on x86,
// and vice versa. We disallow NaNs to avoid this issue.
#[rustfmt::skip]
macro_rules! impl_for_float {
    ($type: ident, $int_type: ident, $method: ident) => {
        #[async_generic(
            #[cfg(feature = "unstable__async")]
            async_variant
        )]
        impl BorshDeserialize for $type {
            #[inline]
            #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                let res = if _sync {
                    let mut buf = [0u8; size_of::<$type>()];
                    reader
                        .read_exact(&mut buf)
                        .map_err(unexpected_eof_to_unexpected_length_of_input)?;
                    $type::from_bits($int_type::from_le_bytes(buf.try_into().unwrap()))
                } else {
                    reader.$method().await?
                };
                if res.is_nan() {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "For portability reasons we do not allow to deserialize NaNs.",
                    ));
                }
                Ok(res)
            }
        }
    };
}

impl_for_float!(f32, u32, read_f32);
impl_for_float!(f64, u64, read_f64);

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for bool {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let b = if _sync {
            <u8 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u8 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        match b {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid bool representation: {}", b),
            )),
        }
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T>
    where
        T: BorshDeserializeAsync,
)]
impl<T> BorshDeserialize for Option<T>
where
    T: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let flag = if _sync {
            <u8 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u8 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        match flag {
            0 => Ok(None),
            1 => Ok(Some({
                let res = T::deserialize_reader(reader);
                if _sync { res } else { res.await }?
            })),
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Invalid Option representation: {}. The first byte must be 0 or 1",
                    flag
                ),
            )),
        }
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T, E>
    where
        T: BorshDeserializeAsync,
        E: BorshDeserializeAsync,
)]
impl<T, E> BorshDeserialize for core::result::Result<T, E>
where
    T: BorshDeserialize,
    E: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let flag = if _sync {
            <u8 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u8 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        match flag {
            0 => Ok(Err({
                let res = E::deserialize_reader(reader);
                if _sync { res } else { res.await }?
            })),
            1 => Ok(Ok({
                let res = T::deserialize_reader(reader);
                if _sync { res } else { res.await }?
            })),
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Invalid Result representation: {}. The first byte must be 0 or 1",
                    flag
                ),
            )),
        }
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for String {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        String::from_utf8(if _sync {
            <Vec<u8> as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <Vec<u8> as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?)
        .map_err(|err| {
            #[cfg(feature = "std")]
            {
                Error::new(ErrorKind::InvalidData, err)
            }
            #[cfg(not(feature = "std"))]
            {
                use crate::__private::maybestd::string::ToString;
                Error::new(ErrorKind::InvalidData, err.to_string())
            }
        })
    }
}

/// Module is available if borsh is built with `features = ["ascii"]`.
#[cfg(feature = "ascii")]
pub mod ascii {
    //!
    //! Module defines [`BorshDeserialize`]
    #![cfg_attr(feature = "unstable__async", doc = " & [`BorshDeserializeAsync`]")]
    //! implementation for some types from [`ascii`] crate.

    use async_generic::async_generic;

    use super::BorshDeserialize;
    #[cfg(feature = "unstable__async")]
    use super::{AsyncRead, BorshDeserializeAsync};
    use crate::{
        __private::maybestd::vec::Vec,
        io::{Error, ErrorKind, Read, Result},
    };

    #[async_generic(
        #[cfg(feature = "unstable__async")]
        async_variant
    )]
    impl BorshDeserialize for ascii::AsciiString {
        #[inline]
        #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            let bytes = if _sync {
                <Vec<u8> as BorshDeserialize>::deserialize_reader(reader)
            } else {
                <Vec<u8> as BorshDeserializeAsync>::deserialize_reader(reader).await
            }?;
            ascii::AsciiString::from_ascii(bytes).map_err(|err| {
                #[cfg(feature = "std")]
                {
                    Error::new(ErrorKind::InvalidData, err)
                }
                #[cfg(not(feature = "std"))]
                {
                    use crate::__private::maybestd::string::ToString;
                    Error::new(ErrorKind::InvalidData, err.to_string())
                }
            })
        }
    }

    #[async_generic(
    #[cfg(feature = "unstable__async")]
        async_variant
    )]
    impl BorshDeserialize for ascii::AsciiChar {
        #[inline]
        #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            let byte = if _sync {
                <u8 as BorshDeserialize>::deserialize_reader(reader)
            } else {
                <u8 as BorshDeserializeAsync>::deserialize_reader(reader).await
            }?;
            ascii::AsciiChar::from_ascii(byte).map_err(|err| {
                #[cfg(feature = "std")]
                {
                    Error::new(ErrorKind::InvalidData, err)
                }
                #[cfg(not(feature = "std"))]
                {
                    use crate::__private::maybestd::string::ToString;
                    Error::new(ErrorKind::InvalidData, err.to_string())
                }
            })
        }
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T>
    where
        T: BorshDeserializeAsync,
)]
impl<T> BorshDeserialize for Vec<T>
where
    T: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        check_zst::<T>()?;

        let len = if _sync {
            <u32 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u32 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        if len == 0 {
            Ok(Vec::new())
        } else if let Some(vec_bytes) = {
            let res = T::vec_from_reader(len, reader);
            if _sync { res } else { res.await }?
        } {
            Ok(vec_bytes)
        } else {
            // TODO(16): return capacity allocation when we can safely do that.
            let mut result = Vec::with_capacity(hint::cautious::<T>(len));
            for _ in 0..len {
                result.push({
                    let res = T::deserialize_reader(reader);
                    if _sync { res } else { res.await }?
                });
            }
            Ok(result)
        }
    }
}

#[cfg(feature = "bytes")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for bytes::Bytes {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let vec = if _sync {
            <Vec<u8> as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <Vec<u8> as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        Ok(vec.into())
    }
}

#[cfg(feature = "bytes")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for BytesMut {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let len = if _sync {
            <u32 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u32 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        let mut out = BytesMut::with_capacity(hint::cautious::<u8>(len));
        for _ in 0..len {
            out.put_u8(if _sync {
                <u8 as BorshDeserialize>::deserialize_reader(reader)
            } else {
                <u8 as BorshDeserializeAsync>::deserialize_reader(reader).await
            }?);
        }
        Ok(out)
    }
}

#[cfg(feature = "bson")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for bson::oid::ObjectId {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 12];
        {
            let res = reader.read_exact(&mut buf);
            if _sync { res } else { res.await }?;
        }
        Ok(bson::oid::ObjectId::from_bytes(buf))
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T>
    where
        T: ToOwned + ?Sized,
        T::Owned: BorshDeserializeAsync,
        for<'a> &'a T: Send,
)]
impl<T> BorshDeserialize for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Cow::Owned({
            let res = T::Owned::deserialize_reader(reader);
            if _sync { res } else { res.await }?
        }))
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T>
    where
        T: BorshDeserializeAsync,
)]
impl<T> BorshDeserialize for VecDeque<T>
where
    T: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let vec = {
            let res = <Vec<T>>::deserialize_reader(reader);
            if _sync { res } else { res.await }?
        };
        Ok(vec.into())
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T>
    where
        T: BorshDeserializeAsync,
)]
impl<T> BorshDeserialize for LinkedList<T>
where
    T: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let vec = {
            let res = <Vec<T>>::deserialize_reader(reader);
            if _sync { res } else { res.await }?
        };
        Ok(vec.into_iter().collect::<LinkedList<T>>())
    }
}

/// Module is available if borsh is built with `features = ["std"]` or `features = ["hashbrown"]`.
///
/// Module defines [`BorshDeserialize`]
#[cfg_attr(feature = "unstable__async", doc = " & [`BorshDeserializeAsync`]")]
/// implementation for [`HashMap`](std::collections::HashMap)/[`HashSet`](std::collections::HashSet).
#[cfg(hash_collections)]
pub mod hashes {
    use core::hash::{BuildHasher, Hash};

    use async_generic::async_generic;

    use super::BorshDeserialize;
    #[cfg(feature = "unstable__async")]
    use super::{AsyncRead, BorshDeserializeAsync};
    #[cfg(feature = "de_strict_order")]
    use crate::io::{Error, ErrorKind};
    use crate::{
        __private::maybestd::{
            collections::{HashMap, HashSet},
            vec::Vec,
        },
        error::check_zst,
        io::{Read, Result},
    };

    #[cfg(feature = "de_strict_order")]
    const ERROR_WRONG_ORDER_OF_KEYS: &str = "keys were not serialized in ascending order";

    #[async_generic(
        #[cfg(feature = "unstable__async")]
        async_variant<T, H>
        where
            T: BorshDeserializeAsync + Eq + Hash + Ord,
            H: BuildHasher + Default + Send,
    )]
    impl<T, H> BorshDeserialize for HashSet<T, H>
    where
        T: BorshDeserialize + Eq + Hash + Ord,
        H: BuildHasher + Default,
    {
        #[inline]
        #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            // NOTE: deserialize-as-you-go approach as once was in HashSet is better in the sense
            // that it allows to fail early, and not allocate memory for all the elements
            // which may fail `cmp()` checks
            // NOTE: deserialize first to `Vec<T>` is faster
            let vec = {
                let res = <Vec<T>>::deserialize_reader(reader);
                if _sync { res } else { res.await }?
            };

            #[cfg(feature = "de_strict_order")]
            // TODO: replace with `is_sorted` api when stabilizes https://github.com/rust-lang/rust/issues/53485
            // TODO: first replace with `array_windows` api when stabilizes https://github.com/rust-lang/rust/issues/75027
            for pair in vec.windows(2) {
                let [a, b] = pair else {
                    unreachable!("`windows` always return a slice of length 2 or nothing");
                };
                let cmp_result = a.cmp(b).is_lt();
                if !cmp_result {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        ERROR_WRONG_ORDER_OF_KEYS,
                    ));
                }
            }

            Ok(vec.into_iter().collect::<HashSet<T, H>>())
        }
    }

    #[async_generic(
    #[cfg(feature = "unstable__async")]
        async_variant<K, V, H>
        where
            K: BorshDeserializeAsync + Eq + Hash + Ord,
            V: BorshDeserializeAsync,
            H: BuildHasher + Default + Send,
    )]
    impl<K, V, H> BorshDeserialize for HashMap<K, V, H>
    where
        K: BorshDeserialize + Eq + Hash + Ord,
        V: BorshDeserialize,
        H: BuildHasher + Default,
    {
        #[inline]
        #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            check_zst::<K>()?;
            // NOTE: deserialize-as-you-go approach as once was in HashSet is better in the sense
            // that it allows to fail early, and not allocate memory for all the entries
            // which may fail `cmp()` checks
            // NOTE: deserialize first to `Vec<(K, V)>` is faster
            let vec = {
                let res = <Vec<(K, V)>>::deserialize_reader(reader);
                if _sync { res } else { res.await }?
            };

            #[cfg(feature = "de_strict_order")]
            // TODO: replace with `is_sorted` api when stabilizes https://github.com/rust-lang/rust/issues/53485
            // TODO: first replace with `array_windows` api when stabilizes https://github.com/rust-lang/rust/issues/75027
            for pair in vec.windows(2) {
                let [(a_k, _a_v), (b_k, _b_v)] = pair else {
                    unreachable!("`windows` always return a slice of length 2 or nothing");
                };
                let cmp_result = a_k.cmp(b_k).is_lt();
                if !cmp_result {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        ERROR_WRONG_ORDER_OF_KEYS,
                    ));
                }
            }

            Ok(vec.into_iter().collect::<HashMap<K, V, H>>())
        }
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T>
    where
        T: BorshDeserializeAsync + Ord,
)]
impl<T> BorshDeserialize for BTreeSet<T>
where
    T: BorshDeserialize + Ord,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        // NOTE: deserialize-as-you-go approach as once was in HashSet is better in the sense
        // that it allows to fail early, and not allocate memory for all the elements
        // which may fail `cmp()` checks
        // NOTE: deserialize first to `Vec<T>` is faster
        let vec = {
            let res = <Vec<T>>::deserialize_reader(reader);
            if _sync { res } else { res.await }?
        };

        #[cfg(feature = "de_strict_order")]
        // TODO: replace with `is_sorted` api when stabilizes https://github.com/rust-lang/rust/issues/53485
        // TODO: first replace with `array_windows` api when stabilizes https://github.com/rust-lang/rust/issues/75027
        for pair in vec.windows(2) {
            let [a, b] = pair else {
                unreachable!("`windows` always return a slice of length 2 or nothing");
            };
            let cmp_result = a.cmp(b).is_lt();
            if !cmp_result {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    ERROR_WRONG_ORDER_OF_KEYS,
                ));
            }
        }
        // NOTE: BTreeSet has an optimization inside of impl <T> FromIterator<T> for BTreeSet<T, Global>,
        // based on BTreeMap::bulk_build_from_sorted_iter
        Ok(vec.into_iter().collect::<BTreeSet<T>>())
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<K, V>
    where
        K: BorshDeserializeAsync + Ord,
        V: BorshDeserializeAsync,
)]
impl<K, V> BorshDeserialize for BTreeMap<K, V>
where
    K: BorshDeserialize + Ord,
    V: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        check_zst::<K>()?;
        // NOTE: deserialize-as-you-go approach as once was in HashSet is better in the sense
        // that it allows to fail early, and not allocate memory for all the entries
        // which may fail `cmp()` checks
        // NOTE: deserialize first to `Vec<(K, V)>` is faster
        let vec = {
            let res = <Vec<(K, V)>>::deserialize_reader(reader);
            if _sync { res } else { res.await }?
        };

        #[cfg(feature = "de_strict_order")]
        // TODO: replace with `is_sorted` api when stabilizes https://github.com/rust-lang/rust/issues/53485
        // TODO: first replace with `array_windows` api when stabilizes https://github.com/rust-lang/rust/issues/75027
        for pair in vec.windows(2) {
            let [(a_k, _a_v), (b_k, _b_v)] = pair else {
                unreachable!("`windows` always return a slice of length 2 or nothing");
            };
            let cmp_result = a_k.cmp(b_k).is_lt();
            if !cmp_result {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    ERROR_WRONG_ORDER_OF_KEYS,
                ));
            }
        }

        // NOTE: BTreeMap has an optimization inside of impl<K, V> FromIterator<(K, V)> for BTreeMap<K, V, Global>,
        // based on BTreeMap::bulk_build_from_sorted_iter
        Ok(vec.into_iter().collect::<BTreeMap<K, V>>())
    }
}

#[cfg(feature = "std")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for std::net::SocketAddr {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let kind = if _sync {
            <u8 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u8 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        match kind {
            0 => if _sync {
                <std::net::SocketAddrV4 as BorshDeserialize>::deserialize_reader(reader)
            } else {
                <std::net::SocketAddrV4 as BorshDeserializeAsync>::deserialize_reader(reader).await
            }
            .map(std::net::SocketAddr::V4),
            1 => if _sync {
                <std::net::SocketAddrV6 as BorshDeserialize>::deserialize_reader(reader)
            } else {
                <std::net::SocketAddrV6 as BorshDeserializeAsync>::deserialize_reader(reader).await
            }
            .map(std::net::SocketAddr::V6),
            value => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid SocketAddr variant: {}", value),
            )),
        }
    }
}

#[cfg(feature = "std")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for std::net::IpAddr {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let kind = if _sync {
            <u8 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u8 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        match kind {
            0 => if _sync {
                <std::net::Ipv4Addr as BorshDeserialize>::deserialize_reader(reader)
            } else {
                <std::net::Ipv4Addr as BorshDeserializeAsync>::deserialize_reader(reader).await
            }
            .map(std::net::IpAddr::V4),
            1 => if _sync {
                <std::net::Ipv6Addr as BorshDeserialize>::deserialize_reader(reader)
            } else {
                <std::net::Ipv6Addr as BorshDeserializeAsync>::deserialize_reader(reader).await
            }
            .map(std::net::IpAddr::V6),
            value => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid IpAddr variant: {}", value),
            )),
        }
    }
}

#[cfg(feature = "std")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for std::net::SocketAddrV4 {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let ip = if _sync {
            <std::net::Ipv4Addr as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <std::net::Ipv4Addr as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        let port = if _sync {
            <u16 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u16 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        Ok(std::net::SocketAddrV4::new(ip, port))
    }
}

#[cfg(feature = "std")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for std::net::SocketAddrV6 {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let ip = if _sync {
            <std::net::Ipv6Addr as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <std::net::Ipv6Addr as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        let port = if _sync {
            <u16 as BorshDeserialize>::deserialize_reader(reader)
        } else {
            <u16 as BorshDeserializeAsync>::deserialize_reader(reader).await
        }?;
        Ok(std::net::SocketAddrV6::new(ip, port, 0, 0))
    }
}

#[cfg(feature = "std")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for std::net::Ipv4Addr {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 4];
        let res = reader.read_exact(&mut buf);
        if _sync { res } else { res.await }
            .map_err(unexpected_eof_to_unexpected_length_of_input)?;
        Ok(std::net::Ipv4Addr::from(buf))
    }
}

#[cfg(feature = "std")]
#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant
)]
impl BorshDeserialize for std::net::Ipv6Addr {
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf = [0u8; 16];
        let res = reader.read_exact(&mut buf);
        if _sync { res } else { res.await }
            .map_err(unexpected_eof_to_unexpected_length_of_input)?;
        Ok(std::net::Ipv6Addr::from(buf))
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T, U>
    where
        U: Into<Box<T>> + Borrow<T>,
        T: ToOwned<Owned = U> + ?Sized + Send,
        T::Owned: BorshDeserializeAsync,
)]
impl<T, U> BorshDeserialize for Box<T>
where
    U: Into<Box<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok({
            let res = T::Owned::deserialize_reader(reader);
            if _sync { res } else { res.await }?.into()
        })
    }
}

impl<T, const N: usize> BorshDeserialize for [T; N]
where
    T: BorshDeserialize,
{
    #[inline]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        struct ArrayDropGuard<T, const N: usize> {
            buffer: [MaybeUninit<T>; N],
            init_count: usize,
        }
        impl<T, const N: usize> Drop for ArrayDropGuard<T, N> {
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
        impl<T, const N: usize> ArrayDropGuard<T, N> {
            unsafe fn transmute_to_array(mut self) -> [T; N] {
                debug_assert_eq!(self.init_count, N);
                // Set init_count to 0 so that the values do not get dropped twice.
                self.init_count = 0;
                // SAFETY: This cast is required because `mem::transmute` does not work with
                //         const generics https://github.com/rust-lang/rust/issues/61956. This
                //         array is guaranteed to be initialized by this point.
                core::ptr::read(&self.buffer as *const _ as *const [T; N])
            }
            fn fill_buffer(&mut self, mut f: impl FnMut() -> Result<T>) -> Result<()> {
                // TODO: replace with `core::array::try_from_fn` when stabilized to avoid manually
                // dropping uninitialized values through the guard drop.
                for elem in self.buffer.iter_mut() {
                    elem.write(f()?);
                    self.init_count += 1;
                }
                Ok(())
            }
        }

        if let Some(arr) = T::array_from_reader(reader)? {
            Ok(arr)
        } else {
            let mut result = ArrayDropGuard {
                buffer: unsafe { MaybeUninit::uninit().assume_init() },
                init_count: 0,
            };

            result.fill_buffer(|| T::deserialize_reader(reader))?;

            // SAFETY: The elements up to `i` have been initialized in `fill_buffer`.
            Ok(unsafe { result.transmute_to_array() })
        }
    }
}

#[cfg(feature = "unstable__async")]
impl<T, const N: usize> BorshDeserializeAsync for [T; N]
where
    T: BorshDeserializeAsync,
{
    #[inline]
    async fn deserialize_reader<R: AsyncRead>(reader: &mut R) -> Result<Self> {
        struct ArrayDropGuard<'r, T: BorshDeserializeAsync, const N: usize, R: AsyncRead> {
            buffer: [MaybeUninit<T>; N],
            init_count: usize,
            reader: &'r mut R,
        }
        impl<'r, T: BorshDeserializeAsync, const N: usize, R: AsyncRead> Drop
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

        impl<'r, T: BorshDeserializeAsync, const N: usize, R: AsyncRead> ArrayDropGuard<'r, T, N, R> {
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

#[test]
fn array_deserialization_doesnt_leak() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static DESERIALIZE_COUNT: AtomicUsize = AtomicUsize::new(0);
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[allow(unused)]
    struct MyType(u8);

    #[async_generic(
    #[cfg(feature = "unstable__async")]
        async_variant
    )]
    impl BorshDeserialize for MyType {
        #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            let val = if _sync {
                <u8 as BorshDeserialize>::deserialize_reader(reader)
            } else {
                <u8 as BorshDeserializeAsync>::deserialize_reader(reader).await
            }?;
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

    assert!(<[MyType; 5] as BorshDeserialize>::deserialize(&mut &[0u8; 3][..]).is_err());
    assert_eq!(DESERIALIZE_COUNT.load(Ordering::SeqCst), 3);
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 3);

    assert!(<[MyType; 2] as BorshDeserialize>::deserialize(&mut &[0u8; 2][..]).is_ok());
    assert_eq!(DESERIALIZE_COUNT.load(Ordering::SeqCst), 5);
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 5);

    #[cfg(feature = "std")]
    {
        // Test that during a panic in deserialize, the values are still dropped.
        let result = std::panic::catch_unwind(|| {
            <[MyType; 3] as BorshDeserialize>::deserialize(&mut &[0u8; 3][..]).unwrap();
        });
        assert!(result.is_err());
        assert_eq!(DESERIALIZE_COUNT.load(Ordering::SeqCst), 8);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 7); // 5 because 6 panicked and was not init
    }
}

macro_rules! impl_tuple {
    (@unit $name:ty) => {
        #[async_generic(
            #[cfg(feature = "unstable__async")]
            async_variant
        )]
        impl BorshDeserialize for $name {
            #[inline]
            #[async_generic(
                async_signature[ready]<R: AsyncRead>(_: &mut R) -> impl Future<Output=Result<Self>> + Send
            )]
            fn deserialize_reader<R: Read>(_: &mut R) -> Result<Self> {
                Ok(<$name>::default())
            }
        }
    };

    ($($name:ident)+) => {
        #[async_generic(
            #[cfg(feature = "unstable__async")]
            async_variant<$($name),+>
            where
                $($name: BorshDeserializeAsync,)+
        )]
        impl<$($name),+> BorshDeserialize for ($($name,)+)
        where
            $($name: BorshDeserialize,)+
        {
            #[inline]
            #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                Ok(if _sync {
                    ($(<$name as BorshDeserialize>::deserialize_reader(reader)?,)+)
                } else {
                    ($(<$name as BorshDeserializeAsync>::deserialize_reader(reader).await?,)+)
                })
            }
        }
    };
}

impl_tuple!(@unit ());
impl_tuple!(@unit core::ops::RangeFull);

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

macro_rules! impl_range {
    ($type:ident, $make:expr, $n: literal, $($side:ident),*) => {
        #[async_generic(
            #[cfg(feature = "unstable__async")]
            async_variant<T>
            where
                T: BorshDeserializeAsync,
        )]
        impl<T: BorshDeserialize> BorshDeserialize for core::ops::$type<T> {
            #[inline]
            #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
            fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
                let [$($side,)*] = {
                    let res = <[T; $n]>::deserialize_reader(reader);
                    if _sync { res } else { res.await }?
                };
                Ok($make)
            }
        }
    };
}

impl_range!(Range, start..end, 2, start, end);
impl_range!(RangeInclusive, start..=end, 2, start, end);
impl_range!(RangeFrom, start.., 1, start);
impl_range!(RangeTo, ..end, 1, end);
impl_range!(RangeToInclusive, ..=end, 1, end);

/// Module is available if borsh is built with `features = ["rc"]`.
#[cfg(feature = "rc")]
pub mod rc {
    //!
    //! Module defines [`BorshDeserialize`]
    #![cfg_attr(feature = "unstable__async", doc = " & [`BorshDeserializeAsync`]")]
    //! implementation for [`alloc::rc::Rc`](Rc) and [`alloc::sync::Arc`](Arc).

    use async_generic::async_generic;

    use super::BorshDeserialize;
    #[cfg(feature = "unstable__async")]
    use super::{AsyncRead, BorshDeserializeAsync};
    use crate::{
        __private::maybestd::{boxed::Box, rc::Rc, sync::Arc},
        io::{Read, Result},
    };

    /// This impl requires the [`"rc"`] Cargo feature of borsh.
    ///
    /// Deserializing a data structure containing `Rc` will not attempt to
    /// deduplicate `Rc` references to the same data. Every deserialized `Rc`
    /// will end up with a strong count of 1.
    impl<T: ?Sized> BorshDeserialize for Rc<T>
    where
        Box<T>: BorshDeserialize,
    {
        #[inline]
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            Ok(<Box<T>>::deserialize_reader(reader)?.into())
        }
    }

    /// This impl requires the [`"rc"`] Cargo feature of borsh.
    ///
    /// Deserializing a data structure containing `Arc` will not attempt to
    /// deduplicate `Arc` references to the same data. Every deserialized `Arc`
    /// will end up with a strong count of 1.
    #[async_generic(
    #[cfg(feature = "unstable__async")]
        async_variant<T: ?Sized>
        where
            Box<T>: BorshDeserializeAsync,
            Self: Send,
    )]
    impl<T: ?Sized> BorshDeserialize for Arc<T>
    where
        Box<T>: BorshDeserialize,
    {
        #[inline]
        #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
        fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
            Ok({
                let res = <Box<T>>::deserialize_reader(reader);
                if _sync { res } else { res.await }?.into()
            })
        }
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T: ?Sized + Send>
)]
impl<T: ?Sized> BorshDeserialize for PhantomData<T> {
    #[inline]
    #[async_generic(
        async_signature[ready]<R: AsyncRead>(_: &mut R) -> impl Future<Output=Result<Self>> + Send
    )]
    fn deserialize_reader<R: Read>(_: &mut R) -> Result<Self> {
        Ok(PhantomData)
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T>
    where
        T: BorshDeserializeAsync,
)]
impl<T> BorshDeserialize for core::cell::Cell<T>
where
    T: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let res = T::deserialize_reader(reader);
        if _sync { res } else { res.await }.map(core::cell::Cell::new)
    }
}

#[async_generic(
    #[cfg(feature = "unstable__async")]
    async_variant<T>
    where
        T: BorshDeserializeAsync,
)]
impl<T> BorshDeserialize for core::cell::RefCell<T>
where
    T: BorshDeserialize,
{
    #[inline]
    #[async_generic(async_signature<R: AsyncRead>(reader: &mut R) -> Result<Self>)]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let res = T::deserialize_reader(reader);
        if _sync { res } else { res.await }.map(core::cell::RefCell::new)
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
/// # Panics
/// If the data is invalid, this function will panic.
/// # Errors
/// If the data is invalid, this function will return an error.
/// # Note
/// This function will return an error if the data is not fully read.
pub fn from_slice<T: BorshDeserialize>(v: &[u8]) -> Result<T> {
    let mut v_mut = v;
    let object = T::deserialize(&mut v_mut)?;
    if !v_mut.is_empty() {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ));
    }
    Ok(object)
}

/// Deserializes an object from a reader.
/// # Example
#[async_generic(
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
    sync_signature;

    /// ```
    /// use borsh::{BorshDeserializeAsync, BorshSerialize, from_reader_async, to_vec};
    ///
    /// /// derive is only available if borsh is built with `features = ["derive"]`
    /// # #[cfg(feature = "derive")]
    /// #[derive(BorshSerialize, BorshDeserializeAsync, PartialEq, Debug)]
    /// struct MyStruct {
    ///     a: u64,
    ///     b: Vec<u8>,
    /// }
    ///
    /// # tokio_test::block_on(async {
    /// # #[cfg(feature = "derive")]
    /// let original = MyStruct { a: 10, b: vec![1, 2, 3] };
    /// # #[cfg(feature = "derive")]
    /// let encoded = to_vec(&original).unwrap();
    /// # #[cfg(feature = "derive")]
    /// let decoded = from_reader_async::<_, MyStruct>(&mut encoded.as_slice()).await.unwrap();
    /// # #[cfg(feature = "derive")]
    /// assert_eq!(original, decoded);
    /// # });
    /// ```
    #[cfg(feature = "unstable__async")]
    async_signature[impl_fut]<R: AsyncRead, T: BorshDeserializeAsync>(
        reader: &mut R,
    ) -> impl Future<Output=Result<T>> + Send + Captures<&mut R>
)]
pub fn from_reader<R: Read, T: BorshDeserialize>(reader: &mut R) -> Result<T> {
    T::try_from_reader(reader)
}

#[cfg(feature = "unstable__async")]
use captures::Captures;
#[cfg(feature = "unstable__async")]
mod captures {
    /// This is a [trick](https://github.com/rust-lang/rfcs/blob/master/text/3498-lifetime-capture-rules-2024.md#the-captures-trick),
    /// used to not over-restrict the lifetime and trait bounds of a RPIT.
    ///
    /// Once the MSRV is >=1.82, this should be removed and replaced with `use<>`
    /// notation for precise capturing.
    #[doc(hidden)]
    pub trait Captures<T: ?Sized> {}

    impl<T: ?Sized, U: ?Sized> Captures<T> for U {}
}
