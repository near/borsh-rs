//! Schemas: a type's declaration (its name) and its definitions (the shapes of it and every
//! type it contains), keyed by declaration. Walks over a container live in `walks`.
use crate as borsh; // The derives below name the crate `borsh`.
use crate::__private::maybestd::{
    borrow,
    boxed::Box,
    collections::{btree_map::Entry, BTreeMap, BTreeSet, LinkedList, VecDeque},
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use crate::io::{Read, Result as IOResult, Write};
use crate::{BorshDeserialize, BorshSchema as BorshSchemaMacro, BorshSerialize};
use core::borrow::Borrow;
use core::marker::PhantomData;

mod walks;
pub use walks::{SchemaContainerValidateError, SchemaMaxSerializedSizeError};

/// The type name, for primitives and std types. Derived types use their name and the
/// declarations of their generic parameters.
pub type Declaration = String;
pub type VariantName = String;
/// The tag of a variant.
pub type DiscriminantValue = i64;
pub type FieldName = String;

/// The shape of a type.
#[derive(Clone, PartialEq, Eq, Debug, BorshSerialize, BorshDeserialize, BorshSchemaMacro)]
pub enum Definition {
    /// A fixed-size value of the given number of bytes.
    Primitive(u8),
    /// A length (`length_width` bytes, or none for fixed-length arrays) and that many elements.
    Sequence {
        length_width: u8,
        length_range: core::ops::RangeInclusive<u64>,
        elements: Declaration,
    },
    /// Each element in order.
    Tuple { elements: Vec<Declaration> },
    /// A tag of `tag_width` bytes, then the fields of the variant with that discriminant.
    Enum {
        tag_width: u8,
        variants: Vec<(DiscriminantValue, VariantName, Declaration)>,
    },
    /// Each field in order.
    Struct { fields: Fields },
}

impl Definition {
    /// The `length_width` of fixed-length arrays: no length is written.
    pub const ARRAY_LENGTH_WIDTH: u8 = 0;
    /// The `length_width` of `Vec` and other collections: a `u32`.
    pub const DEFAULT_LENGTH_WIDTH: u8 = 4;
    /// The `length_range` of `Vec` and other collections.
    pub const DEFAULT_LENGTH_RANGE: core::ops::RangeInclusive<u64> = 0..=(u32::MAX as u64);
}

/// The fields of a struct or enum variant.
#[derive(Clone, PartialEq, Eq, Debug, BorshSerialize, BorshDeserialize, BorshSchemaMacro)]
pub enum Fields {
    NamedFields(Vec<(FieldName, Declaration)>),
    UnnamedFields(Vec<Declaration>),
    Empty,
}

/// A type's declaration and every definition it needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BorshSchemaContainer {
    declaration: Declaration,
    definitions: BTreeMap<Declaration, Definition>,
}

impl BorshSchemaContainer {
    pub fn new(declaration: Declaration, definitions: BTreeMap<Declaration, Definition>) -> Self {
        Self {
            declaration,
            definitions,
        }
    }

    /// The schema of `T`.
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

    pub fn get_definition<Q: Ord + ?Sized>(&self, declaration: &Q) -> Option<&Definition>
    where
        Declaration: Borrow<Q>,
    {
        self.definitions.get(declaration)
    }

    pub fn get_mut_definition<Q: Ord + ?Sized>(
        &mut self,
        declaration: &Q,
    ) -> Option<&mut Definition>
    where
        Declaration: Borrow<Q>,
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

    pub fn remove_definition<Q: Ord + ?Sized>(&mut self, declaration: &Q) -> Option<Definition>
    where
        Declaration: Borrow<Q>,
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
        self.declaration.serialize(writer)?;
        self.definitions.serialize(writer)
    }
}

