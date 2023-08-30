use crate::__private::maybestd::{
    collections::BTreeMap,
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

pub fn schema_container_of<T: BorshSchema>() -> BorshSchemaContainer {
    let mut definitions = BTreeMap::new();
    T::add_definitions_recursively(&mut definitions);

    BorshSchemaContainer::new(T::declaration(), definitions)
}

/// Returns the largest possible size of serialised object.
///
/// Returns `None` if the maximal size overflows `usize`.  This may happen for
/// nested dynamically-sized types such as `Vec<Vec<u8>>` whose maximum size is
/// `4 + u32::MAX * (4 + u32::MAX)`.
///
/// The function has limitations which may lead it to overestimate the size.
/// For example, hypothetical `IPv4Packet` would be encoded as at most ~64 KiB.
/// However, if it uses sequence schema, this function will claim that the
/// maximum size is ~4 GiB.
///
/// # Example
///
/// ```
/// assert_eq!(Some(0), borsh::max_serialized_size::<()>());
/// assert_eq!(Some(8), borsh::max_serialized_size::<usize>());
///
/// // 4 bytes of length and u32::MAX for the longest possible string.
/// assert_eq!(Some(4 + 4294967295), borsh::max_serialized_size::<String>());
///
/// assert_eq!(None, borsh::max_serialized_size::<Vec<String>>());
/// ```
pub fn max_serialized_size<T: BorshSchema>() -> Option<usize> {
    let mut definitions = BTreeMap::new();
    T::add_definitions_recursively(&mut definitions);
    max_serialized_size_impl(1, T::declaration().as_str(), &definitions)
}

/// Checks whether given declaration schema serialises to an empty string.
///
/// Certain types always serialise to an empty string (most notably `()`).  This
/// function checks whether `declaration` is one of such types.
///
/// This is used by [`BorshSchema::max_serialized_size()`] to handle weird types
/// such as `[[[(); u32::MAX]; u32::MAX]; u32::MAX]` which serialises to an
/// empty string even though its number of elements overflows `usize`.
fn is_zero_size(declaration: &str, defs: &BTreeMap<Declaration, Definition>) -> bool {
    match defs.get(declaration).ok_or(declaration) {
        Ok(Definition::Array { length, elements }) => {
            *length == 0 || is_zero_size(elements.as_str(), defs)
        }
        Ok(Definition::Sequence { .. }) => false,
        Ok(Definition::Tuple { elements }) => elements
            .into_iter()
            .all(|element| is_zero_size(element.as_str(), defs)),
        Ok(Definition::Enum { .. }) => false,
        Ok(Definition::Struct { fields }) => match fields {
            Fields::NamedFields(fields) => fields
                .into_iter()
                .all(|(_, field)| is_zero_size(field.as_str(), defs)),
            Fields::UnnamedFields(fields) => fields
                .into_iter()
                .all(|field| is_zero_size(field.as_str(), defs)),
            Fields::Empty => true,
        },

        Err(dec) => dec == "nil",
    }
}

/// Implementation of [`BorshSchema::max_serialized_size`].
fn max_serialized_size_impl(
    count: usize,
    declaration: &str,
    defs: &BTreeMap<Declaration, Definition>,
) -> Option<usize> {
    use core::convert::TryFrom;

    /// Maximum number of elements in a vector or length of a string which can
    /// be serialised.
    ///
    /// This is `u32::MAX` cast to `usize`.
    const MAX_LEN: usize = 4294967295;

    /// Calculates max serialised size of a tuple with given members.
    fn tuple<'a>(
        count: usize,
        elements: impl core::iter::IntoIterator<Item = &'a Declaration>,
        defs: &BTreeMap<Declaration, Definition>,
    ) -> Option<usize> {
        let mut sum: usize = 0;
        for el in elements {
            sum = sum.checked_add(max_serialized_size_impl(1, el, defs)?)?;
        }
        count.checked_mul(sum)
    }

    match defs.get(declaration).ok_or(declaration) {
        Ok(Definition::Array { length, elements }) => {
            // Aggregate `count` and `length` to a single number.  If this
            // overflows, check if array’s element is zero-sized.
            let count = usize::try_from(*length)
                .ok()
                .and_then(|len| len.checked_mul(count));
            match count {
                Some(0) => Some(0),
                Some(count) => max_serialized_size_impl(count, elements, defs),
                None if is_zero_size(elements, defs) => Some(0),
                None => None,
            }
        }
        Ok(Definition::Sequence { elements }) => {
            // Assume that sequence has MAX_LEN elements since that’s the most
            // it can have.
            let sz = max_serialized_size_impl(MAX_LEN, elements, defs)?;
            // Add four to account for encoded length.
            count.checked_mul(sz.checked_add(4)?)
        }

        Ok(Definition::Enum { variants }) => {
            // Size of an enum is the largest variant plus one for tag.
            let mut max = 0;
            for (_, variant) in variants {
                let sz = max_serialized_size_impl(1, variant, defs)?;
                max = max.max(sz);
            }
            max.checked_add(1)
        }

        // Tuples and structs sum sizes of all the members.
        Ok(Definition::Tuple { elements }) => tuple(count, elements, defs),
        Ok(Definition::Struct { fields }) => match fields {
            Fields::NamedFields(fields) => {
                tuple(count, fields.into_iter().map(|(_, field)| field), defs)
            }
            Fields::UnnamedFields(fields) => tuple(count, fields, defs),
            Fields::Empty => Some(0),
        },

        // Handle primitive types.  They have well-known sizes.
        Err("nil") => Some(0),
        Err("bool" | "i8" | "u8") => Some(count),
        Err("i16" | "u16") => count.checked_mul(2),
        Err("i32" | "u32" | "f32") => count.checked_mul(4),
        Err("i64" | "u64" | "f64") => count.checked_mul(8),
        Err("i128" | "u128") => count.checked_mul(16),

        // Assume string with maximum length.  This is equivalent to `Vec<u8>`.
        Err("string") => count.checked_mul(MAX_LEN.checked_add(4)?),

        Err(_) => None,
    }
}

