use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::{
    convert::{TryFrom, TryInto},
    hash::{BuildHasher, Hash},
    mem::{forget, size_of},
};

use crate::maybestd::{
    borrow::{Borrow, Cow, ToOwned},
    boxed::Box,
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque},
    format,
    io::{Error, ErrorKind, Result},
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

/// A data-structure that can be de-serialized from binary format by NBOR.
pub trait BorshDeserialize: Sized {
    /// Deserializes this instance from a given slice of bytes.
    /// Updates the buffer to point at the remaining bytes.
    fn deserialize(buf: &mut &[u8]) -> Result<Self>;

    /// Deserialize this instance from a slice of bytes.
    fn try_from_slice(v: &[u8]) -> Result<Self> {
        let mut v_mut = v;
        let result = Self::deserialize(&mut v_mut)?;
        if !v_mut.is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ));
        }
        Ok(result)
    }

    #[inline]
    #[doc(hidden)]
    fn vec_from_bytes(len: u32, buf: &mut &[u8]) -> Result<Option<Vec<Self>>> {
        let _ = len;
        let _ = buf;
        Ok(None)
    }

    #[inline]
    #[doc(hidden)]
    fn array_from_bytes<const N: usize>(buf: &mut &[u8]) -> Result<Option<[Self; N]>> {
        let _ = buf;
        Ok(None)
    }
}

impl BorshDeserialize for u8 {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let res = buf[0];
        *buf = &buf[1..];
        Ok(res)
    }

    #[inline]
    #[doc(hidden)]
    fn vec_from_bytes(len: u32, buf: &mut &[u8]) -> Result<Option<Vec<Self>>> {
        let len = len.try_into().map_err(|_| ErrorKind::InvalidInput)?;
        if buf.len() < len {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let (front, rest) = buf.split_at(len);
        *buf = rest;
        Ok(Some(front.to_vec()))
    }

    #[inline]
    #[doc(hidden)]
    fn array_from_bytes<const N: usize>(buf: &mut &[u8]) -> Result<Option<[Self; N]>> {
        if buf.len() < N {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let (front, rest) = buf.split_at(N);
        *buf = rest;
        let front: [u8; N] = front.try_into().unwrap();
        Ok(Some(front))
    }
}

macro_rules! impl_for_integer {
    ($type: ident) => {
        impl BorshDeserialize for $type {
            #[inline]
            fn deserialize(buf: &mut &[u8]) -> Result<Self> {
                if buf.len() < size_of::<$type>() {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        ERROR_UNEXPECTED_LENGTH_OF_INPUT,
                    ));
                }
                let res = $type::from_le_bytes(buf[..size_of::<$type>()].try_into().unwrap());
                *buf = &buf[size_of::<$type>()..];
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
        impl BorshDeserialize for $type {
            #[inline]
            fn deserialize(buf: &mut &[u8]) -> Result<Self> {
                <$type>::new(BorshDeserialize::deserialize(buf)?)
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

impl BorshDeserialize for isize {
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let i: i64 = BorshDeserialize::deserialize(buf)?;
        let i = isize::try_from(i).map_err(|_| {
            Error::new(
                ErrorKind::InvalidInput,
                ERROR_OVERFLOW_ON_MACHINE_WITH_32_BIT_ISIZE,
            )
        })?;
        Ok(i)
    }
}

impl BorshDeserialize for usize {
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let u: u64 = BorshDeserialize::deserialize(buf)?;
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
        impl BorshDeserialize for $type {
            #[inline]
            fn deserialize(buf: &mut &[u8]) -> Result<Self> {
                if buf.len() < size_of::<$type>() {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        ERROR_UNEXPECTED_LENGTH_OF_INPUT,
                    ));
                }
                let res = $type::from_bits($int_type::from_le_bytes(
                    buf[..size_of::<$int_type>()].try_into().unwrap(),
                ));
                *buf = &buf[size_of::<$int_type>()..];
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

impl BorshDeserialize for bool {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let b = buf[0];
        *buf = &buf[1..];
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

impl<T> BorshDeserialize for Option<T>
where
    T: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let flag = buf[0];
        *buf = &buf[1..];
        if flag == 0 {
            Ok(None)
        } else if flag == 1 {
            Ok(Some(T::deserialize(buf)?))
        } else {
            let msg = format!(
                "Invalid Option representation: {}. The first byte must be 0 or 1",
                flag
            );

            Err(Error::new(ErrorKind::InvalidInput, msg))
        }
    }
}

impl<T, E> BorshDeserialize for core::result::Result<T, E>
where
    T: BorshDeserialize,
    E: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let flag = buf[0];
        *buf = &buf[1..];
        if flag == 0 {
            Ok(Err(E::deserialize(buf)?))
        } else if flag == 1 {
            Ok(Ok(T::deserialize(buf)?))
        } else {
            let msg = format!(
                "Invalid Result representation: {}. The first byte must be 0 or 1",
                flag
            );

            Err(Error::new(ErrorKind::InvalidInput, msg))
        }
    }
}

impl BorshDeserialize for String {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        String::from_utf8(Vec::<u8>::deserialize(buf)?).map_err(|err| {
            let msg = err.to_string();
            Error::new(ErrorKind::InvalidData, msg)
        })
    }
}

impl<T> BorshDeserialize for Vec<T>
where
    T: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let len = u32::deserialize(buf)?;
        if len == 0 {
            Ok(Vec::new())
        } else if let Some(vec_bytes) = T::vec_from_bytes(len, buf)? {
            Ok(vec_bytes)
        } else if size_of::<T>() == 0 {
            let mut result = vec![T::deserialize(buf)?];

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
                result.push(T::deserialize(buf)?);
            }
            Ok(result)
        }
    }
}

