use crate::io::{Error, ErrorKind, Result};
use core::fmt;
use core::mem::size_of;
pub const ERROR_ZST_FORBIDDEN: &str = "Collections of zero-sized types are not allowed due to deny-of-service concerns on deserialization.";

pub(crate) fn check_zst<T>() -> Result<()> {
    if size_of::<T>() == 0 {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN));
    }
    Ok(())
}

/// The error a bounded decode returns when a charge does not fit the remaining memory limit.
///
/// It reaches the caller wrapped in an [`io::Error`](crate::io::Error) of kind
/// [`OutOfMemory`](crate::io::ErrorKind::OutOfMemory).
///
/// With the `std` feature, it is the error's inner error:
///
/// ```
/// use borsh::error::LimitExceeded;
///
/// // Claims 1000 `u64`s, which need 8000 bytes.
/// let input = 1000u32.to_le_bytes();
/// let err = borsh::from_slice_bounded::<Vec<u64>>(&input, 100).unwrap_err();
/// # #[cfg(feature = "std")]
/// # {
/// let exceeded = err
///     .get_ref()
///     .and_then(|inner| inner.downcast_ref::<LimitExceeded>())
///     .unwrap();
/// assert_eq!((exceeded.limit, exceeded.requested, exceeded.remaining), (100, 8000, 100));
/// # }
/// ```
///
/// Without `std`, `io::Error` only keeps a message, so check
/// `err.kind() == ErrorKind::OutOfMemory`. A reader or a hand-written implementation can
/// return that kind too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct LimitExceeded {
    /// The limit the decode started with.
    pub limit: usize,
    /// The bytes the failed charge asked for. `usize::MAX` when the amount overflowed or is
    /// not a valid allocation size.
    pub requested: usize,
    /// The bytes that were still available when the charge failed.
    pub remaining: usize,
}

impl LimitExceeded {
    pub(crate) fn into_io_error(self) -> Error {
        #[cfg(feature = "std")]
        {
            Error::new(ErrorKind::OutOfMemory, self)
        }
        #[cfg(not(feature = "std"))]
        {
            use crate::__private::maybestd::string::ToString;
            Error::new(ErrorKind::OutOfMemory, self.to_string())
        }
    }
}

impl fmt::Display for LimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Memory limit exceeded: {} bytes requested, {} of {} bytes remaining",
            self.requested, self.remaining, self.limit
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LimitExceeded {}
