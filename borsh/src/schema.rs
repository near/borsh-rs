//!
//! Since Borsh is not a self-descriptive format we have a way to describe types serialized with Borsh so that
//! we can deserialize serialized blobs without having Rust types available. Additionally, this can be used to
//! serialize content provided in a different format, e.g. JSON object `{"user": "alice", "message": "Message"}`
//! can be serialized by JS code into Borsh format such that it can be deserialized into `struct UserMessage {user: String, message: String}`
//! on Rust side.
//!
//! The important components are: `BorshSchema` trait, `Definition` and `Declaration` types, and `BorshSchemaContainer` struct.
//! * `BorshSchema` trait allows any type that implements it to be self-descriptive, i.e. generate it's own schema;
//! * `Declaration` is used to describe the type identifier, e.g. `HashMap<u64, String>`;
//! * `Definition` is used to describe the structure of the type;
//! * `BorshSchemaContainer` is used to store all declarations and definitions that are needed to work with a single type.

#![allow(dead_code)] // Unclear why rust check complains on fields of `Definition` variants.
use crate as borsh; // For `#[derive(BorshSerialize, BorshDeserialize)]`.
use crate::__private::maybestd::{
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use crate::io::{Read, Result as IOResult, Write};
use crate::{BorshDeserialize, BorshSchema as BorshSchemaMacro, BorshSerialize};
use core::borrow::Borrow;
use core::cmp::Ord;
use core::marker::PhantomData;

mod container_ext;

pub use container_ext::{SchemaContainerValidateError, SchemaMaxSerializedSizeError};

/// The type that we use to represent the declaration of the Borsh type.
pub type Declaration = String;
/// The type that we use for the name of the variant.
pub type VariantName = String;
/// The type that we use for value of discriminant.
pub type DiscriminantValue = i64;
/// The name of the field in the struct (can be used to convert JSON to Borsh using the schema).
pub type FieldName = String;
/// The type that we use to represent the definition of the Borsh type.

/// Description of data encoding on the wire.
#[derive(Clone, PartialEq, Eq, Debug, BorshSerialize, BorshDeserialize, BorshSchemaMacro)]
pub enum Definition {
    /// A fixed-size type, which is considered undivisible
    Primitive(u8),

    /// A sequence of homogeneous elements.
    ///
    /// If `length_width` is non-zero, the sequence is tagged, i.e. prefixed by
    /// the number of elements in the sequence.  In that case, the length is
    /// encoded as a `length_width`-byte wide little-endian unsigned integer.
    ///
    /// If `length_width` is zero, the sequence is untagged.  In that case, if
    /// `length_range` contains a single number, the sequence is fixed-sized
    /// with the range determining number of elements.  Otherwise, knowledge of
    /// the type is necessary to be able to decode the number of elements.
    ///
    /// Prototypical examples of the use of this definitions are:
    /// * `[T; N]` → `length_width: 0, length_range: N..=N, elements: "T"` and
    /// * `Vec<T>` → `length_width: 4, length_range: 0..=u32::MAX,
    ///   elements: "T"`.
    ///
    /// With `length_width` and `length_range` other custom encoding formats can
    /// also be expressed.  For example:
    /// * `BoundedVec<LO, HI, T>` → `length_width: 4, length_range: LO..=HI`;
    /// * `PascalString` → `length_width: 1, length_range: 0..=255`;
    /// * `Ipv4Packet` → `length_width: 0, length_range: 20..=65536` or
    /// * `VarInt<u32>` → `length_width: 0, length_range: 1..=5`.
    Sequence {
        /// How many bytes does the length tag occupy.
        ///
        /// Zero if this is fixed-length array or the length must be determined
        /// by means not specified in the schema.  The schema is invalid if the
        /// value is greater than eight.
        length_width: u8,

        /// Bounds on the possible lengths of the sequence.
        ///
        /// Note: The schema is invalid if the range is empty or `length_width`
        /// is non-zero and either bound of the range cannot be represented as
        /// `length_width`-byte-wide unsigned integer.
        length_range: core::ops::RangeInclusive<u64>,

        /// Type of each element of the sequence.
        elements: Declaration,
    },

    /// A fixed-size tuple with the length known at the compile time and the elements of different
    /// types.
    Tuple { elements: Vec<Declaration> },

    /// A possibly tagged union, a.k.a enum.
    ///
    /// Tagged unions are prefixed by a tag identifying encoded variant followed
    /// by encoding of that variant.  The tag is `tag_width`-byte wide
    /// little-endian number.
    ///
    /// Untagged unions don’t have a separate tag which means that knowledge of
    /// the type is necessary to fully analyse the binary.  Variants may still
    /// be used to list possible values or determine the longest possible
    /// encoding.
    Enum {
        /// Width in bytes of the discriminant tag.
        ///
        /// Zero indicates this is an untagged union.  In standard borsh
        /// encoding this is one.  Custom encoding formats may use larger width
        /// if they need to encode more than 256 variants.  The schema is
        /// invalid if the value is greater than eight.
        tag_width: u8,

        /// Possible variants of the enumeration.
        /// `VariantName` is metadata, not present in a type's serialized representation.
        variants: Vec<(DiscriminantValue, VariantName, Declaration)>,
    },

    /// A structure, structurally similar to a tuple.
    Struct { fields: Fields },
}

impl Definition {
    /// Array length isn't present in payload, it's determined by type of data
    /// serialized.
    pub const ARRAY_LENGTH_WIDTH: u8 = 0;

    /// Convenience constant representing the length width of a standard borsh
    /// sequence.
    ///
    /// Can be used for `Definition::Sequence::length_width`.
    pub const DEFAULT_LENGTH_WIDTH: u8 = 4;

    /// Convenience constant representing the length range of a standard borsh
    /// sequence.
    ///
    /// It equals `0..=u32::MAX`.  Can be used with
    /// `Definition::Sequence::length_range`.
    pub const DEFAULT_LENGTH_RANGE: core::ops::RangeInclusive<u64> = 0..=(u32::MAX as u64);
}

/// The collection representing the fields of a struct.
#[derive(Clone, PartialEq, Eq, Debug, BorshSerialize, BorshDeserialize, BorshSchemaMacro)]
pub enum Fields {
    /// The struct with named fields, structurally identical to a tuple.
    /// `FieldName` is metadata, not present in a type's serialized representation.
    NamedFields(Vec<(FieldName, Declaration)>),
    /// The struct with unnamed fields, structurally identical to a tuple.
    UnnamedFields(Vec<Declaration>),
    /// The struct with no fields, structurally identical to an empty tuple.
    Empty,
}

/// All schema information needed to deserialize a single type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BorshSchemaContainer {
    /// Declaration of the type.
    declaration: Declaration,
    /// All definitions needed to deserialize the given type.
    definitions: BTreeMap<Declaration, Definition>,
}

