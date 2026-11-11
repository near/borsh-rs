use core::hash::{BuildHasher, Hash};

use crate::__private::maybestd::collections::{HashMap, HashSet};
use crate::error::check_zst;
use crate::io::{Read, Result};

#[cfg(feature = "de_strict_order")]
use super::check_strict_order;
use super::{decode_seq, size, BorshDeserializeBounded, Budget};

impl<T, H> BorshDeserializeBounded for HashSet<T, H>
where
    T: BorshDeserializeBounded + Eq + Hash + Ord,
    H: BuildHasher + Default,
{
    #[inline]
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let vec = decode_seq::<T, _>(reader, budget, size::table::<T>)?;

        #[cfg(feature = "de_strict_order")]
        check_strict_order(&vec, |a, b| a.cmp(b))?;

        Ok(vec.into_iter().collect::<HashSet<T, H>>())
    }
}

impl<K, V, H> BorshDeserializeBounded for HashMap<K, V, H>
where
    K: BorshDeserializeBounded + Eq + Hash + Ord,
    V: BorshDeserializeBounded,
    H: BuildHasher + Default,
{
    #[inline]
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        check_zst::<K>()?;
        let vec = decode_seq::<(K, V), _>(reader, budget, size::table::<(K, V)>)?;

        #[cfg(feature = "de_strict_order")]
        check_strict_order(&vec, |(a, _), (b, _)| a.cmp(b))?;

        Ok(vec.into_iter().collect::<HashMap<K, V, H>>())
    }
}
