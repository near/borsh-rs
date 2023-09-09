use crate::__private::maybestd::{
    io::{Error, ErrorKind, Result},
    vec::Vec,
};
use crate::from_slice;
use crate::schema::{BorshSchemaContainer, Declaration, Definition, Fields};
use crate::{BorshDeserialize, BorshSchema, BorshSerialize};

/// Deserialize this instance from a slice of bytes, but assume that at the beginning we have
/// bytes describing the schema of the type. We deserialize this schema and verify that it is
/// correct.
pub fn try_from_slice_with_schema<T: BorshDeserialize + BorshSchema>(v: &[u8]) -> Result<T> {
    let (schema, object) = from_slice::<(BorshSchemaContainer, T)>(v)?;
    if schema_container_of::<T>() != schema {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Borsh schema does not match",
        ));
    }
    Ok(object)
}

/// Serialize object into a vector of bytes and prefix with the schema serialized as vector of
/// bytes in Borsh format.
pub fn try_to_vec_with_schema<T: BorshSerialize + BorshSchema>(value: &T) -> Result<Vec<u8>> {
    let schema = schema_container_of::<T>();
    let mut res = crate::to_vec(&schema)?;
    value.serialize(&mut res)?;
    Ok(res)
}

/// generate [BorshSchemaContainer] for type `T`
///
/// this is an alias of [BorshSchemaContainer::for_type]
pub fn schema_container_of<T: BorshSchema>() -> BorshSchemaContainer {
    BorshSchemaContainer::for_type::<T>()
}

/// Possible error when calculating theoretical maximum size of encoded type.
///
/// This is error returned by [`max_serialized_size`] function.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MaxSizeError {
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
    MissingDefinition,
}

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
/// assert_eq!(Ok(0), borsh::max_serialized_size(&schema));
///
/// let schema = BorshSchemaContainer::for_type::<usize>();
/// assert_eq!(Ok(8), borsh::max_serialized_size(&schema));
///
/// // 4 bytes of length and u32::MAX for the longest possible string.
/// let schema = BorshSchemaContainer::for_type::<String>();
/// assert_eq!(Ok(4 + 4294967295), borsh::max_serialized_size(&schema));
///
/// let schema = BorshSchemaContainer::for_type::<Vec<String>>();
/// assert_eq!(Err(borsh::MaxSizeError::Overflow),
///            borsh::max_serialized_size(&schema));
/// ```
pub fn max_serialized_size(
    schema: &BorshSchemaContainer,
) -> core::result::Result<usize, MaxSizeError> {
    let mut stack = Vec::new();
    max_serialized_size_impl(1, schema.declaration(), schema, &mut stack)
}

/// Checks whether given declaration schema serialises to an empty string.
///
/// Certain types always serialise to an empty string (most notably `()`).  This
/// function checks whether `declaration` is one of such types.
///
/// This is used by [`BorshSchema::max_serialized_size()`] to handle weird types
/// such as `[[[(); u32::MAX]; u32::MAX]; u32::MAX]` which serialises to an
/// empty string even though its number of elements overflows `usize`.
fn is_zero_size(declaration: &str, schema: &BorshSchemaContainer) -> bool {
    match schema.get_definition(declaration).ok_or(declaration) {
        Ok(Definition::Array { length, elements }) => {
            *length == 0 || is_zero_size(elements.as_str(), schema)
        }
        Ok(Definition::Sequence { .. }) => false,
        Ok(Definition::Tuple { elements }) => elements
            .iter()
            .all(|element| is_zero_size(element.as_str(), schema)),
        Ok(Definition::Enum { .. }) => false,
        Ok(Definition::Struct { fields }) => match fields {
            Fields::NamedFields(fields) => fields
                .iter()
                .all(|(_, field)| is_zero_size(field.as_str(), schema)),
            Fields::UnnamedFields(fields) => fields
                .iter()
                .all(|field| is_zero_size(field.as_str(), schema)),
            Fields::Empty => true,
        },

        Err(dec) => dec == "nil",
    }
}