impl BorshSchemaContainer {
    pub fn new(declaration: Declaration, definitions: BTreeMap<Declaration, Definition>) -> Self {
        Self {
            declaration,
            definitions,
        }
    }

    /// generate [BorshSchemaContainer] for type `T`
    pub fn for_type<T: BorshSchema + ?Sized>() -> Self {
        let mut definitions = Default::default();
        T::add_definitions_recursively(&mut definitions);
        Self::new(T::declaration(), definitions)
    }

    pub fn declaration(&self) -> &Declaration {
        &self.declaration
    }
    pub fn definitions(&self) -> impl Iterator<Item = (&'_ Declaration, &'_ Definition)> {
        self.definitions.iter()
    }

    pub fn get_definition<Q>(&self, declaration: &Q) -> Option<&Definition>
    where
        Declaration: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.definitions.get(declaration)
    }

    pub fn get_mut_definition<Q>(&mut self, declaration: &Q) -> Option<&mut Definition>
    where
        Declaration: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.definitions.get_mut(declaration)
    }

    pub fn insert_definition(
        &mut self,
        declaration: Declaration,
        definition: Definition,
    ) -> Option<Definition> {
        self.definitions.insert(declaration, definition)
    }
    pub fn remove_definition<Q>(&mut self, declaration: &Q) -> Option<Definition>
    where
        Declaration: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.definitions.remove(declaration)
    }
}

