use super::{BorshSchemaContainer, Declaration, Definition, Fields};

use max_size::is_zero_size;
pub use max_size::SchemaMaxSerializedSizeError;
pub use validate::SchemaContainerValidateError;

mod max_size;
mod validate;