impl BorshDeserialize for BorshSchemaContainer
where
    Declaration: BorshDeserialize,
    BTreeMap<Declaration, Definition>: BorshDeserialize,
{
    fn deserialize_reader<R: Read>(reader: &mut R) -> IOResult<Self> {
        let declaration = BorshDeserialize::deserialize_reader(reader)?;
        Ok(Self::new(
            declaration,
            BorshDeserialize::deserialize_reader(reader)?,
        ))
    }
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
        let fields = Fields::NamedFields(vec![
            ("declaration".to_string(), Declaration::declaration()),
            (
                "definitions".to_string(),
                <BTreeMap<Declaration, Definition>>::declaration(),
            ),
        ]);
        add_definition(
            <Self as BorshSchema>::declaration(),
            Definition::Struct { fields },
            definitions,
        );
        Declaration::add_definitions_recursively(definitions);
        <BTreeMap<Declaration, Definition>>::add_definitions_recursively(definitions);
    }
}

/// Add a definition. Adding a different definition under the same declaration panics: two
/// types with the same name can't share a schema.
pub fn add_definition(
    declaration: Declaration,
    definition: Definition,
    definitions: &mut BTreeMap<Declaration, Definition>,
) {
    match definitions.entry(declaration) {
        Entry::Occupied(occ) => assert_eq!(
            occ.get(),
            &definition,
            "Redefining type schema for {}. Types with the same names are not supported.",
            occ.key()
        ),
        Entry::Vacant(vac) => {
            vac.insert(definition);
        }
    }
}

/// For derived types: add `T`'s definition, then its fields' definitions unless this walk
/// already added them, so recursive types terminate.
///
/// Declarations alone can't end the walk: a cycle back into `T` and another type with the
/// same name, such as `Signed<v2::Transfer>` after `Signed<v1::Transfer>`, look the same.
/// Neither can [`core::any::type_name`] alone: it isn't guaranteed unique, and items in
/// different blocks of one function print the same path. So the walk is cut on the pair of
/// both, which a cycle repeats. Two same-named types in sibling blocks with the same
/// declaration still share a pair, so the second one's fields aren't walked, as on master.
///
/// The trait passes only `definitions`, so the pairs walked so far are kept there under the
/// reserved key `"\0walked"`, which no Rust type name or derived declaration contains. The
/// outermost call removes it on return and on panic, so only a hand-written
/// `add_definitions_recursively` called during a walk can see it.
#[doc(hidden)]
pub fn add_struct_definition<T: BorshSchema + ?Sized>(
    definition: Definition,
    definitions: &mut BTreeMap<Declaration, Definition>,
    add_fields_definitions: impl FnOnce(&mut BTreeMap<Declaration, Definition>),
) {
    const WALKED: &str = "\0walked";
    /// Removes the walked-types key when the outermost call ends.
    struct Walk<'a>(&'a mut BTreeMap<Declaration, Definition>, bool);
    impl Drop for Walk<'_> {
        fn drop(&mut self) {
            if self.1 {
                self.0.remove(WALKED);
            }
        }
    }
    add_definition(T::declaration(), definition, definitions);
    let outermost = !definitions.contains_key(WALKED);
    let walk = Walk(definitions, outermost);
    let walked = walk.0.entry(WALKED.into()).or_insert(Definition::Tuple {
        elements: Vec::new(),
    });
    let key = format!("{}\0{}", core::any::type_name::<T>(), T::declaration());
    if let Definition::Tuple { elements } = walked {
        if !elements.contains(&key) {
            elements.push(key);
            add_fields_definitions(&mut *walk.0);
        }
    }
}

/// The schema of a type: its declaration, and its definitions and those of every type it
/// contains.
pub trait BorshSchema {
    /// Add this type's definitions, and those of the types it contains, to `definitions`.
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>);

    /// The name of this type.
    fn declaration() -> Declaration;
}

/// Free functions over schemas, re-exported at the crate root.
pub(crate) mod helpers {
    use super::{BorshSchema, BorshSchemaContainer, SchemaMaxSerializedSizeError};
    use crate::__private::maybestd::vec::Vec;
    use crate::io::{Error, ErrorKind, Result as IOResult};
    use crate::{from_slice, BorshDeserialize, BorshSerialize};

