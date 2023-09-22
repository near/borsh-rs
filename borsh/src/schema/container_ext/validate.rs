use super::{is_zero_size, ZeroSizeError};
use super::{BorshSchemaContainer, Declaration, Definition, Fields};
use crate::__private::maybestd::{string::ToString, vec::Vec};

impl BorshSchemaContainer {
    /// Validates container for violation of any well-known rules with
    /// respect to `borsh` serialization.
    ///
    /// # Example
    ///
    /// ```
    /// use borsh::schema::BorshSchemaContainer;
    ///
    /// let schema = BorshSchemaContainer::for_type::<usize>();
    /// assert_eq!(Ok(()), schema.validate());
    /// ```
    pub fn validate(&self) -> core::result::Result<(), Error> {
        let mut stack = Vec::new();
        validate_impl(self.declaration(), self, &mut stack)
    }
}

/// Possible error when validating a [`BorshSchemaContainer`], generated for some type `T`,
/// for violation of any well-known rules with respect to `borsh` serialization.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// sequences of zero-sized types of dynamic length are forbidden by definition
    /// see <https://github.com/near/borsh-rs/pull/202> and related ones
    ZSTSequence(Declaration),
    /// Declared tag width is too large.  Tags may be at most eight bytes.
    TagTooWide(Declaration),
    /// Declared tag width is too small.  Tags must be large enough to represent
    /// possible length of sequence.
    TagTooNarrow(Declaration),
    /// Some of the declared types were lacking definition, which is considered
    /// a container's validation error
    MissingDefinition(Declaration),
    /// A Sequence defined with an empty length range.
    EmptyLengthRange(Declaration),
}

fn check_tag_width(declaration: &Declaration, width: u8, max: u64) -> Result<(), Error> {
    match width {
        0 => Ok(()),
        1..=7 if max < 1 << (width * 8) => Ok(()),
        1..=7 => Err(Error::TagTooNarrow(declaration.clone())),
        8 => Ok(()),
        _ => Err(Error::TagTooWide(declaration.clone())),
    }
}

fn validate_impl<'a>(
    declaration: &'a Declaration,
    schema: &'a BorshSchemaContainer,
    stack: &mut Vec<&'a Declaration>,
) -> core::result::Result<(), Error> {
    let definition = match schema.get_definition(declaration) {
        Some(definition) => definition,
        None => {
            return Err(Error::MissingDefinition(declaration.to_string()));
        }
    };
    if stack.iter().any(|dec| *dec == declaration) {
        return Ok(());
    }
    stack.push(declaration);
    match definition {
        Definition::Primitive(_size) => {}
        Definition::Sequence {
            length_width,
            length_range,
            elements,
        } => {
            if length_range.is_empty() {
                return Err(Error::EmptyLengthRange(declaration.clone()));
            }
            check_tag_width(declaration, *length_width, *length_range.end())?;
            match is_zero_size(elements, schema) {
                Ok(true) => return Err(Error::ZSTSequence(declaration.clone())),
                Ok(false) => (),
                // a recursive type either has no exit, so it cannot be instantiated
                // or it uses `Definiotion::Enum` or `Definition::Sequence` to exit from recursion
                // which make it non-zero size
                Err(ZeroSizeError::Recursive) => (),
                Err(ZeroSizeError::MissingDefinition(declaration)) => {
                    return Err(Error::MissingDefinition(declaration));
                }
            }
            validate_impl(elements, schema, stack)?;
        }
        Definition::Enum {
            tag_width,
            variants,
        } => {
            // TODO(#230): variants.len() is actually incorrect here.  Since
            // arbitrary discriminant can be used for variants, an actual
            // largest discriminant may be larger than the number of variants.
            // For the time being we’re using variants.len() as a proxy but
            // eventually variant discriminants should be added to the schema.
            let max_discriminant = variants.len().saturating_sub(1) as u64;
            check_tag_width(declaration, *tag_width, max_discriminant)?;
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

#[test]
fn test_check_tag_width() {
    for (width, max, want) in [
        (0, u64::MAX, Ok(())),
        (1, 255, Ok(())),
        (1, 256, Err(Error::TagTooNarrow("test".into()))),
        (2, (1 << 16) - 1, Ok(())),
        (2, 1 << 16, Err(Error::TagTooNarrow("test".into()))),
        (3, (1 << 24) - 1, Ok(())),
        (3, 1 << 24, Err(Error::TagTooNarrow("test".into()))),
        (4, (1 << 32) - 1, Ok(())),
        (4, 1 << 32, Err(Error::TagTooNarrow("test".into()))),
        (5, (1 << 40) - 1, Ok(())),
        (5, 1 << 40, Err(Error::TagTooNarrow("test".into()))),
        (6, (1 << 48) - 1, Ok(())),
        (6, 1 << 48, Err(Error::TagTooNarrow("test".into()))),
        (7, (1 << 56) - 1, Ok(())),
        (7, 1 << 56, Err(Error::TagTooNarrow("test".into()))),
        (8, u64::MAX, Ok(())),
        (9, 0, Err(Error::TagTooWide("test".into()))),
    ] {
        assert_eq!(
            want,
            check_tag_width(&"test".into(), width, max),
            "width={width}; max={max}"
        );
    }
}
