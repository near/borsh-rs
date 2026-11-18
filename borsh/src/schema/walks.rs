//! Walks over a schema container: the largest encoding, and validation. A container can come
//! from untrusted bytes, so no walk recurses (a deep chain can't overflow the stack) and each
//! walks a declaration once however often it is referenced (shared declarations can't blow
//! up the walk). Each keeps the declarations it has yet to visit on the heap.
use super::{BorshSchemaContainer, Declaration, Definition, Fields};
use crate::__private::maybestd::{
    collections::{BTreeMap, BTreeSet},
    string::ToString,
    vec,
    vec::Vec,
};
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

/// The declarations `definition` refers to, last first, so that popping them visits them in
/// order.
fn members(definition: &Definition) -> Vec<&Declaration> {
    let mut members: Vec<_> = match definition {
        Definition::Sequence { elements, .. } => vec![elements],
        Definition::Tuple { elements }
        | Definition::Struct {
            fields: Fields::UnnamedFields(elements),
        } => elements.iter().collect(),
        Definition::Struct {
            fields: Fields::NamedFields(fields),
        } => fields.iter().map(|(_, d)| d).collect(),
        Definition::Enum { variants, .. } => variants.iter().map(|(_, _, d)| d).collect(),
        Definition::Primitive(_)
        | Definition::Struct {
            fields: Fields::Empty,
        } => Vec::new(),
    };
    members.reverse();
    members
}

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
    pub fn max_serialized_size(&self) -> Result<usize, M> {
        // The size of one value of each declaration sized so far. Any error ends the walk, so
        // only sizes are kept.
        let mut sizes = BTreeMap::new();
        let mut zero_sizes = ZeroSizes::new();
        // The declarations being sized: `path` to find recursion, `frames` to resume them.
        let mut path = BTreeSet::new();
        let mut frames: Vec<Frame> = Vec::new();
        let mut next = Some(self.declaration().as_str());
        // The size of the declaration reached last, once known.
        let mut size = None;
        loop {
            if let Some(decl) = next.take() {
                size = sizes.get(decl).copied();
                if size.is_none() {
                    if !path.insert(decl) {
                        return Err(M::Recursive);
                    }
                    frames.push(Frame::new(decl, self, &path, &mut zero_sizes)?);
                }
            }
            let Some(frame) = frames.last_mut() else {
                return Ok(size.expect("the root declaration is sized"));
            };
            if let Some(member) = size.take() {
                frame.acc = match frame.fold {
                    Fold::Sum => frame.acc.checked_add(member),
                    Fold::Max => Some(frame.acc.max(member)),
                    Fold::Times(count) => count.checked_mul(member),
                }
                .ok_or(M::Overflow)?;
            }
            if let Some(member) = frame.members.pop() {
                next = Some(member.as_str());
                continue;
            }
            let (decl, total) = (frame.decl, frame.acc.checked_add(frame.extra));
            let total = total.ok_or(M::Overflow)?;
            frames.pop();
            path.remove(decl);
            sizes.insert(decl, total);
            size = Some(total);
        }
    }

    /// Validates container for violation of any well-known rules with
    /// respect to `borsh` serialization.
    ///
    /// Every declaration must be defined, tags and lengths must have valid widths, and no
    /// sequence may hold elements that encode to zero bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use borsh::schema::BorshSchemaContainer;
    ///
    /// let schema = BorshSchemaContainer::for_type::<usize>();
    /// assert_eq!(Ok(()), schema.validate());
    /// ```
    pub fn validate(&self) -> Result<(), V> {
        // Every declaration reached so far, checked once however often it is referenced. The
        // first error is the same as a walk that skips only the declarations on its path:
        // everything reachable from a visited declaration is checked already or on the path.
        let mut visited = BTreeSet::new();
        let mut zero_sizes = ZeroSizes::new();
        // Declarations left to check, the next one last.
        let mut pending = vec![self.declaration()];
        while let Some(decl) = pending.pop() {
            let definition = self
                .get_definition(decl)
                .ok_or_else(|| V::MissingDefinition(decl.to_string()))?;
            if visited.insert(decl) {
                check(decl, definition, self, &mut zero_sizes)?;
                pending.extend(members(definition));
            }
        }
        Ok(())
    }
}

/// A declaration that `max_serialized_size` is sizing.
struct Frame<'a> {
    decl: &'a str,
    /// Members left to size, the next one last.
    members: Vec<&'a Declaration>,
    /// How member sizes combine into `acc`.
    fold: Fold,
    acc: usize,
    /// Added once all members are sized: a tag or length width, or a primitive's size.
    extra: usize,
}

enum Fold {
    Sum,
    Max,
    Times(usize),
}