impl BorshSerialize for BorshSchemaContainer
where
    Declaration: BorshSerialize,
    BTreeMap<Declaration, Definition>: BorshSerialize,
{
    fn serialize<W: Write>(&self, writer: &mut W) -> IOResult<()> {
        let declaration = self.declaration();
        let definitions: BTreeMap<&Declaration, &Definition> = self.definitions().collect();
        BorshSerialize::serialize(declaration, writer)?;
        BorshSerialize::serialize(&definitions, writer)?;
        Ok(())
    }
}

impl BorshDeserialize for BorshSchemaContainer
where
    Declaration: BorshDeserialize,
    BTreeMap<Declaration, Definition>: BorshDeserialize,
{
    fn deserialize_reader<R: Read>(reader: &mut R) -> IOResult<Self> {
        let declaration: Declaration = BorshDeserialize::deserialize_reader(reader)?;
        let definitions: BTreeMap<Declaration, Definition> =
            BorshDeserialize::deserialize_reader(reader)?;
        Ok(Self::new(declaration, definitions))
    }
}

/// Helper method to add a single type definition to the map.
pub fn add_definition(
    declaration: Declaration,
    definition: Definition,
    definitions: &mut BTreeMap<Declaration, Definition>,
) {
    match definitions.entry(declaration) {
        Entry::Occupied(occ) => {
            let existing_def = occ.get();
            assert_eq!(
                existing_def,
                &definition,
                "Redefining type schema for {}. Types with the same names are not supported.",
                occ.key()
            );
        }
        Entry::Vacant(vac) => {
            vac.insert(definition);
        }
    }
}

/// The declaration and the definition of the type that can be used to (de)serialize Borsh without
/// the Rust type that produced it.
pub trait BorshSchema {
    /// Recursively, using DFS, add type definitions required for this type.
    /// Type definition partially explains how to serialize/deserialize a type.
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>);

    /// Get the name of the type without brackets.
    fn declaration() -> Declaration;
}

impl BorshSchema for BorshSchemaContainer
where
    Declaration: BorshSchema,
    BTreeMap<Declaration, Definition>: BorshSchema,
{
    fn declaration() -> Declaration {
        "BorshSchemaContainer".to_string()
    }
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let fields = Fields::NamedFields(<[_]>::into_vec(Box::new([
            (
                "declaration".to_string(),
                <Declaration as BorshSchema>::declaration(),
            ),
            (
                "definitions".to_string(),
                <BTreeMap<Declaration, Definition> as BorshSchema>::declaration(),
            ),
        ])));
        let definition = Definition::Struct { fields };
        add_definition(
            <Self as BorshSchema>::declaration(),
            definition,
            definitions,
        );
        <Declaration as BorshSchema>::add_definitions_recursively(definitions);
        <BTreeMap<Declaration, Definition> as BorshSchema>::add_definitions_recursively(
            definitions,
        );
    }
}
impl<T> BorshSchema for Box<T>
where
    T: BorshSchema + ?Sized,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        T::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        T::declaration()
    }
}
/// Module is available if borsh is built with `features = ["rc"]`.
#[cfg(feature = "rc")]
pub mod rc {
    //!
    //! Module defines [BorshSchema] implementation for
    //! [alloc::rc::Rc](std::rc::Rc) and [alloc::sync::Arc](std::sync::Arc).
    use crate::BorshSchema;

    use super::{Declaration, Definition};
    use crate::__private::maybestd::collections::BTreeMap;
    use crate::__private::maybestd::{rc::Rc, sync::Arc};

    impl<T> BorshSchema for Rc<T>
    where
        T: BorshSchema + ?Sized,
    {
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            T::add_definitions_recursively(definitions);
        }

        fn declaration() -> Declaration {
            T::declaration()
        }
    }

    impl<T> BorshSchema for Arc<T>
    where
        T: BorshSchema + ?Sized,
    {
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            T::add_definitions_recursively(definitions);
        }

        fn declaration() -> Declaration {
            T::declaration()
        }
    }
}

