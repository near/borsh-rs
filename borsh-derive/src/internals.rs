pub mod attributes;
pub mod deserialize;
mod enum_discriminant_map;
mod generics;
#[cfg(feature = "schema")]
pub mod schema;
pub mod serialize;

#[cfg(feature = "schema")]
pub use schema::{process_enum, process_struct};

pub use deserialize::{enum_de, struct_de, union_de};
pub use serialize::{enum_ser, struct_ser, union_ser};
#[cfg(test)]
mod test_helpers;
