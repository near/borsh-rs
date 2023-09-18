use super::{BorshSchemaContainer, Declaration, Definition, Fields};
use crate::__private::maybestd::{string::ToString, vec::Vec};

use core::num::NonZeroUsize;

/// NonZeroUsize of value one.
// TODO: Replace usage by NonZeroUsize::MIN once MSRV is 1.70+.
const ONE: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };

impl BorshSchemaContainer {
    /// Returns the largest possible size of a serialised object based solely on its type.
    ///
    /// The function has limitations which may lead it to overestimate the size.
    /// For example, hypothetical `IPv4Packet` would be encoded as at most ~64 KiB.
    /// However, if it uses sequence schema, this function will claim that the
    /// maximum size is ~4 GiB.
    ///
    /// Even when if returned upper bound is correct, the theoretical value may be
    /// *much* larger than any practical length.  For example, maximum encoded
    /// length of `String` is 4 GiB while in practice one may encounter strings of
    /// at most dozen of characters.  Depending on usage, caller should apply upper
    /// bound on the result.
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
        max_serialized_size_impl(ONE, self.declaration(), self, &mut stack)
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

/// Implementation of [`BorshSchema::max_serialized_size`].
fn max_serialized_size_impl<'a>(
    count: NonZeroUsize,
    declaration: &'a str,
    schema: &'a BorshSchemaContainer,
    stack: &mut Vec<&'a str>,
) -> Result<usize, Error> {
    use core::convert::TryFrom;

    /// Maximum number of elements in a vector or length of a string which can
    /// be serialised.
    const MAX_LEN: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(u32::MAX as usize) };

    fn add(x: usize, y: usize) -> Result<usize, Error> {
        x.checked_add(y).ok_or(Error::Overflow)
    }

    fn mul(x: NonZeroUsize, y: usize) -> Result<usize, Error> {
        x.get().checked_mul(y).ok_or(Error::Overflow)
    }

    /// Calculates max serialised size of a tuple with given members.
    fn tuple<'a>(
        count: NonZeroUsize,
        elements: impl core::iter::IntoIterator<Item = &'a Declaration>,
        schema: &'a BorshSchemaContainer,
        stack: &mut Vec<&'a str>,
    ) -> Result<usize, Error> {
        let mut sum: usize = 0;
        for el in elements {
            sum = add(sum, max_serialized_size_impl(ONE, el, schema, stack)?)?;
        }
        mul(count, sum)
    }

    if stack.iter().any(|dec| *dec == declaration) {
        return Err(Error::Recursive);
    }
    stack.push(declaration);

    let res = match schema.get_definition(declaration).ok_or(declaration) {
        Ok(Definition::Primitive(size)) => match size {
            0 => Ok(0),
            size => {
                let count_sizes = usize::try_from(*size)
                    .ok()
                    .and_then(|size| size.checked_mul(count.get()));
                count_sizes.ok_or(Error::Overflow)
            }
        },
        Ok(Definition::Array { length, elements }) => {
            // Aggregate `count` and `length` to a single number.  If this
            // overflows, check if array’s element is zero-sized.
            let count_lengths = usize::try_from(*length)
                .ok()
                .and_then(|len| len.checked_mul(count.get()));
            let count_lengths = match count_lengths {
                Some(count_lengths) => count_lengths,
                None if is_zero_size_impl(elements.as_str(), schema, stack)? => {
                    return Ok(0);
                }
                None => {
                    return Err(Error::Overflow);
                }
            };
            let count_lengths = NonZeroUsize::new(count_lengths);

            match count_lengths {
                None => Ok(0),
                Some(count_lengths) => {
                    max_serialized_size_impl(count_lengths, elements, schema, stack)
                }
            }
        }
        Ok(Definition::Sequence { elements }) => {
            // Assume that sequence has MAX_LEN elements since that’s the most
            // it can have.
            let sz = max_serialized_size_impl(MAX_LEN, elements, schema, stack)?;
            mul(count, add(sz, 4)?)
        }

        Ok(Definition::Enum {
            tag_width,
            variants,
        }) => {
            let mut max = 0;
            for (_, variant) in variants {
                let sz = max_serialized_size_impl(ONE, variant, schema, stack)?;
                max = max.max(sz);
            }
            max.checked_add(usize::from(*tag_width))
                .ok_or(Error::Overflow)
        }

        // Tuples and structs sum sizes of all the members.
        Ok(Definition::Tuple { elements }) => tuple(count, elements, schema, stack),
        Ok(Definition::Struct { fields }) => match fields {
            Fields::NamedFields(fields) => {
                tuple(count, fields.iter().map(|(_, field)| field), schema, stack)
            }
            Fields::UnnamedFields(fields) => tuple(count, fields, schema, stack),
            Fields::Empty => Ok(0),
        },

        // string is just Vec<u8>
        // "string" definition is added in another pr
        Err("string") => mul(count, add(MAX_LEN.get(), 4)?),

        Err(declaration) => Err(Error::MissingDefinition(declaration.to_string())),
    }?;

    stack.pop();
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
pub(super) fn is_zero_size(
    declaration: &Declaration,
    schema: &BorshSchemaContainer,
) -> Result<bool, ZeroSizeError> {
    let mut stack = Vec::new();
    is_zero_size_impl(declaration, schema, &mut stack)
}

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

