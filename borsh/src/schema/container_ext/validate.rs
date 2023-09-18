use super::is_zero_size;
use super::{BorshSchemaContainer, Declaration, Definition, Fields};
use crate::__private::maybestd::{string::ToString, vec::Vec};

impl BorshSchemaContainer {
    /// Validates container for violation of any well-known rules with
    /// respect to `borsh` serialization.
    ///
    /// Zero-sized types should follow the convention of either providing a [Definition] or
    /// specifying `"nil"` as their [Declaration] for this method to work correctly.
    ///
    /// # Example
    ///
    /// ```
    /// use borsh::schema::BorshSchemaContainer;
    ///
    /// let schema = BorshSchemaContainer::for_type::<usize>();
    /// assert_eq!(Ok(()), schema.validate());
    /// ```
    pub fn validate(&self) -> core::result::Result<(), SchemaContainerValidateError> {
        let mut stack = Vec::new();
        validate_impl(self.declaration(), self, &mut stack)
    }
}

/// Possible error when validating a [`BorshSchemaContainer`], generated for some type `T`,
/// for violation of any well-known rules with respect to `borsh` serialization.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SchemaContainerValidateError {
    /// sequences of zero-sized types of dynamic length are forbidden by definition
    /// see <https://github.com/near/borsh-rs/pull/202> and related ones
    ZSTSequence(Declaration),
    /// Declared tag width is too large.  Tags may be at most eight bytes.
    TagTooWide(Declaration),
}

fn validate_impl<'a>(
    declaration: &'a Declaration,
    schema: &'a BorshSchemaContainer,
    stack: &mut Vec<&'a Declaration>,
) -> core::result::Result<(), SchemaContainerValidateError> {
    let definition = match schema.get_definition(declaration) {
        Some(definition) => definition,
        // it's not an error for a type to not contain any definition
        // it's either a `borsh`'s lib type like `"string"` or type declared by user
        None => {
            return Ok(());
        }
    };
    if stack.iter().any(|dec| *dec == declaration) {
        return Ok(());
    }
    stack.push(declaration);
    match definition {
        Definition::Array { elements, .. } => validate_impl(elements, schema, stack)?,
        Definition::Sequence { elements } => {
            // a recursive type either has no exit, so it cannot be instantiated
            // or it uses `Definiotion::Enum` or `Definition::Sequence` to exit from recursion
            // which make it non-zero size
            if is_zero_size(elements, schema).unwrap_or(false) {
                return Err(SchemaContainerValidateError::ZSTSequence(
                    declaration.to_string(),
                ));
            }
            validate_impl(elements, schema, stack)?;
        }
        Definition::Enum {
            tag_width,
            variants,
        } => {
            if *tag_width > 8 {
                return Err(SchemaContainerValidateError::TagTooWide(
                    declaration.to_string(),
                ));
            }
            for (_, variant) in variants {
                validate_impl(variant, schema, stack)?;
            }
        }
        Definition::Tuple { elements } => {
            for element_type in elements {
                validate_impl(element_type, schema, stack)?;
            }
        }
        Definition::Struct { fields } => match fields {
            Fields::NamedFields(fields) => {
                for (_field_name, field_type) in fields {
                    validate_impl(field_type, schema, stack)?;
                }
            }
            Fields::UnnamedFields(fields) => {
                for field_type in fields {
                    validate_impl(field_type, schema, stack)?;
                }
            }
            Fields::Empty => {}
        },
    };
    stack.pop();
    Ok(())
}