#[test]
fn test_max_serialized_size() {
    #[cfg(not(feature = "std"))]
    use alloc::string::String;

    mod test_structs {
        use crate as borsh;
        #[cfg(not(feature = "std"))]
        use alloc::string::ToString as _;

        #[derive(::borsh_derive::BorshSchema)]
        pub struct TestEmpty;

        #[derive(::borsh_derive::BorshSchema)]
        pub struct TestNamed {
            _foo: usize,
            _bar: [u8; 15],
        }

        #[derive(::borsh_derive::BorshSchema)]
        pub struct TestUnnamed(usize, [u8; 15]);
    }

    const MAX_LEN: usize = 4294967295;

    assert_eq!(Some(2), max_serialized_size::<u16>());
    assert_eq!(Some(8), max_serialized_size::<usize>());

    assert_eq!(Some(1), max_serialized_size::<Option<()>>());
    assert_eq!(Some(2), max_serialized_size::<Option<u8>>());
    assert_eq!(
        Some(9),
        max_serialized_size::<core::result::Result<u8, usize>>()
    );

    assert_eq!(Some(0), max_serialized_size::<()>());
    assert_eq!(Some(1), max_serialized_size::<(u8,)>());
    assert_eq!(Some(5), max_serialized_size::<(u8, u32)>());

    assert_eq!(Some(0), max_serialized_size::<[u8; 0]>());
    assert_eq!(Some(16), max_serialized_size::<[u8; 16]>());
    assert_eq!(Some(16), max_serialized_size::<[[u8; 4]; 4]>());

    assert_eq!(Some(4 + MAX_LEN), max_serialized_size::<Vec<u8>>());
    assert_eq!(Some(4 + MAX_LEN), max_serialized_size::<String>());

    assert_eq!(None, max_serialized_size::<Vec<Vec<u8>>>());
    assert_eq!(Some(4 + MAX_LEN * 4), max_serialized_size::<Vec<Vec<()>>>());
    assert_eq!(
        Some(0),
        max_serialized_size::<[[[(); MAX_LEN]; MAX_LEN]; MAX_LEN]>()
    );

    use test_structs::*;

    assert_eq!(Some(0), max_serialized_size::<TestEmpty>());
    assert_eq!(Some(23), max_serialized_size::<TestNamed>());
    assert_eq!(Some(23), max_serialized_size::<TestUnnamed>());
    assert_eq!(None, max_serialized_size::<BorshSchemaContainer>());
}
