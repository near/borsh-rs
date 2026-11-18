use crate::io::{Error, ErrorKind, Result};
use core::mem::size_of;

pub const ERROR_ZST_FORBIDDEN: &str = "Collections of zero-sized types are not allowed due to \
                                        deny-of-service concerns on deserialization.";

/// Collections reject element types that are zero-sized or always encode to zero bytes.
pub(crate) fn check_zst<T>(encodes_to_zero_bytes: bool) -> Result<()> {
    if size_of::<T>() == 0 || encodes_to_zero_bytes {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN));
    }
    Ok(())
}