    /// Decode `T` after its schema, rejecting a schema other than `T`'s.
    pub fn try_from_slice_with_schema<T: BorshDeserialize + BorshSchema>(v: &[u8]) -> IOResult<T> {
        let (schema, object) = from_slice::<(BorshSchemaContainer, T)>(v)?;
        if schema_container_of::<T>() != schema {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Borsh schema does not match",
            ));
        }
        Ok(object)
    }

    /// Encode `value` after the schema of `T`.
    pub fn try_to_vec_with_schema<T: BorshSerialize + BorshSchema + ?Sized>(
        value: &T,
    ) -> IOResult<Vec<u8>> {
        let mut res = crate::to_vec(&schema_container_of::<T>())?;
        value.serialize(&mut res)?;
        Ok(res)
    }

    /// The schema of `T`, as [BorshSchemaContainer::for_type].
    pub fn schema_container_of<T: BorshSchema + ?Sized>() -> BorshSchemaContainer {
        BorshSchemaContainer::for_type::<T>()
    }

    /// Returns the largest possible size of a serialised object based solely on its type `T`.
    ///
    /// this is a shortcut for using [BorshSchemaContainer::max_serialized_size]
    /// # Example
    ///
    /// ```
    /// use borsh::schema::BorshSchemaContainer;
    ///
    /// assert_eq!(Ok(8), borsh::max_serialized_size::<usize>());
    /// ```
    pub fn max_serialized_size<T: BorshSchema + ?Sized>(
    ) -> core::result::Result<usize, SchemaMaxSerializedSizeError> {
        BorshSchemaContainer::for_type::<T>().max_serialized_size()
    }
}

/// The schema of a type that encodes as `T` does.
macro_rules! same_as {
    ($($(#[$cfg:meta])* [$($g:tt)*] $t:ty => $inner:ty;)*) => {$(
        $(#[$cfg])*
        impl<$($g)*> BorshSchema for $t {
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                <$inner>::add_definitions_recursively(definitions);
            }
            fn declaration() -> Declaration {
                <$inner>::declaration()
            }
        }
    )*};
}
same_as! {
    [T: BorshSchema + ?Sized] Box<T> => T;
    [T: BorshSchema + Copy] core::cell::Cell<T> => T;
    [T: BorshSchema] core::cell::RefCell<T> => T;
    [] String => str;
    [T] PhantomData<T> => ();
}
impl<T: borrow::ToOwned + ?Sized> BorshSchema for borrow::Cow<'_, T>
where
    T::Owned: BorshSchema,
{
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        T::Owned::add_definitions_recursively(definitions);
    }
    fn declaration() -> Declaration {
        T::Owned::declaration()
    }
}

/// Module is available if borsh is built with `features = ["rc"]`.
#[cfg(feature = "rc")]
pub mod rc {
    use super::*;
    use crate::__private::maybestd::{rc::Rc, sync::Arc};
    same_as! {
        [T: BorshSchema + ?Sized] Rc<T> => T;
        [T: BorshSchema + ?Sized] Arc<T> => T;
    }
}

macro_rules! primitive {
    ($($t:ty: $name:expr => $size:expr;)*) => {$(
        impl BorshSchema for $t {
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                add_definition(Self::declaration(), Definition::Primitive($size), definitions);
            }
            fn declaration() -> Declaration {
                $name.into()
            }
        }
    )*};
}
primitive! {
    bool: "bool" => 1; f32: "f32" => 4; f64: "f64" => 8;
    i8: "i8" => 1; i16: "i16" => 2; i32: "i32" => 4; i64: "i64" => 8; i128: "i128" => 16;
    u8: "u8" => 1; u16: "u16" => 2; u32: "u32" => 4; u64: "u64" => 8; u128: "u128" => 16;
    isize: "i64" => 8; usize: "u64" => 8;
    core::num::NonZeroI8: "NonZeroI8" => 1; core::num::NonZeroI16: "NonZeroI16" => 2;
    core::num::NonZeroI32: "NonZeroI32" => 4; core::num::NonZeroI64: "NonZeroI64" => 8;
    core::num::NonZeroI128: "NonZeroI128" => 16; core::num::NonZeroU8: "NonZeroU8" => 1;
    core::num::NonZeroU16: "NonZeroU16" => 2; core::num::NonZeroU32: "NonZeroU32" => 4;
    core::num::NonZeroU64: "NonZeroU64" => 8; core::num::NonZeroU128: "NonZeroU128" => 16;
    core::num::NonZeroUsize: "NonZeroUsize" => 8;
    (): "()" => 0;
}