fn is_zero_size_impl<'a>(
    declaration: &'a str,
    schema: &'a BorshSchemaContainer,
    stack: &mut Vec<&'a str>,
) -> Result<bool, ZeroSizeError> {
    fn all<'a, T: 'a>(
        iter: impl Iterator<Item = T>,
        f_key: impl Fn(&T) -> &'a Declaration,
        schema: &'a BorshSchemaContainer,
        stack: &mut Vec<&'a str>,
    ) -> Result<bool, ZeroSizeError> {
        for element in iter {
            let declaration = f_key(&element);
            if !is_zero_size_impl(declaration.as_str(), schema, stack)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    if stack.iter().any(|dec| *dec == declaration) {
        return Err(ZeroSizeError::Recursive);
    }
    stack.push(declaration);

    let res = match schema.get_definition(declaration).ok_or(declaration) {
        Ok(Definition::Primitive(size)) => *size == 0,
        Ok(Definition::Array { length, elements }) => {
            *length == 0 || is_zero_size_impl(elements.as_str(), schema, stack)?
        }
        Ok(Definition::Sequence { .. }) => false,
        Ok(Definition::Tuple { elements }) => all(elements.iter(), |key| *key, schema, stack)?,
        Ok(Definition::Enum {
            tag_width: 0,
            variants,
        }) => all(
            variants.iter(),
            |(_variant_name, declaration)| declaration,
            schema,
            stack,
        )?,
        Ok(Definition::Enum { .. }) => false,
        Ok(Definition::Struct { fields }) => match fields {
            Fields::NamedFields(fields) => all(
                fields.iter(),
                |(_field_name, declaration)| declaration,
                schema,
                stack,
            )?,
            Fields::UnnamedFields(fields) => {
                all(fields.iter(), |declaration| declaration, schema, stack)?
            }
            Fields::Empty => true,
        },

        // another pr removes this exclusion rule
        Err("string") => false,

        Err(declaration) => {
            return Err(ZeroSizeError::MissingDefinition(declaration.into()));
        }
    };
    stack.pop();
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    // this is not integration test module, so can use __private for ease of imports;
    // it cannot be made integration, as it tests `is_zero_size` function, chosen to be non-pub
    use crate::{
        BorshSchema,
        __private::maybestd::{
            boxed::Box,
            collections::BTreeMap,
            format,
            string::{String, ToString},
            vec,
        },
    };

    #[track_caller]
    fn test_ok<T: BorshSchema>(want: usize) {
        let schema = BorshSchemaContainer::for_type::<T>();
        assert_eq!(Ok(want), schema.max_serialized_size());
        assert_eq!(
            want == 0,
            is_zero_size(schema.declaration(), &schema).unwrap()
        );
    }

    #[track_caller]
    fn test_err<T: BorshSchema>(err: Error) {
        let schema = BorshSchemaContainer::for_type::<T>();
        assert_eq!(Err(err), schema.max_serialized_size());
    }

    const MAX_LEN: usize = u32::MAX as usize;

    #[test]
    fn test_is_zero_size_recursive_check_bypassed() {
        use crate as borsh;

        #[derive(::borsh_derive::BorshSchema)]
        struct RecursiveExitSequence(Vec<RecursiveExitSequence>);

        let schema = BorshSchemaContainer::for_type::<RecursiveExitSequence>();
        assert_eq!(Ok(false), is_zero_size(schema.declaration(), &schema));
    }

    #[test]
    fn test_is_zero_size_recursive_check_err() {
        use crate as borsh;

        #[derive(::borsh_derive::BorshSchema)]
        struct RecursiveNoExitStructUnnamed(Box<RecursiveNoExitStructUnnamed>);

        let schema = BorshSchemaContainer::for_type::<RecursiveNoExitStructUnnamed>();
        assert_eq!(
            Err(ZeroSizeError::Recursive),
            is_zero_size(schema.declaration(), &schema)
        );
    }

    #[test]
    fn max_serialized_size_primitives() {
        test_ok::<()>(0);
        test_ok::<bool>(1);

        test_ok::<f32>(4);
        test_ok::<f64>(8);

        test_ok::<i8>(1);
        test_ok::<i16>(2);
        test_ok::<i32>(4);
        test_ok::<i64>(8);
        test_ok::<i128>(16);

        test_ok::<u8>(1);
        test_ok::<u16>(2);
        test_ok::<u32>(4);
        test_ok::<u64>(8);
        test_ok::<u128>(16);

        test_ok::<core::num::NonZeroI8>(1);
        test_ok::<core::num::NonZeroI16>(2);
        test_ok::<core::num::NonZeroI32>(4);
        test_ok::<core::num::NonZeroI64>(8);
        test_ok::<core::num::NonZeroI128>(16);

        test_ok::<core::num::NonZeroU8>(1);
        test_ok::<core::num::NonZeroU16>(2);
        test_ok::<core::num::NonZeroU32>(4);
        test_ok::<core::num::NonZeroU64>(8);
        test_ok::<core::num::NonZeroU128>(16);

        test_ok::<isize>(8);
        test_ok::<usize>(8);
        test_ok::<core::num::NonZeroUsize>(8);
    }

    #[test]
    fn max_serialized_size_built_in_types() {
        test_ok::<core::ops::RangeFull>(0);
        test_ok::<core::ops::RangeInclusive<u8>>(2);
        test_ok::<core::ops::RangeToInclusive<u64>>(8);

        test_ok::<Option<()>>(1);
        test_ok::<Option<u8>>(2);
        test_ok::<Result<u8, usize>>(9);
        test_ok::<Result<u8, Vec<u8>>>(1 + 4 + MAX_LEN);

        test_ok::<()>(0);
        test_ok::<(u8,)>(1);
        test_ok::<(u8, u32)>(5);

        test_ok::<[u8; 0]>(0);
        test_ok::<[u8; 16]>(16);
        test_ok::<[[u8; 4]; 4]>(16);

        test_ok::<Vec<u8>>(4 + MAX_LEN);
        test_ok::<String>(4 + MAX_LEN);

        test_err::<Vec<Vec<u8>>>(Error::Overflow);
        test_ok::<Vec<Vec<()>>>(4 + MAX_LEN * 4);
        test_ok::<[[[(); MAX_LEN]; MAX_LEN]; MAX_LEN]>(0);
    }

    #[test]
    fn max_serialized_size_derived_types() {
        use crate as borsh;

        #[derive(::borsh_derive::BorshSchema)]
        pub struct Empty;

        #[derive(::borsh_derive::BorshSchema)]
        pub struct Named {
            _foo: usize,
            _bar: [u8; 15],
        }

        #[derive(::borsh_derive::BorshSchema)]
        pub struct Unnamed(usize, [u8; 15]);

        #[derive(::borsh_derive::BorshSchema)]
        struct Multiple {
            _usz0: usize,
            _usz1: usize,
            _usz2: usize,
            _vec0: Vec<usize>,
            _vec1: Vec<usize>,
        }

        #[derive(::borsh_derive::BorshSchema)]
        struct Recursive(Option<Box<Recursive>>);

        test_ok::<Empty>(0);
        test_ok::<Named>(23);
        test_ok::<Unnamed>(23);
        test_ok::<Multiple>(3 * 8 + 2 * (4 + MAX_LEN * 8));
        test_err::<BorshSchemaContainer>(Error::Overflow);
        test_err::<Recursive>(Error::Recursive);
    }

    #[test]
    fn max_serialized_size_custom_enum() {
        #[allow(dead_code)]
        enum Maybe<const N: usize, T> {
            Just(T),
            Nothing,
        }

        impl<const N: usize, T: BorshSchema> BorshSchema for Maybe<N, T> {
            fn declaration() -> Declaration {
                let res = format!(r#"Maybe<{}>"#, T::declaration());
                res
            }
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                let definition = Definition::Enum {
                    tag_width: N as u8,
                    variants: vec![
                        ("Just".into(), T::declaration()),
                        ("Nothing".into(), "nil".into()),
                    ],
                };
                crate::schema::add_definition(Self::declaration(), definition, definitions);
                T::add_definitions_recursively(definitions);
                <()>::add_definitions_recursively(definitions);
            }
        }

        test_ok::<Maybe<0, ()>>(0);
        test_ok::<Maybe<0, u16>>(2);
        test_ok::<Maybe<0, u64>>(8);

        test_ok::<Maybe<1, ()>>(1);
        test_ok::<Maybe<1, u16>>(3);
        test_ok::<Maybe<1, u64>>(9);

        test_ok::<Maybe<4, ()>>(4);
        test_ok::<Maybe<4, u16>>(6);
        test_ok::<Maybe<4, u64>>(12);
    }
}
