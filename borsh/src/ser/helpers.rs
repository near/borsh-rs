use crate::maybestd::{
    io::{Result, Write},
    vec::Vec,
};
use crate::BorshSerialize;

/// Serialize an object into a vector of bytes.
#[inline]
pub fn serialize_to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: BorshSerialize + ?Sized,
{
    value.try_to_vec()
}

/// Serializes an object directly into a `Writer`.
#[inline]
pub fn serialize_to_writer<T, W: Write>(value: &T, mut writer: W) -> Result<()>
where
    T: BorshSerialize + ?Sized,
{
    value.serialize(&mut writer)
}