/// A `u32` length and that many elements.
fn sequence(elements: Declaration) -> Definition {
    Definition::Sequence {
        length_width: Definition::DEFAULT_LENGTH_WIDTH,
        length_range: Definition::DEFAULT_LENGTH_RANGE,
        elements,
    }
}

/// Collections: a sequence of `$elem`, declared as `$decl`.
macro_rules! seq {
    ($($(#[$cfg:meta])* [$($g:tt)*] $t:ty => $elem:ty, $decl:expr;)*) => {$(
        $(#[$cfg])*
        impl<$($g)*> BorshSchema for $t {
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                add_definition(Self::declaration(), sequence(<$elem>::declaration()), definitions);
                <$elem>::add_definitions_recursively(definitions);
            }
            fn declaration() -> Declaration {
                $decl
            }
        }
    )*};
}
seq! {
    [] str => u8, "String".into();
    [T: BorshSchema] Vec<T> => T, format!("Vec<{}>", T::declaration());
    [T: BorshSchema] VecDeque<T> => T, format!("VecDeque<{}>", T::declaration());
    [T: BorshSchema] LinkedList<T> => T, format!("LinkedList<{}>", T::declaration());
    [T: BorshSchema] [T] => T, format!("Vec<{}>", T::declaration());
    [T: BorshSchema] BTreeSet<T> => T, format!("BTreeSet<{}>", T::declaration());
    [K: BorshSchema, V: BorshSchema] BTreeMap<K, V> => (K, V),
        format!("BTreeMap<{}, {}>", K::declaration(), V::declaration());
}

/// Module is available if borsh is built with `features = ["std"]` or `features = ["hashbrown"]`.
#[cfg(any(feature = "std", feature = "hashbrown"))]
pub mod hashes {
    use super::*;
    use crate::__private::maybestd::collections::{HashMap, HashSet};

    seq! {
        [K: BorshSchema, V: BorshSchema, S] HashMap<K, V, S> => (K, V),
            format!("HashMap<{}, {}>", K::declaration(), V::declaration());
        [T: BorshSchema, S] HashSet<T, S> => T, format!("HashSet<{}>", T::declaration());
    }
}

/// Module is available if borsh is built with `features = ["ascii"]`.
#[cfg(feature = "ascii")]
pub mod ascii {
    use super::*;

    same_as! {
        [] ::ascii::AsciiString => ::ascii::AsciiStr;
    }
    seq! {
        [] ::ascii::AsciiStr => ::ascii::AsciiChar, "AsciiString".into();
    }

    impl BorshSchema for ::ascii::AsciiChar {
        fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
            add_definition(Self::declaration(), Definition::Primitive(1), definitions);
        }
        fn declaration() -> Declaration {
            "AsciiChar".into()
        }
    }
}

/// Module is available if borsh is built with `features = ["bytes"]`.
#[cfg(feature = "bytes")]
pub mod bytes {
    use super::*;

    seq! {
        [] ::bytes::Bytes => u8, "bytes::Bytes".into();
        [] ::bytes::BytesMut => u8, "bytes::BytesMut".into();
    }
}

impl BorshSchema for core::ops::RangeFull {
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        add_definition(
            Self::declaration(),
            Definition::Struct {
                fields: Fields::Empty,
            },
            definitions,
        );
    }
    fn declaration() -> Declaration {
        "RangeFull".into()
    }
}