macro_rules! impl_for_renamed_primitives {
    ($($ty: ty : $name: ident => $size: expr);+) => {
    $(
        impl BorshSchema for $ty {
            #[inline]
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                let definition = Definition::Primitive($size);
                add_definition(Self::declaration(), definition, definitions);
            }
            #[inline]
            fn declaration() -> Declaration { stringify!($name).into() }
        }
    )+
    };

    ($($ty: ty : $name: expr, $size: expr);+) => {
    $(
        impl BorshSchema for $ty {
            #[inline]
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                let definition = Definition::Primitive($size);
                add_definition(Self::declaration(), definition, definitions);
            }
            #[inline]
            fn declaration() -> Declaration { $name.into() }
        }
    )+
    };
}

macro_rules! impl_for_primitives {
    ($($ty: ident => $size: expr);+) => {
        impl_for_renamed_primitives!{$($ty : $ty => $size);+}
    };
}

impl_for_primitives!(bool => 1; f32 => 4; f64 => 8; i8 => 1; i16 => 2; i32 => 4; i64 => 8; i128 => 16);
impl_for_primitives!(u8 => 1; u16 => 2; u32 => 4; u64 => 8; u128 => 16);
impl_for_renamed_primitives!(isize: i64 => 8);
impl_for_renamed_primitives!(usize: u64 => 8);

impl_for_renamed_primitives!(core::num::NonZeroI8: NonZeroI8 => 1);
impl_for_renamed_primitives!(core::num::NonZeroI16: NonZeroI16 => 2);
impl_for_renamed_primitives!(core::num::NonZeroI32: NonZeroI32 => 4);
impl_for_renamed_primitives!(core::num::NonZeroI64: NonZeroI64 => 8);
impl_for_renamed_primitives!(core::num::NonZeroI128: NonZeroI128 => 16);
impl_for_renamed_primitives!(core::num::NonZeroU8: NonZeroU8 => 1);
impl_for_renamed_primitives!(core::num::NonZeroU16: NonZeroU16 => 2);
impl_for_renamed_primitives!(core::num::NonZeroU32: NonZeroU32 => 4);
impl_for_renamed_primitives!(core::num::NonZeroU64: NonZeroU64 => 8);
impl_for_renamed_primitives!(core::num::NonZeroU128: NonZeroU128 => 16);
// see 12 lines above
impl_for_renamed_primitives!(core::num::NonZeroUsize: NonZeroUsize => 8);

impl_for_renamed_primitives!((): "()", 0);

impl BorshSchema for String {
    #[inline]
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        str::add_definitions_recursively(definitions);
    }
    #[inline]
    fn declaration() -> Declaration {
        str::declaration()
    }
}

impl BorshSchema for str {
    #[inline]
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let definition = Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: u8::declaration(),
        };
        add_definition(Self::declaration(), definition, definitions);
        u8::add_definitions_recursively(definitions);
    }
    #[inline]
    fn declaration() -> Declaration {
        "String".into()
    }
}

/// Module is available if borsh is built with `features = ["ascii"]`.
#[cfg(feature = "ascii")]
pub mod ascii {
    //!
    //! Module defines [BorshSchema] implementation for
    //! some types from [ascii](::ascii) crate.
    use crate::BorshSchema;

    use super::{add_definition, Declaration, Definition};
    use crate::__private::maybestd::collections::BTreeMap;

    impl BorshSchema for ascii::AsciiString {
        #[inline]
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            ascii::AsciiStr::add_definitions_recursively(definitions);
        }
        #[inline]
        fn declaration() -> Declaration {
            ascii::AsciiStr::declaration()
        }
    }

    impl BorshSchema for ascii::AsciiStr {
        #[inline]
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            let definition = Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: ascii::AsciiChar::declaration(),
            };
            add_definition(Self::declaration(), definition, definitions);
            ascii::AsciiChar::add_definitions_recursively(definitions);
        }
        #[inline]
        fn declaration() -> Declaration {
            "AsciiString".into()
        }
    }

    impl BorshSchema for ascii::AsciiChar {
        #[inline]
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            add_definition(Self::declaration(), Definition::Primitive(1), definitions);
        }
        #[inline]
        fn declaration() -> Declaration {
            "AsciiChar".into()
        }
    }
}

