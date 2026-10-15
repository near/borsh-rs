use crate::io::{Error, ErrorKind, Result};
use core::mem::size_of;
pub const ERROR_ZST_FORBIDDEN: &str = "Collections of zero-sized types are not allowed due to deny-of-service concerns on deserialization.";

pub(crate) fn check_zst<T>() -> Result<()> {
    if size_of::<T>() == 0 {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN));
    }
    Ok(())
}

/// Inner error of the [`ErrorKind::OutOfMemory`] error returned when a decode
/// would exceed the memory limit given to
/// [`from_slice_with_limit`](crate::from_slice_with_limit) or
/// [`from_reader_with_limit`](crate::from_reader_with_limit).
#[cfg(feature = "de_limit")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitExceeded;

#[cfg(feature = "de_limit")]
impl core::fmt::Display for LimitExceeded {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Memory limit exceeded")
    }
}

#[cfg(feature = "de_limit")]
impl std::error::Error for LimitExceeded {}