macro_rules! range {
    ($($t:ident($($side:ident),*);)*) => {$(
        impl<T: BorshSchema> BorshSchema for core::ops::$t<T> {
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                let fields = vec![$((stringify!($side).into(), T::declaration())),*];
                let fields = Fields::NamedFields(fields);
                add_definition(Self::declaration(), Definition::Struct { fields }, definitions);
                T::add_definitions_recursively(definitions);
            }
            fn declaration() -> Declaration {
                format!(concat!(stringify!($t), "<{}>"), T::declaration())
            }
        }
    )*};
}
range! {
    Range(start, end);
    RangeInclusive(start, end);
    RangeFrom(start);
    RangeTo(end);
    RangeToInclusive(end);
}

impl<T: BorshSchema, const N: usize> BorshSchema for [T; N] {
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let length = N as u64;
        let definition = Definition::Sequence {
            length_width: Definition::ARRAY_LENGTH_WIDTH,
            length_range: length..=length,
            elements: T::declaration(),
        };
        add_definition(Self::declaration(), definition, definitions);
        T::add_definitions_recursively(definitions);
    }
    fn declaration() -> Declaration {
        format!("[{}; {}]", T::declaration(), N)
    }
}

/// `Option` and `Result` are enums with a one-byte tag.
macro_rules! enum_like {
    ($($t:ty, [$($g:ident),*], $fmt:literal, $(($tag:literal, $var:literal, $decl:ty)),*;)*) => {$(
        impl<$($g: BorshSchema),*> BorshSchema for $t {
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                let variants = vec![$(($tag, $var.to_string(), <$decl>::declaration())),*];
                let definition = Definition::Enum { tag_width: 1, variants };
                add_definition(Self::declaration(), definition, definitions);
                $(<$decl>::add_definitions_recursively(definitions);)*
            }
            fn declaration() -> Declaration {
                format!($fmt, $($g::declaration()),*)
            }
        }
    )*};
}
enum_like! {
    Option<T>, [T], "Option<{}>", (0, "None", ()), (1, "Some", T);
    core::result::Result<T, E>, [T, E], "Result<{}, {}>", (1, "Ok", T), (0, "Err", E);
}

macro_rules! tuple {
    ($($idx:tt $name:ident)+) => {
        impl<$($name: BorshSchema),+> BorshSchema for ($($name,)+) {
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                let elements = vec![$($name::declaration()),+];
                add_definition(Self::declaration(), Definition::Tuple { elements }, definitions);
                $($name::add_definitions_recursively(definitions);)+
            }
            fn declaration() -> Declaration {
                let params: Vec<Declaration> = vec![$($name::declaration()),+];
                if params.len() == 1 {
                    format!("({},)", params[0])
                } else {
                    format!("({})", params.join(", "))
                }
            }
        }
    };
}
tuples!(tuple []
    0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9
    10 T10 11 T11 12 T12 13 T13 14 T14 15 T15 16 T16 17 T17 18 T18 19 T19 20 T20
);

/// IP addresses have the schemas of these types.
#[cfg(feature = "std")]
#[allow(dead_code)]
mod ip {
    use crate::BorshSchema;

    #[derive(BorshSchema)]
    #[borsh(crate = "crate")]
    pub struct Ipv4Addr {
        octets: [u8; 4],
    }

    #[derive(BorshSchema)]
    #[borsh(crate = "crate")]
    pub struct Ipv6Addr {
        octets: [u8; 16],
    }

    #[derive(BorshSchema)]
    #[borsh(crate = "crate")]
    pub enum IpAddr {
        V4(core::net::Ipv4Addr),
        V6(core::net::Ipv6Addr),
    }
}
#[cfg(feature = "std")]
same_as! {
    [] core::net::Ipv4Addr => ip::Ipv4Addr;
    [] core::net::Ipv6Addr => ip::Ipv6Addr;
    [] core::net::IpAddr => ip::IpAddr;
}
