//! Walks over a schema container: the largest encoding, and validation. Both are depth-first,
//! with the declarations on the current path as the recursion check.
use super::{BorshSchemaContainer, Declaration, Definition, Fields};
use crate::__private::maybestd::{string::ToString, vec, vec::Vec};
use core::convert::TryFrom;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SchemaMaxSerializedSizeError {
    /// The size doesn't fit in `usize`.
    Overflow,
    /// The type is recursive, so it has no largest size.
    Recursive,
    /// A declaration has no definition.
    MissingDefinition(Declaration),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SchemaContainerValidateError {
    /// A sequence of elements that encode to zero bytes.
    ZSTSequence(Declaration),
    /// A tag or length wider than 8 bytes.
    TagTooWide(Declaration),
    /// A length too narrow for the largest length allowed.
    TagTooNarrow(Declaration),
    /// A length width of 3, 5, 6 or 7 bytes.
    TagNotPowerOfTwo(Declaration),
    /// A declaration has no definition.
    MissingDefinition(Declaration),
    /// A sequence whose length range is empty.
    EmptyLengthRange(Declaration),
}

use SchemaContainerValidateError as V;
use SchemaMaxSerializedSizeError as M;

/// The declarations a tuple, struct or enum holds, in order. Other definitions hold none here;
/// each walk handles sequences itself.
fn members(definition: &Definition) -> Vec<&Declaration> {
    match definition {
        Definition::Tuple { elements }
        | Definition::Struct {
            fields: Fields::UnnamedFields(elements),
        } => elements.iter().collect(),
        Definition::Struct {
            fields: Fields::NamedFields(fields),
        } => fields.iter().map(|(_, d)| d).collect(),
        Definition::Enum { variants, .. } => variants.iter().map(|(_, _, d)| d).collect(),
        _ => Vec::new(),
    }
}

impl BorshSchemaContainer {
    /// The largest encoding of any value of the declared type.
    ///
    /// ```
    /// use borsh::schema::BorshSchemaContainer;
    /// let schema = BorshSchemaContainer::for_type::<Vec<u8>>();
    /// assert_eq!(Ok(4 + u32::MAX as usize), schema.max_serialized_size());
    /// ```
    pub fn max_serialized_size(&self) -> Result<usize, M> {
        max_size(1, self.declaration(), self, &mut Vec::new())
    }

    /// Check the schema is well-formed: every declaration is defined, tags and lengths have
    /// valid widths, and no sequence holds elements that encode to zero bytes.
    pub fn validate(&self) -> Result<(), V> {
        validate(self.declaration(), self, &mut Vec::new())
    }
}

/// Look up `decl` and push it on the path of declarations being walked; a declaration
/// already on the path is a cycle.
fn enter<'a>(
    decl: &'a str,
    schema: &'a BorshSchemaContainer,
    path: &mut Vec<&'a str>,
) -> Result<&'a Definition, M> {
    if path.contains(&decl) {
        return Err(M::Recursive);
    }
    path.push(decl);
    schema
        .get_definition(decl)
        .ok_or_else(|| M::MissingDefinition(decl.to_string()))
}

/// The walk behind `max_serialized_size`. `count` is at least 1.
fn max_size<'a>(
    count: usize,
    decl: &'a str,
    schema: &'a BorshSchemaContainer,
    path: &mut Vec<&'a str>,
) -> Result<usize, M> {
    let definition = enter(decl, schema, path)?;
    let mut size_of_one = |decl| max_size(1, decl, schema, path);
    let size = match definition {
        Definition::Primitive(size) => count.checked_mul(usize::from(*size)),
        Definition::Sequence {
            length_width,
            length_range,
            elements,
        } => {
            let items = match usize::try_from(*length_range.end()) {
                Ok(0) => 0,
                Ok(len) => max_size(len, elements, schema, path)?,
                Err(_) if is_zero_size(elements, schema, path)? => 0,
                Err(_) => return Err(M::Overflow),
            };
            let one = items.checked_add(usize::from(*length_width));
            one.ok_or(M::Overflow)?.checked_mul(count)
        }
        Definition::Enum { tag_width, .. } => {
            let mut max = 0;
            for variant in members(definition) {
                max = max.max(size_of_one(variant)?);
            }
            max.checked_add(usize::from(*tag_width))
        }
        Definition::Tuple { .. } | Definition::Struct { .. } => {
            let mut sum = 0usize;
            for member in members(definition) {
                sum = sum.checked_add(size_of_one(member)?).ok_or(M::Overflow)?;
            }
            sum.checked_mul(count)
        }
    };
    path.pop();
    size.ok_or(M::Overflow)
}

