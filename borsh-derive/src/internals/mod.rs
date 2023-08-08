pub mod attributes;
pub mod deserialize;
mod enum_discriminant;
mod generics;
#[cfg(feature = "schema")]
pub mod schema;
pub mod serialize;

#[cfg(test)]
mod test_helpers;
