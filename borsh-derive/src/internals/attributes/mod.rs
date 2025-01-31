use syn::{Attribute, Path};

pub mod field;
pub mod item;
pub mod parsing;

/// first field is attr name
/// second field is its expected value format representation for error printing
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Symbol {
    pub name: &'static str,
    pub expected: &'static str,
    support: AsyncSupport,
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
enum AsyncSupport {
    Sync,
    Async,
    Both,
}

impl Symbol {
    pub const fn new(name: &'static str, expected: &'static str) -> Self {
        Self {
            name,
            expected,
            support: AsyncSupport::Both,
        }
    }

    pub const fn new_sync(name: &'static str, expected: &'static str) -> Self {
        Self {
            name,
            expected,
            support: AsyncSupport::Sync,
        }
    }

    pub const fn new_async(name: &'static str, expected: &'static str) -> Self {
        Self {
            name,
            expected,
            support: AsyncSupport::Async,
        }
    }

    pub const fn test_support<const IS_ASYNC: bool>(&self) -> bool {
        if IS_ASYNC {
            matches!(self.support, AsyncSupport::Async | AsyncSupport::Both)
        } else {
            matches!(self.support, AsyncSupport::Sync | AsyncSupport::Both)
        }
    }
}

/// `borsh` - top level prefix in nested meta attribute
pub const BORSH: Symbol = Symbol::new("borsh", "borsh(...)");
/// `bound` - sub-borsh nested meta, field-level only; `BorshSerialize` and `BorshDeserialize` contexts
pub const BOUND: Symbol = Symbol::new("bound", "bound(...)");
/// `async_bound` - sub-borsh nested meta, field-level only; `BorshSerializeAsync` and `BorshDeserializeAsync` contexts
pub const ASYNC_BOUND: Symbol = Symbol::new_async("async_bound", "async_bound(...)");
///  `use_discriminant` - sub-borsh nested meta, item-level only, enums only;
/// `BorshSerialize`, `BorshDeserialize`, `BorshSerializeAsync` and `BorshDeserializeAsync` contexts
pub const USE_DISCRIMINANT: Symbol = Symbol::new("use_discriminant", "use_discriminant = ...");
/// `serialize` - sub-bound nested meta attribute
pub const SERIALIZE: Symbol = Symbol::new("serialize", "serialize = ...");
/// `deserialize` - sub-bound nested meta attribute
pub const DESERIALIZE: Symbol = Symbol::new("deserialize", "deserialize = ...");
/// `skip` - sub-borsh nested meta, field-level only attribute;
/// `BorshSerialize`, `BorshDeserialize`, `BorshSerializeAsync`, `BorshDeserializeAsync` and `BorshSchema` contexts
pub const SKIP: Symbol = Symbol::new("skip", "skip");
/// `init` - sub-borsh nested meta, item-level only attribute; `BorshDeserialize` and `BorshDeserializeAsync` contexts
pub const INIT: Symbol = Symbol::new("init", "init = ...");
/// `serialize_with` - sub-borsh nested meta, field-level only; `BorshSerialize` context
pub const SERIALIZE_WITH: Symbol = Symbol::new_sync("serialize_with", "serialize_with = ...");
/// `deserialize_with` - sub-borsh nested meta, field-level only; `BorshDeserialize` context
pub const DESERIALIZE_WITH: Symbol = Symbol::new_sync("deserialize_with", "deserialize_with = ...");
/// `serialize_with_async` - sub-borsh nested meta, field-level only; `BorshSerializeAsync` context
pub const SERIALIZE_WITH_ASYNC: Symbol =
    Symbol::new_async("serialize_with_async", "serialize_with_async = ...");
/// `deserialize_with_async` - sub-borsh nested meta, field-level only; `BorshDeserializeAsync` context
pub const DESERIALIZE_WITH_ASYNC: Symbol =
    Symbol::new_async("deserialize_with_async", "deserialize_with_async = ...");
/// `crate` - sub-borsh nested meta, item-level only;
/// `BorshSerialize`, `BorshDeserialize`, `BorshSerializeAsync`, `BorshDeserializeAsync` and `BorshSchema` contexts
pub const CRATE: Symbol = Symbol::new("crate", "crate = ...");

#[cfg(feature = "schema")]
pub mod schema_keys {
    use super::Symbol;

    /// `schema` - sub-borsh nested meta, `BorshSchema` context
    pub const SCHEMA: Symbol = Symbol::new("schema", "schema(...)");
    /// `params` - sub-schema nested meta, field-level only attribute
    pub const PARAMS: Symbol = Symbol::new("params", "params = ...");
    /// `serialize_with` - sub-borsh nested meta, field-level only, `BorshSerialize` context
    /// `with_funcs` - sub-schema nested meta, field-level only attribute
    pub const WITH_FUNCS: Symbol = Symbol::new("with_funcs", "with_funcs(...)");
    /// `declaration` - sub-with_funcs nested meta, field-level only attribute
    pub const DECLARATION: Symbol = Symbol::new("declaration", "declaration = ...");
    /// `definitions` - sub-with_funcs nested meta, field-level only attribute
    pub const DEFINITIONS: Symbol = Symbol::new("definitions", "definitions = ...");
}

#[derive(Clone, Copy)]
pub enum BoundType {
    Serialize,
    Deserialize,
}
impl PartialEq<Symbol> for Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.name)
    }
}

impl<'a> PartialEq<Symbol> for &'a Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.name)
    }
}

fn get_one_attribute(attrs: &[Attribute]) -> syn::Result<Option<&Attribute>> {
    let mut attrs = attrs.iter().filter(|attr| attr.path() == BORSH);
    let borsh = attrs.next();
    if let Some(other_borsh) = attrs.next() {
        return Err(syn::Error::new_spanned(
            other_borsh,
            format!("multiple `{}` attributes not allowed", BORSH.name),
        ));
    }
    Ok(borsh)
}
