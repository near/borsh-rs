use super::{AsyncBorshSerialize, AsyncWriter};
use crate::maybestd::{io::Result, vec::Vec};

/// Serialize an object into a vector of bytes.
pub async fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: AsyncBorshSerialize + ?Sized + Sync,
{
    value.try_to_vec().await
}

/// Serializes an object directly into a `Writer`.
pub async fn to_writer<T, W: AsyncWriter>(mut writer: W, value: &T) -> Result<()>
where
    T: AsyncBorshSerialize + ?Sized + Sync,
{
    value.serialize(&mut writer).await
}
