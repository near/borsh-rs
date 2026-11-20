//! Impls for the types of borsh's optional dependencies, each behind its own feature.

#[cfg(feature = "ascii")]
mod ascii {
    use crate::__private::maybestd::{string::ToString, vec::Vec};
    use crate::io::{Error, ErrorKind, Read, Result};

    use super::super::{BorshDeserializeBounded, Budget};

    impl BorshDeserializeBounded for ascii::AsciiString {
        #[inline]
        fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
            let bytes = Vec::<u8>::deserialize_bounded_alloc(reader, budget)?;
            ascii::AsciiString::from_ascii(bytes)
                .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))
        }
    }

    impl_bounded_via_plain!(ascii::AsciiChar);
}

#[cfg(feature = "bytes")]
mod bytes {
    use crate::__private::maybestd::vec::Vec;
    use crate::io::{Read, Result};

    use super::super::{charge_seq, push_capped, size, BorshDeserializeBounded, Budget};

    impl BorshDeserializeBounded for bytes::Bytes {
        #[inline]
        fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
            let vec = <Vec<u8>>::deserialize_bounded_alloc(reader, budget)?;
            Ok(vec.into())
        }
    }

    impl BorshDeserializeBounded for bytes::BytesMut {
        #[inline]
        fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
            let len = u32::deserialize_bounded_alloc(reader, budget)?;
            if len == 0 {
                return Ok(bytes::BytesMut::new());
            }
            let n = charge_seq::<u8>(budget, len, size::vec::<u8>)?;
            // Byte by byte like the plain impl, not through the `u8` vector hook, so that
            // readers see the same calls.
            let mut out = Vec::with_capacity(crate::de::hint::cautious::<u8>(len));
            for _ in 0..len {
                push_capped(&mut out, u8::deserialize_bounded_alloc(reader, budget)?, n);
            }
            // Reuses the buffer.
            Ok(out.into_iter().collect())
        }
    }
}

#[cfg(feature = "indexmap")]
mod indexmap {
    use core::hash::{BuildHasher, Hash};

    use crate::error::check_zst;
    use crate::io::{Read, Result};

    use super::super::{decode_seq, size, BorshDeserializeBounded, Budget};

    impl<K, V, S> BorshDeserializeBounded for indexmap::IndexMap<K, V, S>
    where
        K: BorshDeserializeBounded + Eq + Hash,
        V: BorshDeserializeBounded,
        S: BuildHasher + Default,
    {
        #[inline]
        fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
            check_zst::<K>()?;
            let vec = decode_seq::<(K, V), _>(reader, budget, size::index_map::<K, V>)?;
            Ok(vec.into_iter().collect::<indexmap::IndexMap<K, V, S>>())
        }
    }

    impl<T, S> BorshDeserializeBounded for indexmap::IndexSet<T, S>
    where
        T: BorshDeserializeBounded + Eq + Hash,
        S: BuildHasher + Default,
    {
        #[inline]
        fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
            check_zst::<T>()?;
            let vec = decode_seq::<T, _>(reader, budget, size::index_map::<T, ()>)?;
            Ok(vec.into_iter().collect::<indexmap::IndexSet<T, S>>())
        }
    }
}

#[cfg(feature = "uuid")]
impl_bounded_via_plain!(uuid::Uuid);

#[cfg(feature = "bson")]
impl_bounded_via_plain!(bson::oid::ObjectId);