impl<'a> Frame<'a> {
    fn new(
        decl: &'a str,
        schema: &'a BorshSchemaContainer,
        path: &BTreeSet<&'a str>,
        zero_sizes: &mut ZeroSizes<'a>,
    ) -> Result<Self, M> {
        let definition = schema
            .get_definition(decl)
            .ok_or_else(|| M::MissingDefinition(decl.to_string()))?;
        let (members, fold, extra) = match definition {
            Definition::Primitive(size) => (Vec::new(), Fold::Sum, usize::from(*size)),
            Definition::Sequence {
                length_width,
                length_range,
                elements,
            } => {
                let width = usize::from(*length_width);
                // The longest sequence: `max_len` elements.
                match usize::try_from(*length_range.end()) {
                    Ok(0) => (Vec::new(), Fold::Sum, width),
                    Ok(max_len) => (vec![elements], Fold::Times(max_len), width),
                    Err(_) if is_zero_size(elements, schema, path, zero_sizes)? => {
                        (Vec::new(), Fold::Sum, width)
                    }
                    Err(_) => return Err(M::Overflow),
                }
            }
            Definition::Enum { tag_width, .. } => {
                (members(definition), Fold::Max, usize::from(*tag_width))
            }
            Definition::Tuple { .. } | Definition::Struct { .. } => {
                (members(definition), Fold::Sum, 0)
            }
        };
        Ok(Frame {
            decl,
            members,
            fold,
            acc: 0,
            extra,
        })
    }
}

/// The zero-size answers of one walk, so a declaration that many sequences hold is checked
/// once. `None` marks a check under way, or one that ran into recursion; reaching it again is
/// recursion, since each declaration on the path when recursion is found leads back into it.
type ZeroSizes<'a> = BTreeMap<&'a str, Option<bool>>;

/// Whether every value of `decl` encodes to zero bytes. Fails with `Recursive` (also on
/// reaching a declaration in `outer`, the path of the walk that asks) or `MissingDefinition`.
fn is_zero_size<'a>(
    decl: &'a str,
    schema: &'a BorshSchemaContainer,
    outer: &BTreeSet<&'a str>,
    cache: &mut ZeroSizes<'a>,
) -> Result<bool, M> {
    // Declarations being checked, each with the members left to check, the next one last.
    let mut frames: Vec<(&str, Vec<&Declaration>)> = Vec::new();
    let mut next = Some(decl);
    // The answer for the declaration reached last, once known.
    let mut zero = None;
    loop {
        if let Some(decl) = next.take() {
            if outer.contains(decl) {
                return Err(M::Recursive);
            }
            zero = match cache.get(decl) {
                Some(known) => Some(known.ok_or(M::Recursive)?),
                None => {
                    let definition = schema
                        .get_definition(decl)
                        .ok_or_else(|| M::MissingDefinition(decl.to_string()))?;
                    match zero_size_members(definition) {
                        Some(members) => {
                            cache.insert(decl, None);
                            frames.push((decl, members));
                            None
                        }
                        None => Some(false),
                    }
                }
            };
        }
        let Some((decl, members)) = frames.last_mut() else {
            return Ok(zero.expect("the first declaration is checked"));
        };
        if zero != Some(false) {
            if let Some(member) = members.pop() {
                next = Some(member.as_str());
                continue;
            }
        }
        let all_zero = zero != Some(false);
        cache.insert(decl, Some(all_zero));
        frames.pop();
        zero = Some(all_zero);
    }
}

/// The members of `definition`, last first, which all encode to zero bytes exactly when it
/// does; `None` if it never does.
fn zero_size_members(definition: &Definition) -> Option<Vec<&Declaration>> {
    match definition {
        Definition::Primitive(size) => (*size == 0).then(Vec::new),
        Definition::Sequence { length_width, .. } if *length_width != 0 => None,
        Definition::Sequence { length_range, .. } if *length_range == (0..=0) => Some(Vec::new()),
        Definition::Enum { tag_width, .. } if *tag_width != 0 => None,
        _ => Some(members(definition)),
    }
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

/// Checks `decl`'s own definition, not its members.
fn check<'a>(
    decl: &'a Declaration,
    definition: &'a Definition,
    schema: &'a BorshSchemaContainer,
    zero_sizes: &mut ZeroSizes<'a>,
) -> Result<(), V> {
    match definition {
        // A fixed-length array: no length to check.
        Definition::Sequence {
            length_width: Definition::ARRAY_LENGTH_WIDTH,
            length_range,
            ..
        } if !length_range.is_empty() && length_range.start() == length_range.end() => Ok(()),
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
            match is_zero_size(elements, schema, &BTreeSet::new(), zero_sizes) {
                Ok(true) => Err(V::ZSTSequence(decl.clone())),
                Err(M::MissingDefinition(d)) => Err(V::MissingDefinition(d)),
                Ok(false) | Err(_) => Ok(()),
            }
        }
        Definition::Enum { tag_width, .. } if *tag_width > 8 => {
            Err(V::TagTooWide(decl.to_string()))
        }
        _ => Ok(()),
    }
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
            is_zero_size(
                schema.declaration(),
                &schema,
                &BTreeSet::new(),
                &mut ZeroSizes::new()
            )
        );

        #[derive(::borsh_derive::BorshSchema)]
        #[allow(dead_code)]
        struct NoExit(Box<NoExit>);
        let schema = BorshSchemaContainer::for_type::<NoExit>();
        assert_eq!(
            Err(M::Recursive),
            is_zero_size(
                schema.declaration(),
                &schema,
                &BTreeSet::new(),
                &mut ZeroSizes::new()
            )
        );
    }
}
