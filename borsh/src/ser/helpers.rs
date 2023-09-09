use crate::BorshSerialize;
use crate::__private::maybestd::vec::Vec;
use crate::io::{Result, Write};

pub(super) const DEFAULT_SERIALIZER_CAPACITY: usize = 1024;

/// Serialize an object into a vector of bytes.
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: BorshSerialize + ?Sized,
{
    let mut result = Vec::with_capacity(DEFAULT_SERIALIZER_CAPACITY);
    value.serialize(&mut result)?;
    Ok(result)
}

/// Serializes an object directly into a `Writer`.
pub fn to_writer<T, W: Write>(mut writer: W, value: &T) -> Result<()>
where
    T: BorshSerialize + ?Sized,
{
    value.serialize(&mut writer)
}