impl<T> BorshDeserialize for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        Ok(Cow::Owned(BorshDeserialize::deserialize(buf)?))
    }
}

impl<T> BorshDeserialize for VecDeque<T>
where
    T: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let vec = <Vec<T>>::deserialize(buf)?;
        Ok(vec.into())
    }
}

impl<T> BorshDeserialize for LinkedList<T>
where
    T: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let vec = <Vec<T>>::deserialize(buf)?;
        Ok(vec.into_iter().collect::<LinkedList<T>>())
    }
}

impl<T> BorshDeserialize for BinaryHeap<T>
where
    T: BorshDeserialize + Ord,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let vec = <Vec<T>>::deserialize(buf)?;
        Ok(vec.into_iter().collect::<BinaryHeap<T>>())
    }
}

impl<T, H> BorshDeserialize for HashSet<T, H>
where
    T: BorshDeserialize + Eq + Hash,
    H: BuildHasher + Default,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let vec = <Vec<T>>::deserialize(buf)?;
        Ok(vec.into_iter().collect::<HashSet<T, H>>())
    }
}

impl<K, V, H> BorshDeserialize for HashMap<K, V, H>
where
    K: BorshDeserialize + Eq + Hash,
    V: BorshDeserialize,
    H: BuildHasher + Default,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let len = u32::deserialize(buf)?;
        // TODO(16): return capacity allocation when we can safely do that.
        let mut result = HashMap::with_hasher(H::default());
        for _ in 0..len {
            let key = K::deserialize(buf)?;
            let value = V::deserialize(buf)?;
            result.insert(key, value);
        }
        Ok(result)
    }
}

impl<T> BorshDeserialize for BTreeSet<T>
where
    T: BorshDeserialize + Ord,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let vec = <Vec<T>>::deserialize(buf)?;
        Ok(vec.into_iter().collect::<BTreeSet<T>>())
    }
}

impl<K, V> BorshDeserialize for BTreeMap<K, V>
where
    K: BorshDeserialize + Ord + core::hash::Hash,
    V: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let len = u32::deserialize(buf)?;
        let mut result = BTreeMap::new();
        for _ in 0..len {
            let key = K::deserialize(buf)?;
            let value = V::deserialize(buf)?;
            result.insert(key, value);
        }
        Ok(result)
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::SocketAddr {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let kind = u8::deserialize(buf)?;
        match kind {
            0 => std::net::SocketAddrV4::deserialize(buf).map(std::net::SocketAddr::V4),
            1 => std::net::SocketAddrV6::deserialize(buf).map(std::net::SocketAddr::V6),
            value => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Invalid SocketAddr variant: {}", value),
            )),
        }
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::SocketAddrV4 {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let ip = std::net::Ipv4Addr::deserialize(buf)?;
        let port = u16::deserialize(buf)?;
        Ok(std::net::SocketAddrV4::new(ip, port))
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::SocketAddrV6 {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let ip = std::net::Ipv6Addr::deserialize(buf)?;
        let port = u16::deserialize(buf)?;
        Ok(std::net::SocketAddrV6::new(ip, port, 0, 0))
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::Ipv4Addr {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if buf.len() < 4 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let bytes: [u8; 4] = buf[..4].try_into().unwrap();
        let res = std::net::Ipv4Addr::from(bytes);
        *buf = &buf[4..];
        Ok(res)
    }
}

#[cfg(feature = "std")]
impl BorshDeserialize for std::net::Ipv6Addr {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if buf.len() < 16 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let bytes: [u8; 16] = buf[..16].try_into().unwrap();
        let res = std::net::Ipv6Addr::from(bytes);
        *buf = &buf[16..];
        Ok(res)
    }
}

