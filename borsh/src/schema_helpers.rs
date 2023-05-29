use crate::maybestd::{
    io::{Error, ErrorKind, Result},
    vec::Vec,
};
use crate::schema::BorshSchemaContainer;
use crate::{BorshDeserialize, BorshSchema, BorshSerialize};

/// Deserialize this instance from a slice of bytes, but assume that at the beginning we have
/// bytes describing the schema of the type. We deserialize this schema and verify that it is
/// correct.
pub fn try_from_slice_with_schema<T: BorshDeserialize + BorshSchema>(v: &[u8]) -> Result<T> {
    let mut v_mut = v;
    let (schema, object) = <(BorshSchemaContainer, T)>::deserialize(&mut v_mut)?;
    if !v_mut.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            crate::de::ERROR_NOT_ALL_BYTES_READ,
        ));
    }
    if T::schema_container() != schema {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Borsh schema does not match",
        ));
    }
    Ok(object)
}

pub fn from_slice<T: BorshDeserialize>(v: &[u8]) -> Result<T> {
    let mut v_mut = v;
    let object = T::deserialize(&mut v_mut)?;
    if !v_mut.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            crate::de::ERROR_NOT_ALL_BYTES_READ,
        ));
    }
    Ok(object)
}

/// Serialize object into a vector of bytes and prefix with the schema serialized as vector of
/// bytes in Borsh format.
pub fn try_to_vec_with_schema<T: BorshSerialize + BorshSchema>(value: &T) -> Result<Vec<u8>> {
    let schema = T::schema_container();
    let mut res = schema.try_to_vec()?;
    res.extend(value.try_to_vec()?);
    Ok(res)
}
