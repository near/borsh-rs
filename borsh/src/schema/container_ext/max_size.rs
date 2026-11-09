use super::{BorshSchemaContainer, Declaration, Definition, Fields};
use crate::__private::maybestd::{collections::BTreeMap, string::ToString, vec::Vec};

impl BorshSchemaContainer {
    /// Returns the largest possible size of a serialised object based solely on its type.
    ///
    /// Even when if returned upper bound is correct, the theoretical value may be
    /// *much* larger than any practical length.  For example, maximum encoded
    /// length of `String` is 4 GiB while in practice one may encounter strings of
    /// at most dozen of characters.
    ///
    /// # Example
    ///
    /// ```
    /// use borsh::schema::BorshSchemaContainer;
    ///
    /// let schema = BorshSchemaContainer::for_type::<()>();
    /// assert_eq!(Ok(0), schema.max_serialized_size());
    ///
    /// let schema = BorshSchemaContainer::for_type::<usize>();
    /// assert_eq!(Ok(8), schema.max_serialized_size());
    ///
    /// // 4 bytes of length and u32::MAX for the longest possible string.
    /// let schema = BorshSchemaContainer::for_type::<String>();
    /// assert_eq!(Ok(4 + 4294967295), schema.max_serialized_size());
    ///
    /// let schema = BorshSchemaContainer::for_type::<Vec<String>>();
    /// assert_eq!(Err(borsh::schema::SchemaMaxSerializedSizeError::Overflow),
    ///            schema.max_serialized_size());
    /// ```
    pub fn max_serialized_size(&self) -> Result<usize, Error> {
        let mut stack = Vec::new();
        // Size of one value of each declaration computed so far, so that one
        // referenced many times is walked once.  A size doesn't depend on the
        // path it was computed on, and any error ends the walk, so only sizes
        // are kept.
        let mut sizes = BTreeMap::new();
        let mut zero_size = ZeroSizeCache::new();
        max_serialized_size_impl(
            self.declaration(),
            self,
            &mut stack,
            &mut sizes,
            &mut zero_size,
        )
    }
}

/// Possible error when calculating theoretical maximum size of encoded type `T`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The theoretical maximum size of the encoded value overflows `usize`.
    ///
    /// This may happen for nested dynamically-sized types such as
    /// `Vec<Vec<u8>>` whose maximum size is `4 + u32::MAX * (4 + u32::MAX)`.
    Overflow,

    /// The type is recursive and thus theoretical maximum size is infinite.
    ///
    /// Simple type in which this triggers is `struct Rec(Option<Box<Rec>>)`.
    Recursive,

    /// Some of the declared types were lacking definition making it impossible
    /// to calculate the size.
    MissingDefinition(Declaration),
}

