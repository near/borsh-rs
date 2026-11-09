use super::{BorshSchemaContainer, Declaration, Definition, Fields};
use crate::__private::maybestd::{vec, vec::Vec};

pub use max_size::Error as SchemaMaxSerializedSizeError;
use max_size::{is_zero_size, ZeroSizeCache, ZeroSizeError};
pub use validate::Error as SchemaContainerValidateError;

mod max_size;
mod validate;

/// Returns the declarations `definition` refers to, last first, so that popping
/// them visits them in order.
///
/// The walks over a container keep the declarations they have yet to visit on
/// the heap rather than recursing, since a container decoded from untrusted
/// bytes can nest definitions deeply enough to overflow the stack.
fn members(definition: &Definition) -> Vec<&Declaration> {
    match definition {
        Definition::Primitive(_) => Vec::new(),
        Definition::Sequence { elements, .. } => vec![elements],
        Definition::Tuple { elements } => elements.iter().rev().collect(),
        Definition::Enum { variants, .. } => variants
            .iter()
            .rev()
            .map(|(_, _, variant)| variant)
            .collect(),
        Definition::Struct { fields } => match fields {
            Fields::NamedFields(fields) => fields.iter().rev().map(|(_, field)| field).collect(),
            Fields::UnnamedFields(fields) => fields.iter().rev().collect(),
            Fields::Empty => Vec::new(),
        },
    }
}