/// Whether every value of `decl` encodes to zero bytes. Fails with `Recursive` or
/// `MissingDefinition`.
fn is_zero_size<'a>(
    decl: &'a str,
    schema: &'a BorshSchemaContainer,
    path: &mut Vec<&'a str>,
) -> Result<bool, M> {
    let definition = enter(decl, schema, path)?;
    let zero = match definition {
        Definition::Primitive(size) => *size == 0,
        Definition::Sequence {
            length_width,
            length_range,
            elements,
        } => {
            *length_width == 0
                && ((length_range.clone().count() == 1 && *length_range.start() == 0)
                    || is_zero_size(elements, schema, path)?)
        }
        Definition::Enum { tag_width, .. } if *tag_width != 0 => false,
        _ => {
            let mut all = true;
            for member in members(definition) {
                if !is_zero_size(member, schema, path)? {
                    all = false;
                    break;
                }
            }
            all
        }
    };
    path.pop();
    Ok(zero)
}

/// A `width`-byte length must hold `max`; widths are 0 (none), 1, 2, 4 or 8.
fn check_length_width(decl: &Declaration, width: u8, max: u64) -> Result<(), V> {
    match width {
        0 | 8 => Ok(()),
        3 | 5 | 6 | 7 => Err(V::TagNotPowerOfTwo(decl.clone())),
        1 | 2 | 4 if max < 1 << (width * 8) => Ok(()),
        1 | 2 | 4 => Err(V::TagTooNarrow(decl.clone())),
        _ => Err(V::TagTooWide(decl.clone())),
    }
}

fn validate<'a>(
    decl: &'a Declaration,
    schema: &'a BorshSchemaContainer,
    path: &mut Vec<&'a Declaration>,
) -> Result<(), V> {
    let definition = schema
        .get_definition(decl)
        .ok_or_else(|| V::MissingDefinition(decl.to_string()))?;
    if path.contains(&decl) {
        return Ok(());
    }
    path.push(decl);
    let children = match definition {
        // A fixed-length array: no length to check.
        Definition::Sequence {
            length_width: Definition::ARRAY_LENGTH_WIDTH,
            length_range,
            elements,
        } if length_range.clone().count() == 1 => vec![elements],
        Definition::Sequence {
            length_width,
            length_range,
            elements,
        } => {
            if length_range.is_empty() {
                return Err(V::EmptyLengthRange(decl.clone()));
            }
            check_length_width(decl, *length_width, *length_range.end())?;
            // A recursive element type isn't zero-sized.
            match is_zero_size(elements, schema, &mut Vec::new()) {
                Ok(true) => return Err(V::ZSTSequence(decl.clone())),
                Err(M::MissingDefinition(d)) => return Err(V::MissingDefinition(d)),
                Ok(false) | Err(_) => {}
            }
            vec![elements]
        }
        Definition::Enum { tag_width, .. } if *tag_width > 8 => {
            return Err(V::TagTooWide(decl.to_string()))
        }
        _ => members(definition),
    };
    for child in children {
        validate(child, schema, path)?;
    }
    path.pop();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::__private::maybestd::string::ToString;

    #[test]
    fn length_widths() {
        let narrow: Result<(), V> = Err(V::TagTooNarrow("t".to_string()));
        let odd: Result<(), V> = Err(V::TagNotPowerOfTwo("t".to_string()));
        for (width, max, want) in [
            (0, u64::MAX, Ok(())),
            (1, u8::MAX as u64, Ok(())),
            (1, u8::MAX as u64 + 1, narrow.clone()),
            (2, u16::MAX as u64, Ok(())),
            (2, u16::MAX as u64 + 1, narrow.clone()),
            (3, 100, odd.clone()),
            (4, u32::MAX as u64, Ok(())),
            (4, u32::MAX as u64 + 1, narrow),
            (5, 100, odd.clone()),
            (8, u64::MAX, Ok(())),
            (9, 0, Err(V::TagTooWide("t".to_string()))),
        ] {
            assert_eq!(
                want,
                check_length_width(&"t".into(), width, max),
                "width={width}; max={max}"
            );
        }
    }

    #[test]
    fn zero_size_recursion() {
        use crate as borsh;
        use crate::__private::maybestd::boxed::Box;

        #[derive(::borsh_derive::BorshSchema)]
        #[allow(dead_code)]
        struct ExitsThroughSequence(Vec<ExitsThroughSequence>);
        let schema = BorshSchemaContainer::for_type::<ExitsThroughSequence>();
        assert_eq!(
            Ok(false),
            is_zero_size(schema.declaration(), &schema, &mut Vec::new())
        );

        #[derive(::borsh_derive::BorshSchema)]
        #[allow(dead_code)]
        struct NoExit(Box<NoExit>);
        let schema = BorshSchemaContainer::for_type::<NoExit>();
        assert_eq!(
            Err(M::Recursive),
            is_zero_size(schema.declaration(), &schema, &mut Vec::new())
        );
    }
}