/// Implementation of [`BorshSchema::max_serialized_size`]: the size of one
/// value of `declaration`.
fn max_serialized_size_impl<'a>(
    declaration: &'a str,
    schema: &'a BorshSchemaContainer,
    stack: &mut Vec<&'a str>,
    sizes: &mut BTreeMap<&'a str, usize>,
    zero_size: &mut ZeroSizeCache<'a>,
) -> Result<usize, Error> {
    use core::convert::TryFrom;

    fn add(x: usize, y: usize) -> Result<usize, Error> {
        x.checked_add(y).ok_or(Error::Overflow)
    }

    fn mul(x: usize, y: usize) -> Result<usize, Error> {
        x.checked_mul(y).ok_or(Error::Overflow)
    }

    /// Calculates max serialised size of a tuple with given members.
    fn tuple<'a>(
        elements: impl core::iter::IntoIterator<Item = &'a Declaration>,
        schema: &'a BorshSchemaContainer,
        stack: &mut Vec<&'a str>,
        sizes: &mut BTreeMap<&'a str, usize>,
        zero_size: &mut ZeroSizeCache<'a>,
    ) -> Result<usize, Error> {
        let mut sum: usize = 0;
        for el in elements {
            let sz = max_serialized_size_impl(el, schema, stack, sizes, zero_size)?;
            sum = add(sum, sz)?;
        }
        Ok(sum)
    }

    if let Some(size) = sizes.get(declaration) {
        return Ok(*size);
    }
    if stack.contains(&declaration) {
        return Err(Error::Recursive);
    }
    stack.push(declaration);

    let res = match schema.get_definition(declaration).ok_or(declaration) {
        Ok(Definition::Primitive(size)) => Ok(usize::from(*size)),
        Ok(Definition::Sequence {
            length_width,
            length_range,
            elements,
        }) => {
            // Assume sequence has the maximum number of elements.
            let sz = match usize::try_from(*length_range.end()) {
                Ok(0) => 0,
                Ok(max_len) => mul(
                    max_len,
                    max_serialized_size_impl(elements, schema, stack, sizes, zero_size)?,
                )?,
                Err(_) if is_zero_size_impl(elements, schema, stack, zero_size)? => 0,
                Err(_) => return Err(Error::Overflow),
            };
            add(sz, usize::from(*length_width))
        }

        Ok(Definition::Enum {
            tag_width,
            variants,
        }) => {
            let mut max = 0;
            for (_, _, variant) in variants {
                let sz = max_serialized_size_impl(variant, schema, stack, sizes, zero_size)?;
                max = max.max(sz);
            }
            add(max, usize::from(*tag_width))
        }

        // Tuples and structs sum sizes of all the members.
        Ok(Definition::Tuple { elements }) => tuple(elements, schema, stack, sizes, zero_size),
        Ok(Definition::Struct { fields }) => match fields {
            Fields::NamedFields(fields) => tuple(
                fields.iter().map(|(_, field)| field),
                schema,
                stack,
                sizes,
                zero_size,
            ),
            Fields::UnnamedFields(fields) => tuple(fields, schema, stack, sizes, zero_size),
            Fields::Empty => Ok(0),
        },

        Err(declaration) => Err(Error::MissingDefinition(declaration.to_string())),
    }?;

    stack.pop();
    sizes.insert(declaration, res);
    Ok(res)
}

/// Checks whether given declaration schema serialises to an empty string.
///
/// This is used by [`BorshSchemaContainer::max_serialized_size`] to handle weird types
/// such as `[[[(); u32::MAX]; u32::MAX]; u32::MAX]` which serialises to an
/// empty string even though its number of elements overflows `usize`.
///
/// Error value means that the method has been called recursively.
/// A recursive type either has no exit, so it cannot be instantiated
/// or it uses `Definiotion::Enum` or `Definition::Sequence` to exit from recursion
/// which make it non-zero size
pub(super) fn is_zero_size<'a>(
    declaration: &'a Declaration,
    schema: &'a BorshSchemaContainer,
    cache: &mut ZeroSizeCache<'a>,
) -> Result<bool, ZeroSizeError> {
    is_zero_size_impl(declaration, schema, &[], cache)
}

/// Answers of the zero-size checks made by one [`BorshSchemaContainer::validate`]
/// or [`BorshSchemaContainer::max_serialized_size`] call, so that a declaration
/// reachable from many sequences is checked once.
///
/// `None` marks a declaration whose check is under way or ran into recursion,
/// and is reported as recursive when reached again: each declaration on the
/// path when recursion is found leads into it again, whichever way it is
/// reached.  Any other error ends the caller's walk.
pub(super) type ZeroSizeCache<'a> = BTreeMap<&'a str, Option<bool>>;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ZeroSizeError {
    Recursive,
    MissingDefinition(Declaration),
}

impl From<ZeroSizeError> for Error {
    fn from(value: ZeroSizeError) -> Self {
        match value {
            ZeroSizeError::Recursive => Self::Recursive,
            ZeroSizeError::MissingDefinition(declaration) => Self::MissingDefinition(declaration),
        }
    }
}

