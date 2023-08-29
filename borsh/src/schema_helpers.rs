use crate::__private::maybestd::{
    collections::BTreeMap,
    io::{Error, ErrorKind, Result},
    vec::Vec,
};
use crate::from_slice;
use crate::schema::BorshSchemaContainer;
use crate::{BorshDeserialize, BorshSchema, BorshSerialize};

/// Deserialize this instance from a slice of bytes, but assume that at the beginning we have
/// bytes describing the schema of the type. We deserialize this schema and verify that it is
/// correct.
pub fn try_from_slice_with_schema<T: BorshDeserialize + BorshSchema>(v: &[u8]) -> Result<T> {
    let (schema, object) = from_slice::<(BorshSchemaContainer, T)>(v)?;
    if schema_container_of::<T>() != schema {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Borsh schema does not match",
        ));
    }
    Ok(object)
}

/// Serialize object into a vector of bytes and prefix with the schema serialized as vector of
/// bytes in Borsh format.
pub fn try_to_vec_with_schema<T: BorshSerialize + BorshSchema>(value: &T) -> Result<Vec<u8>> {
    let schema = schema_container_of::<T>();
    let mut res = crate::to_vec(&schema)?;
    value.serialize(&mut res)?;
    Ok(res)
}

pub fn schema_container_of<T: BorshSchema>() -> BorshSchemaContainer {
    let mut definitions = BTreeMap::new();
    T::add_definitions_recursively(&mut definitions);

    BorshSchemaContainer::new(T::declaration(), definitions)
}
