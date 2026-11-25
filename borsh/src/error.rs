use crate::io::{Error, ErrorKind, Result};
use core::mem::size_of;
/// Error message returned when serializing or deserializing a length-prefixed collection of
/// zero-sized types, such as `Vec<()>`.
///
/// See [Collections of zero-sized types](crate#collections-of-zero-sized-types) for why
/// serialization is rejected as well.
pub const ERROR_ZST_FORBIDDEN: &str = "Collections of zero-sized types are not allowed due to deny-of-service concerns on deserialization.";

pub(crate) fn check_zst<T>() -> Result<()> {
    if size_of::<T>() == 0 {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN));
    }
    Ok(())
}

#[cfg(feature = "unstable__bounded")]
pub use crate::de::bounded::LimitExceeded;