/// `outer` holds declarations which count as recursion too: the path of the
/// enclosing [`BorshSchemaContainer::max_serialized_size`] walk.
fn is_zero_size_impl<'a>(
    declaration: &'a str,
    schema: &'a BorshSchemaContainer,
    outer: &[&'a str],
    cache: &mut ZeroSizeCache<'a>,
) -> Result<bool, ZeroSizeError> {
    fn all<'a, T: 'a>(
        iter: impl Iterator<Item = T>,
        f_key: impl Fn(&T) -> &'a Declaration,
        schema: &'a BorshSchemaContainer,
        outer: &[&'a str],
        cache: &mut ZeroSizeCache<'a>,
    ) -> Result<bool, ZeroSizeError> {
        for element in iter {
            let declaration = f_key(&element);
            if !is_zero_size_impl(declaration.as_str(), schema, outer, cache)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    if outer.contains(&declaration) {
        return Err(ZeroSizeError::Recursive);
    }
    if let Some(known) = cache.get(declaration) {
        return known.ok_or(ZeroSizeError::Recursive);
    }
    cache.insert(declaration, None);

    let res = match schema.get_definition(declaration).ok_or(declaration) {
        Ok(Definition::Primitive(size)) => *size == 0,
        Ok(Definition::Sequence {
            length_width,
            length_range,
            elements,
        }) => {
            // NOTE: no early `return` here, so that the answer is recorded below;
            // otherwise a second occurrence of the same array type would be
            // reported as recursive
            *length_width == 0
                && (*length_range == (0..=0)
                    || is_zero_size_impl(elements.as_str(), schema, outer, cache)?)
        }
        Ok(Definition::Tuple { elements }) => {
            all(elements.iter(), |key| *key, schema, outer, cache)?
        }
        Ok(Definition::Enum {
            tag_width: 0,
            variants,
        }) => all(
            variants.iter(),
            |(_variant_discrim, _variant_name, declaration)| declaration,
            schema,
            outer,
            cache,
        )?,
        Ok(Definition::Enum { .. }) => false,
        Ok(Definition::Struct { fields }) => match fields {
            Fields::NamedFields(fields) => all(
                fields.iter(),
                |(_field_name, declaration)| declaration,
                schema,
                outer,
                cache,
            )?,
            Fields::UnnamedFields(fields) => all(
                fields.iter(),
                |declaration| declaration,
                schema,
                outer,
                cache,
            )?,
            Fields::Empty => true,
        },

        Err(declaration) => {
            return Err(ZeroSizeError::MissingDefinition(declaration.into()));
        }
    };
    cache.insert(declaration, Some(res));
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    // this is not integration test module, so can use __private for ease of imports;
    // it cannot be made integration, as it tests `is_zero_size` function, chosen to be non-pub
    use crate::__private::maybestd::{boxed::Box, string::ToString};

    #[test]
    fn test_is_zero_size_recursive_check_bypassed() {
        use crate as borsh;

        #[derive(::borsh_derive::BorshSchema)]
        struct RecursiveExitSequence(Vec<RecursiveExitSequence>);

        let schema = BorshSchemaContainer::for_type::<RecursiveExitSequence>();
        assert_eq!(
            Ok(false),
            is_zero_size(schema.declaration(), &schema, &mut ZeroSizeCache::new())
        );
    }

    #[test]
    fn test_is_zero_size_recursive_check_err() {
        use crate as borsh;

        #[derive(::borsh_derive::BorshSchema)]
        struct RecursiveNoExitStructUnnamed(Box<RecursiveNoExitStructUnnamed>);

        let schema = BorshSchemaContainer::for_type::<RecursiveNoExitStructUnnamed>();
        assert_eq!(
            Err(ZeroSizeError::Recursive),
            is_zero_size(schema.declaration(), &schema, &mut ZeroSizeCache::new())
        );
    }
}