impl<T, U> BorshDeserialize for Box<T>
where
    U: Into<Box<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: BorshDeserialize,
{
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        Ok(T::Owned::deserialize(buf)?.into())
    }
}

impl<T, const N: usize> BorshDeserialize for [T; N]
where
    T: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if let Some(arr) = T::array_from_bytes(buf)? {
            Ok(arr)
        } else {
            struct ArrayDropGuard<T, const N: usize> {
                buffer: [MaybeUninit<T>; N],
                init_count: usize,
            }
            impl<T, const N: usize> Drop for ArrayDropGuard<T, N> {
                fn drop(&mut self) {
                    for element in self.buffer.iter_mut().take(self.init_count) {
                        //* SAFETY: The elements up to `i` have been initialized in
                        //*         `deserialize_array`.
                        unsafe { element.assume_init_drop() };
                    }
                }
            }
            impl<T, const N: usize> ArrayDropGuard<T, N> {
                unsafe fn transmute_to_array(mut self) -> [T; N] {
                    self.init_count = 0;
                    //* SAFETY: This cast is required because `mem::transmute` does not work with
                    //*         const generics https://github.com/rust-lang/rust/issues/61956. This
                    //*         array is guaranteed to be initialized by this point.
                    // TODO determine if ptr::read is actually safe and optimal.
                    (*(&MaybeUninit::new(core::ptr::read(&self.buffer)) as *const _
                        as *const MaybeUninit<_>))
                        .assume_init_read()
                }
            }
            let mut result = unsafe {
                ArrayDropGuard {
                    buffer: MaybeUninit::uninit().assume_init(),
                    init_count: 0,
                }
            };

            // TODO: replace with `core::array::try_from_fn` when stabilized to avoid manually
            // dropping below.
            for elem in result.buffer.iter_mut() {
                elem.write(T::deserialize(buf)?);
                result.init_count += 1;
            }

            //* SAFETY: The elements up to `i` have been initialized in
            //*         `deserialize_array`.
            Ok(unsafe { result.transmute_to_array() })
        }
    }
}

#[test]
fn array_deserialization_doesnt_leak() {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static DESERIALIZE_COUNT: AtomicUsize = AtomicUsize::new(0);
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct MyType(u8);
    impl BorshDeserialize for MyType {
        fn deserialize(buf: &mut &[u8]) -> Result<Self> {
            let val = u8::deserialize(buf)?;
            let v = DESERIALIZE_COUNT.fetch_add(1, Ordering::SeqCst);
            if v >= 5 {
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

    #[cfg(feature = "std")]
    {
        // Test that during a panic in deserialize, the values are still dropped.
        let result = std::panic::catch_unwind(|| {
            <[MyType; 3] as BorshDeserialize>::deserialize(&mut &[0u8; 3][..]).unwrap();
        });
        assert!(result.is_err());
        assert_eq!(DESERIALIZE_COUNT.load(Ordering::SeqCst), 6);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 5); // 5 because 6 panicked and was not init
    }
}

impl BorshDeserialize for () {
    fn deserialize(_buf: &mut &[u8]) -> Result<Self> {
        Ok(())
    }
}

macro_rules! impl_tuple {
    ($($name:ident)+) => {
      impl<$($name),+> BorshDeserialize for ($($name,)+)
      where $($name: BorshDeserialize,)+
      {
        #[inline]
        fn deserialize(buf: &mut &[u8]) -> Result<Self> {

            Ok(($($name::deserialize(buf)?,)+))
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
impl<T, U> BorshDeserialize for Rc<T>
where
    U: Into<Rc<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: BorshDeserialize,
{
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        Ok(T::Owned::deserialize(buf)?.into())
    }
}

#[cfg(feature = "rc")]
impl<T, U> BorshDeserialize for Arc<T>
where
    U: Into<Arc<T>> + Borrow<T>,
    T: ToOwned<Owned = U> + ?Sized,
    T::Owned: BorshDeserialize,
{
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        Ok(T::Owned::deserialize(buf)?.into())
    }
}

impl<T: ?Sized> BorshDeserialize for PhantomData<T> {
    fn deserialize(_: &mut &[u8]) -> Result<Self> {
        Ok(Self::default())
    }
}
