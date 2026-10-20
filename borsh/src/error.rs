use crate::io::{Error, ErrorKind, Result};
use crate::{BorshDeserialize, BorshSerialize};
use core::mem::size_of;
pub const ERROR_ZST_FORBIDDEN: &str = "Collections of zero-sized types are not allowed due to deny-of-service concerns on deserialization.";

// NOTE: the guards check both the in-memory size and the encoded size. `size_of` alone misses
// types such as `Box<()>`, which are not zero-sized in memory but read and write zero bytes, so a
// 4-byte length prefix could still drive up to `u32::MAX` iterations and allocations.
// `ENCODES_TO_ZERO_BYTES` is `false` unless a type is known to encode to zero bytes (manual impls
// don't override it), and the `size_of` check keeps rejecting everything rejected before.
// Both conditions are constants, so the check costs nothing at runtime.

pub(crate) fn check_zst_de<T: BorshDeserialize>() -> Result<()> {
    if size_of::<T>() == 0 || T::ENCODES_TO_ZERO_BYTES {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN));
    }
    Ok(())
}

pub(crate) fn check_zst_ser<T: BorshSerialize>() -> Result<()> {
    if size_of::<T>() == 0 || T::ENCODES_TO_ZERO_BYTES {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN));
    }
    Ok(())
}