impl BorshSchema for core::ops::RangeFull {
    #[inline]
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let fields = Fields::Empty;
        let def = Definition::Struct { fields };
        add_definition(Self::declaration(), def, definitions);
    }
    #[inline]
    fn declaration() -> Declaration {
        "RangeFull".into()
    }
}

macro_rules! impl_for_range {
    ($type:ident, $($name:ident),*) => {
        impl<T: BorshSchema> BorshSchema for core::ops::$type<T> {
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                let decl = T::declaration();
                let fields = Fields::NamedFields(vec![$(
                    (FieldName::from(stringify!($name)), decl.clone())
                ),*]);
                let def = Definition::Struct { fields };
                add_definition(Self::declaration(), def, definitions);
                T::add_definitions_recursively(definitions);
            }
            fn declaration() -> Declaration {
                format!("{}<{}>", stringify!($type), T::declaration())
            }
        }
    };
}

impl_for_range!(Range, start, end);
impl_for_range!(RangeInclusive, start, end);
impl_for_range!(RangeFrom, start);
impl_for_range!(RangeTo, end);
impl_for_range!(RangeToInclusive, end);

impl<T, const N: usize> BorshSchema for [T; N]
where
    T: BorshSchema,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        use core::convert::TryFrom;
        let length = u64::try_from(N).unwrap();
        let definition = Definition::Sequence {
            length_width: Definition::ARRAY_LENGTH_WIDTH,
            length_range: length..=length,
            elements: T::declaration(),
        };
        add_definition(Self::declaration(), definition, definitions);
        T::add_definitions_recursively(definitions);
    }
    fn declaration() -> Declaration {
        format!(r#"[{}; {}]"#, T::declaration(), N)
    }
}

impl<T> BorshSchema for Option<T>
where
    T: BorshSchema,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let definition = Definition::Enum {
            tag_width: 1,
            variants: vec![
                (0u8 as i64, "None".to_string(), <()>::declaration()),
                (1u8 as i64, "Some".to_string(), T::declaration()),
            ],
        };
        add_definition(Self::declaration(), definition, definitions);
        T::add_definitions_recursively(definitions);
        <()>::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        format!(r#"Option<{}>"#, T::declaration())
    }
}

impl<T, E> BorshSchema for core::result::Result<T, E>
where
    T: BorshSchema,
    E: BorshSchema,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let definition = Definition::Enum {
            tag_width: 1,
            variants: vec![
                (1u8 as i64, "Ok".to_string(), T::declaration()),
                (0u8 as i64, "Err".to_string(), E::declaration()),
            ],
        };
        add_definition(Self::declaration(), definition, definitions);
        T::add_definitions_recursively(definitions);
        E::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        format!(r#"Result<{}, {}>"#, T::declaration(), E::declaration())
    }
}

impl<T> BorshSchema for Vec<T>
where
    T: BorshSchema,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let definition = Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: T::declaration(),
        };
        add_definition(Self::declaration(), definition, definitions);
        T::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        format!(r#"Vec<{}>"#, T::declaration())
    }
}

impl<T> BorshSchema for [T]
where
    T: BorshSchema,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let definition = Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: T::declaration(),
        };
        add_definition(Self::declaration(), definition, definitions);
        T::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        format!(r#"Vec<{}>"#, T::declaration())
    }
}

/// Module is available if borsh is built with `features = ["std"]` or `features = ["hashbrown"]`.
#[cfg(hash_collections)]
pub mod hashes {
    //!
    //! Module defines [BorshSchema] implementation for
    //! [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet).
    use crate::BorshSchema;

    use super::{add_definition, Declaration, Definition};
    use crate::__private::maybestd::collections::BTreeMap;

    use crate::__private::maybestd::collections::{HashMap, HashSet};
    #[cfg(not(feature = "std"))]
    use alloc::format;