/// Implementation of [`BorshSchema::max_serialized_size`].
fn max_serialized_size_impl<'a>(
    count: usize,
    declaration: &'a str,
    schema: &'a BorshSchemaContainer,
    stack: &mut Vec<&'a str>,
) -> core::result::Result<usize, MaxSizeError> {
    use core::convert::TryFrom;

    /// Maximum number of elements in a vector or length of a string which can
    /// be serialised.
    const MAX_LEN: usize = u32::MAX as usize;

    fn add(x: usize, y: usize) -> core::result::Result<usize, MaxSizeError> {
        x.checked_add(y).ok_or(MaxSizeError::Overflow)
    }

    fn mul(x: usize, y: usize) -> core::result::Result<usize, MaxSizeError> {
        x.checked_mul(y).ok_or(MaxSizeError::Overflow)
    }

    /// Calculates max serialised size of a tuple with given members.
    fn tuple<'a>(
        count: usize,
        elements: impl core::iter::IntoIterator<Item = &'a Declaration>,
        schema: &'a BorshSchemaContainer,
        stack: &mut Vec<&'a str>,
    ) -> ::core::result::Result<usize, MaxSizeError> {
        let mut sum: usize = 0;
        for el in elements {
            sum = add(sum, max_serialized_size_impl(1, el, schema, stack)?)?;
        }
        mul(count, sum)
    }

    if stack.iter().any(|dec| *dec == declaration) {
        return Err(MaxSizeError::Recursive);
    }
    stack.push(declaration);

    let res = match schema.get_definition(declaration).ok_or(declaration) {
        Ok(Definition::Array { length, elements }) => {
            // Aggregate `count` and `length` to a single number.  If this
            // overflows, check if array’s element is zero-sized.
            let count = usize::try_from(*length)
                .ok()
                .and_then(|len| len.checked_mul(count));
            match count {
                Some(0) => Ok(0),
                Some(count) => max_serialized_size_impl(count, elements, schema, stack),
                None if is_zero_size(elements, schema) => Ok(0),
                None => Err(MaxSizeError::Overflow),
            }
        }
        Ok(Definition::Sequence { elements }) => {
            // Assume that sequence has MAX_LEN elements since that’s the most
            // it can have.
            let sz = max_serialized_size_impl(MAX_LEN, elements, schema, stack)?;
            mul(count, add(sz, 4)?)
        }

        Ok(Definition::Enum { variants }) => {
            // Size of an enum is the largest variant plus one for tag.
            let mut max = 0;
            for (_, variant) in variants {
                let sz = max_serialized_size_impl(1, variant, schema, stack)?;
                max = max.max(sz);
            }
            max.checked_add(1).ok_or(MaxSizeError::Overflow)
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

        // Primitive types.
        Err("nil") => Ok(0),
        Err("bool" | "i8" | "u8") => Ok(count),
        Err("i16" | "u16") => mul(count, 2),
        Err("i32" | "u32" | "f32") => mul(count, 4),
        Err("i64" | "u64" | "f64") => mul(count, 8),
        Err("i128" | "u128") => mul(count, 16),

        // string is just Vec<u8>
        Err("string") => mul(count, add(MAX_LEN, 4)?),

        Err(_) => Err(MaxSizeError::MissingDefinition),
    }?;

    stack.pop();
    Ok(res)
}

#[test]
fn test_max_serialized_size() {
    #[cfg(not(feature = "std"))]
    use alloc::{
        boxed::Box,
        string::{String, ToString},
    };

    #[track_caller]
    fn test_ok<T: BorshSchema>(want: usize) {
        let schema = borsh::schema::BorshSchemaContainer::for_type::<T>();
        assert_eq!(Ok(want), max_serialized_size(&schema));
    }

    #[track_caller]
    fn test_err<T: BorshSchema>(err: MaxSizeError) {
        let schema = borsh::schema::BorshSchemaContainer::for_type::<T>();
        assert_eq!(Err(err), max_serialized_size(&schema));
    }

    const MAX_LEN: usize = u32::MAX as usize;

    test_ok::<u16>(2);
    test_ok::<usize>(8);

    test_ok::<Option<()>>(1);
    test_ok::<Option<u8>>(2);
    test_ok::<core::result::Result<u8, usize>>(9);

    test_ok::<()>(0);
    test_ok::<(u8,)>(1);
    test_ok::<(u8, u32)>(5);

    test_ok::<[u8; 0]>(0);
    test_ok::<[u8; 16]>(16);
    test_ok::<[[u8; 4]; 4]>(16);

    test_ok::<Vec<u8>>(4 + MAX_LEN);
    test_ok::<String>(4 + MAX_LEN);

    test_err::<Vec<Vec<u8>>>(MaxSizeError::Overflow);
    test_ok::<Vec<Vec<()>>>(4 + MAX_LEN * 4);
    test_ok::<[[[(); MAX_LEN]; MAX_LEN]; MAX_LEN]>(0);

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
    test_err::<BorshSchemaContainer>(MaxSizeError::Overflow);
    test_err::<Recursive>(MaxSizeError::Recursive);
}
