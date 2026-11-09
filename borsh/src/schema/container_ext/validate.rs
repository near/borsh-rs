use super::{is_zero_size, members, ZeroSizeCache, ZeroSizeError};
use super::{BorshSchemaContainer, Declaration, Definition};
use crate::__private::maybestd::{collections::BTreeSet, string::ToString, vec};

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
        // Every declaration reached so far, so that each one is checked once
        // however many times it is referenced.  The first error found is the
        // same as when skipping only the declarations on the current path:
        // everything reachable from a declaration visited before is either
        // checked already or on the current path.
        let mut visited = BTreeSet::new();
        let mut zero_size = ZeroSizeCache::new();
        // Declarations left to check, the next one last.
        let mut pending = vec![self.declaration()];
        while let Some(declaration) = pending.pop() {
            let definition = match self.get_definition(declaration) {
                Some(definition) => definition,
                None => {
                    return Err(Error::MissingDefinition(declaration.to_string()));
                }
            };
            if visited.insert(declaration) {
                check_definition(declaration, definition, self, &mut zero_size)?;
                pending.extend(members(definition));
            }
        }
        Ok(())
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
    /// only 0, 1, 2, 4 and 8 bytes long sequences' `length_width` are allowed
    TagNotPowerOfTwo(Declaration),
    /// Some of the declared types were lacking definition, which is considered
    /// a container's validation error
    MissingDefinition(Declaration),
    /// A Sequence defined with an empty length range.
    EmptyLengthRange(Declaration),
}

fn check_length_width(declaration: &Declaration, width: u8, max: u64) -> Result<(), Error> {
    match width {
        0 => Ok(()),
        3 | 5 | 6 | 7 => Err(Error::TagNotPowerOfTwo(declaration.clone())),
        1..=7 if max < 1 << (width * 8) => Ok(()),
        1..=7 => Err(Error::TagTooNarrow(declaration.clone())),
        8 => Ok(()),
        _ => Err(Error::TagTooWide(declaration.clone())),
    }
}

const U64_LEN: u8 = 8;

/// Checks the definition of `declaration`, but not its members.
fn check_definition<'a>(
    declaration: &'a Declaration,
    definition: &'a Definition,
    schema: &'a BorshSchemaContainer,
    zero_size: &mut ZeroSizeCache<'a>,
) -> core::result::Result<(), Error> {
    match definition {
        // arrays branch
        Definition::Sequence {
            length_width,
            length_range,
            ..
        } if *length_width == Definition::ARRAY_LENGTH_WIDTH
            && !length_range.is_empty()
            && length_range.start() == length_range.end() => {}
        Definition::Sequence {
            length_width,
            length_range,
            elements,
        } => {
            if length_range.is_empty() {
                return Err(Error::EmptyLengthRange(declaration.clone()));
            }
            check_length_width(declaration, *length_width, *length_range.end())?;
            match is_zero_size(elements, schema, zero_size) {
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
        }
        Definition::Enum { tag_width, .. } => {
            if *tag_width > U64_LEN {
                return Err(Error::TagTooWide(declaration.to_string()));
            }
        }
        Definition::Primitive(_) | Definition::Tuple { .. } | Definition::Struct { .. } => {}
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check_length_width, Error};
    use crate::__private::maybestd::string::ToString;

    #[test]
    fn test_check_tag_width() {
        let narrow_err: Result<(), Error> = Err(Error::TagTooNarrow("test".to_string()));
        let power_of_two_err: Result<(), Error> = Err(Error::TagNotPowerOfTwo("test".to_string()));

        for (width, max, want) in [
            (0, u64::MAX, Ok(())),
            (1, u8::MAX as u64, Ok(())),
            (1, u8::MAX as u64 + 1, narrow_err.clone()),
            (2, u16::MAX as u64, Ok(())),
            (2, u16::MAX as u64 + 1, narrow_err.clone()),
            (3, 100, power_of_two_err.clone()),
            (4, u32::MAX as u64, Ok(())),
            (4, u32::MAX as u64 + 1, narrow_err),
            (5, 100, power_of_two_err.clone()),
            (6, 100, power_of_two_err.clone()),
            (7, 100, power_of_two_err),
            (8, u64::MAX, Ok(())),
        ] {
            assert_eq!(
                want,
                check_length_width(&"test".into(), width, max),
                "width={width}; max={max}"
            );
        }
    }
}