    impl<K, V> BorshSchema for HashMap<K, V>
    where
        K: BorshSchema,
        V: BorshSchema,
    {
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            let definition = Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: <(K, V)>::declaration(),
            };
            add_definition(Self::declaration(), definition, definitions);
            <(K, V)>::add_definitions_recursively(definitions);
        }

        fn declaration() -> Declaration {
            format!(r#"HashMap<{}, {}>"#, K::declaration(), V::declaration())
        }
    }
    impl<T> BorshSchema for HashSet<T>
    where
        T: BorshSchema,
    {
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            let definition = Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: <T>::declaration(),
            };
            add_definition(Self::declaration(), definition, definitions);
            <T>::add_definitions_recursively(definitions);
        }

        fn declaration() -> Declaration {
            format!(r#"HashSet<{}>"#, T::declaration())
        }
    }
}

impl<K, V> BorshSchema for BTreeMap<K, V>
where
    K: BorshSchema,
    V: BorshSchema,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let definition = Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: <(K, V)>::declaration(),
        };
        add_definition(Self::declaration(), definition, definitions);
        <(K, V)>::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        format!(r#"BTreeMap<{}, {}>"#, K::declaration(), V::declaration())
    }
}

impl<T> BorshSchema for BTreeSet<T>
where
    T: BorshSchema,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let definition = Definition::Sequence {
            length_width: Definition::DEFAULT_LENGTH_WIDTH,
            length_range: Definition::DEFAULT_LENGTH_RANGE,
            elements: <T>::declaration(),
        };
        add_definition(Self::declaration(), definition, definitions);
        <T>::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        format!(r#"BTreeSet<{}>"#, T::declaration())
    }
}

// Because it's a zero-sized marker, its type parameter doesn't need to be
// included in the schema and so it's not bound to `BorshSchema`
impl<T> BorshSchema for PhantomData<T> {
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        <()>::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        <()>::declaration()
    }
}

