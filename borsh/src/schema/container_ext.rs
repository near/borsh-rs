use super::{BorshSchemaContainer, Declaration, Definition, Fields};

use max_size::is_zero_size;
pub use max_size::MaxSizeError;
pub use validate::ValidateError;

mod max_size;
mod validate;
