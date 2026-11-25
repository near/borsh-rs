use super::{members, BorshSchemaContainer, Declaration, Definition};
use crate::__private::maybestd::{
    collections::{BTreeMap, BTreeSet},
    string::ToString,
    vec,
    vec::Vec,
};

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
        // Size of one value of each declaration computed so far, so that one
        // referenced many times is walked once.  A size doesn't depend on the
        // path it was computed on, and any error ends the walk, so only sizes
        // are kept.
        let mut sizes = BTreeMap::new();
        let mut zero_size = ZeroSizeCache::new();
        // Declarations whose sizes are being computed.
        let mut path = BTreeSet::new();
        let mut frames: Vec<Frame> = Vec::new();
        let mut next = Some(self.declaration().as_str());
        // Size of the declaration reached last, once known.
        let mut size = None;
        loop {
            if let Some(declaration) = next.take() {
                size = sizes.get(declaration).copied();
                if size.is_none() {
                    if !path.insert(declaration) {
                        return Err(Error::Recursive);
                    }
                    frames.push(Frame::new(declaration, self, &path, &mut zero_size)?);
                }
            }
            let Some(frame) = frames.last_mut() else {
                return Ok(size.expect("the root declaration has been sized"));
            };
            if let Some(member_size) = size.take() {
                frame.acc = match frame.fold {
                    Fold::Sum => add(frame.acc, member_size)?,
                    Fold::Max => frame.acc.max(member_size),
                    Fold::Times(count) => mul(count, member_size)?,
                };
            }
            if let Some(member) = frame.members.pop() {
                next = Some(member.as_str());
                continue;
            }
            let declaration = frame.declaration;
            let res = add(frame.acc, frame.extra)?;
            frames.pop();
            path.remove(declaration);
            sizes.insert(declaration, res);
            size = Some(res);
        }
    }
}

/// A declaration whose size [`BorshSchemaContainer::max_serialized_size`] is
/// computing, kept on the heap rather than recursing so that deeply nested
/// definitions can't overflow the stack.
struct Frame<'a> {
    declaration: &'a str,
    /// Members left to size, the next one last.
    members: Vec<&'a Declaration>,
    /// How member sizes combine into `acc`.
    fold: Fold,
    acc: usize,
    /// Added to `acc` once all members are sized: the width of a tag or a
    /// length, or the size of a primitive.
    extra: usize,
}

enum Fold {
    Sum,
    Max,
    Times(usize),
}

impl<'a> Frame<'a> {
    fn new(
        declaration: &'a str,
        schema: &'a BorshSchemaContainer,
        path: &BTreeSet<&'a str>,
        zero_size: &mut ZeroSizeCache<'a>,
    ) -> Result<Self, Error> {
        use core::convert::TryFrom;

        let definition = schema
            .get_definition(declaration)
            .ok_or_else(|| Error::MissingDefinition(declaration.to_string()))?;
        let (members, fold, extra) = match definition {
            Definition::Primitive(size) => (Vec::new(), Fold::Sum, usize::from(*size)),
            Definition::Sequence {
                length_width,
                length_range,
                elements,
            } => {
                let width = usize::from(*length_width);
                // Assume sequence has the maximum number of elements.
                match usize::try_from(*length_range.end()) {
                    Ok(0) => (Vec::new(), Fold::Sum, width),
                    Ok(max_len) => (vec![elements], Fold::Times(max_len), width),
                    Err(_) if is_zero_size_impl(elements, schema, path, zero_size)? => {
                        (Vec::new(), Fold::Sum, width)
                    }
                    Err(_) => return Err(Error::Overflow),
                }
            }
            Definition::Enum { tag_width, .. } => {
                (members(definition), Fold::Max, usize::from(*tag_width))
            }
            // Tuples and structs sum sizes of all the members.
            Definition::Tuple { .. } | Definition::Struct { .. } => {
                (members(definition), Fold::Sum, 0)
            }
        };
        Ok(Frame {
            declaration,
            members,
            fold,
            acc: 0,
            extra,
        })
    }
}

fn add(x: usize, y: usize) -> Result<usize, Error> {
    x.checked_add(y).ok_or(Error::Overflow)
}

fn mul(x: usize, y: usize) -> Result<usize, Error> {
    x.checked_mul(y).ok_or(Error::Overflow)
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
    is_zero_size_impl(declaration, schema, &BTreeSet::new(), cache)
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
    outer: &BTreeSet<&'a str>,
    cache: &mut ZeroSizeCache<'a>,
) -> Result<bool, ZeroSizeError> {
    // Declarations being checked, each with the members left to check, the
    // next one last.  Kept on the heap rather than recursing, so that deeply
    // nested definitions can't overflow the stack.
    let mut frames: Vec<(&str, Vec<&Declaration>)> = Vec::new();
    let mut next = Some(declaration);
    // Answer for the declaration reached last, once known.
    let mut zero = None;
    loop {
        if let Some(declaration) = next.take() {
            if outer.contains(declaration) {
                return Err(ZeroSizeError::Recursive);
            }
            zero = match cache.get(declaration) {
                Some(known) => Some(known.ok_or(ZeroSizeError::Recursive)?),
                None => {
                    let definition = schema
                        .get_definition(declaration)
                        .ok_or_else(|| ZeroSizeError::MissingDefinition(declaration.into()))?;
                    match zero_size_members(definition) {
                        Some(members) => {
                            cache.insert(declaration, None);
                            frames.push((declaration, members));
                            None
                        }
                        None => Some(false),
                    }
                }
            };
        }
        let Some((declaration, members)) = frames.last_mut() else {
            return Ok(zero.expect("the first declaration has been checked"));
        };
        if zero != Some(false) {
            if let Some(member) = members.pop() {
                next = Some(member.as_str());
                continue;
            }
        }
        let res = zero != Some(false);
        cache.insert(declaration, Some(res));
        frames.pop();
        zero = Some(res);
    }
}

/// Returns the members of `definition`, last first, which are all zero-sized
/// exactly when it is, or `None` if it never is.
fn zero_size_members(definition: &Definition) -> Option<Vec<&Declaration>> {
    match definition {
        Definition::Primitive(size) => (*size == 0).then(Vec::new),
        Definition::Sequence { length_width, .. } if *length_width != 0 => None,
        Definition::Sequence { length_range, .. } if *length_range == (0..=0) => Some(Vec::new()),
        Definition::Enum { tag_width, .. } if *tag_width != 0 => None,
        _ => Some(members(definition)),
    }
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