macro_rules! impl_tuple {
    ($($name:ident),+) => {
    impl<$($name),+> BorshSchema for ($($name,)+)
    where
        $($name: BorshSchema),+
    {
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            let elements = vec![$($name::declaration()),+];

            let definition = Definition::Tuple { elements };
            add_definition(Self::declaration(), definition, definitions);
            $(
                $name::add_definitions_recursively(definitions);
            )+
        }

        fn declaration() -> Declaration {
            let params = vec![$($name::declaration()),+];
            if params.len() == 1 {
                format!(r#"({},)"#, params[0])
            } else {
                format!(r#"({})"#, params.join(", "))
            }
        }
    }
    };
}

impl_tuple!(T0);
impl_tuple!(T0, T1);
impl_tuple!(T0, T1, T2);
impl_tuple!(T0, T1, T2, T3);
impl_tuple!(T0, T1, T2, T3, T4);
impl_tuple!(T0, T1, T2, T3, T4, T5);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17);
impl_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18);
impl_tuple!(
    T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19
);
impl_tuple!(
    T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20
);

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(hash_collections)]
    use crate::__private::maybestd::collections::{HashMap, HashSet};

    macro_rules! map(
    () => { BTreeMap::new() };
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = BTreeMap::new();
            $(
                m.insert($key.to_string(), $value);
            )+
            m
        }
     };
    );

    #[test]
    fn simple_option() {
        let actual_name = Option::<u64>::declaration();
        let mut actual_defs = map!();
        Option::<u64>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("Option<u64>", actual_name);
        assert_eq!(
            map! {
                "Option<u64>" => Definition::Enum {
                    tag_width: 1,
                    variants: vec![
                        (0, "None".to_string(), "()".to_string()),
                        (1, "Some".to_string(), "u64".to_string()),
                    ]
                },
                "u64" => Definition::Primitive(8),
                "()" => Definition::Primitive(0)
            },
            actual_defs
        );
    }

    #[test]
    fn nested_option() {
        let actual_name = Option::<Option<u64>>::declaration();
        let mut actual_defs = map!();
        Option::<Option<u64>>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("Option<Option<u64>>", actual_name);
        assert_eq!(
            map! {
                "Option<u64>" => Definition::Enum {
                    tag_width: 1,
                    variants: vec![
                        (0, "None".to_string(), "()".to_string()),
                        (1, "Some".to_string(), "u64".to_string()),
                    ]
                },
                "Option<Option<u64>>" => Definition::Enum {
                    tag_width: 1,
                    variants: vec![
                        (0, "None".to_string(), "()".to_string()),
                        (1, "Some".to_string(), "Option<u64>".to_string()),
                    ]
                },
                "u64" => Definition::Primitive(8),
                "()" => Definition::Primitive(0)
            },
            actual_defs
        );
    }

    #[test]
    fn simple_vec() {
        let actual_name = Vec::<u64>::declaration();
        let mut actual_defs = map!();
        Vec::<u64>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("Vec<u64>", actual_name);
        assert_eq!(
            map! {
                "Vec<u64>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u64".to_string(),
                },
                "u64" => Definition::Primitive(8)
            },
            actual_defs
        );
    }

    #[test]
    fn nested_vec() {
        let actual_name = Vec::<Vec<u64>>::declaration();
        let mut actual_defs = map!();
        Vec::<Vec<u64>>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("Vec<Vec<u64>>", actual_name);
        assert_eq!(
            map! {
                "Vec<u64>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u64".to_string(),
                },
                "Vec<Vec<u64>>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "Vec<u64>".to_string(),
                },
                "u64" => Definition::Primitive(8)
            },
            actual_defs
        );
    }

    #[test]
    fn simple_tuple() {
        let actual_name = <(u64, core::num::NonZeroU16, String)>::declaration();
        let mut actual_defs = map!();
        <(u64, core::num::NonZeroU16, String)>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("(u64, NonZeroU16, String)", actual_name);
        assert_eq!(
            map! {
                "(u64, NonZeroU16, String)" => Definition::Tuple {
                    elements: vec![
                        "u64".to_string(),
                        "NonZeroU16".to_string(),
                        "String".to_string()
                    ]
                },
                "u64" => Definition::Primitive(8),
                "NonZeroU16" => Definition::Primitive(2),
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );
    }

    #[test]
    fn nested_tuple() {
        let actual_name = <(u64, (u8, bool), String)>::declaration();
        let mut actual_defs = map!();
        <(u64, (u8, bool), String)>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("(u64, (u8, bool), String)", actual_name);
        assert_eq!(
            map! {
                "(u64, (u8, bool), String)" => Definition::Tuple { elements: vec![
                    "u64".to_string(),
                    "(u8, bool)".to_string(),
                    "String".to_string(),
                ]},
                "(u8, bool)" => Definition::Tuple { elements: vec![ "u8".to_string(), "bool".to_string()]},
                "u64" => Definition::Primitive(8),
                "u8" => Definition::Primitive(1),
                "bool" => Definition::Primitive(1),
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                }
            },
            actual_defs
        );
    }

    #[cfg(hash_collections)]
    #[test]
    fn simple_map() {
        let actual_name = HashMap::<u64, String>::declaration();
        let mut actual_defs = map!();
        HashMap::<u64, String>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("HashMap<u64, String>", actual_name);
        assert_eq!(
            map! {
                "HashMap<u64, String>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "(u64, String)".to_string(),
                } ,
                "(u64, String)" => Definition::Tuple {
                    elements: vec![ "u64".to_string(), "String".to_string()],
                },
                "u64" => Definition::Primitive(8),
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                },
                "u8" => Definition::Primitive(1)

            },
            actual_defs
        );
    }

    #[cfg(hash_collections)]
    #[test]
    fn simple_set() {
        let actual_name = HashSet::<String>::declaration();
        let mut actual_defs = map!();
        HashSet::<String>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("HashSet<String>", actual_name);
        assert_eq!(
            map! {
                "HashSet<String>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "String".to_string(),
                },
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );
    }

    #[test]
    fn b_tree_map() {
        let actual_name = BTreeMap::<u64, String>::declaration();
        let mut actual_defs = map!();
        BTreeMap::<u64, String>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("BTreeMap<u64, String>", actual_name);
        assert_eq!(
            map! {
                "BTreeMap<u64, String>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "(u64, String)".to_string(),
                } ,
                "(u64, String)" => Definition::Tuple { elements: vec![ "u64".to_string(), "String".to_string()]},
                "u64" => Definition::Primitive(8),
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );
    }

    #[test]
    fn b_tree_set() {
        let actual_name = BTreeSet::<String>::declaration();
        let mut actual_defs = map!();
        BTreeSet::<String>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("BTreeSet<String>", actual_name);
        assert_eq!(
            map! {
                "BTreeSet<String>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "String".to_string(),
                },
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );
    }

    #[test]
    fn simple_array() {
        let actual_name = <[u64; 32]>::declaration();
        let mut actual_defs = map!();
        <[u64; 32]>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("[u64; 32]", actual_name);
        assert_eq!(
            map! {
                "[u64; 32]" => Definition::Sequence {
                    length_width: Definition::ARRAY_LENGTH_WIDTH,
                    length_range: 32..=32,
                    elements: "u64".to_string()
                },
                "u64" => Definition::Primitive(8)
            },
            actual_defs
        );
    }

    #[test]
    fn nested_array() {
        let actual_name = <[[[u64; 9]; 10]; 32]>::declaration();
        let mut actual_defs = map!();
        <[[[u64; 9]; 10]; 32]>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("[[[u64; 9]; 10]; 32]", actual_name);
        assert_eq!(
            map! {
                "[u64; 9]" => Definition::Sequence {
                    length_width: Definition::ARRAY_LENGTH_WIDTH,
                    length_range: 9..=9,
                    elements: "u64".to_string()
                },
                "[[u64; 9]; 10]" => Definition::Sequence {
                    length_width: Definition::ARRAY_LENGTH_WIDTH,
                    length_range: 10..=10,
                    elements: "[u64; 9]".to_string()
                },
                "[[[u64; 9]; 10]; 32]" => Definition::Sequence {
                    length_width: Definition::ARRAY_LENGTH_WIDTH,
                    length_range: 32..=32,
                    elements: "[[u64; 9]; 10]".to_string()
                },
                "u64" => Definition::Primitive(8)
            },
            actual_defs
        );
    }

    #[test]
    fn string() {
        let actual_name = str::declaration();
        assert_eq!("String", actual_name);
        let actual_name = String::declaration();
        assert_eq!("String", actual_name);
        let mut actual_defs = map!();
        String::add_definitions_recursively(&mut actual_defs);
        assert_eq!(
            map! {
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );

        let mut actual_defs = map!();
        str::add_definitions_recursively(&mut actual_defs);
        assert_eq!(
            map! {
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                 },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );
    }

    #[test]
    fn boxed_schema() {
        let boxed_declaration = Box::<str>::declaration();
        assert_eq!("String", boxed_declaration);
        let boxed_declaration = Box::<[u8]>::declaration();
        assert_eq!("Vec<u8>", boxed_declaration);
    }

    #[test]
    fn phantom_data_schema() {
        let phantom_declaration = PhantomData::<String>::declaration();
        assert_eq!("()", phantom_declaration);
        let phantom_declaration = PhantomData::<Vec<u8>>::declaration();
        assert_eq!("()", phantom_declaration);
    }

    #[test]
    fn range() {
        assert_eq!("RangeFull", <core::ops::RangeFull>::declaration());
        let mut actual_defs = map!();
        <core::ops::RangeFull>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(
            map! {
                "RangeFull" => Definition::Struct {
                    fields: Fields::Empty
                }
            },
            actual_defs
        );

        let actual_name = <core::ops::Range<u64>>::declaration();
        let mut actual_defs = map!();
        <core::ops::Range<u64>>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("Range<u64>", actual_name);
        assert_eq!(
            map! {
                "Range<u64>" => Definition::Struct {
                    fields: Fields::NamedFields(vec![
                        ("start".into(), "u64".into()),
                        ("end".into(), "u64".into()),
                    ])
                },
                "u64" => Definition::Primitive(8)
            },
            actual_defs
        );

        let actual_name = <core::ops::RangeTo<u64>>::declaration();
        let mut actual_defs = map!();
        <core::ops::RangeTo<u64>>::add_definitions_recursively(&mut actual_defs);
        assert_eq!("RangeTo<u64>", actual_name);
        assert_eq!(
            map! {
                "RangeTo<u64>" => Definition::Struct {
                    fields: Fields::NamedFields(vec![
                        ("end".into(), "u64".into()),
                    ])
                },
                "u64" => Definition::Primitive(8)
            },
            actual_defs
        );
    }
}
