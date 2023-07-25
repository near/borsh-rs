use syn::{Attribute, Path};

pub mod field;
pub mod parsing;

/// firsh fields is attr name
/// seconds field is its expected value format representation for error printing
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Symbol(pub &'static str, pub &'static str);

/// borsh - top level prefix in nested meta attribute
pub const BORSH: Symbol = Symbol("borsh", "borsh(...)");
/// bound - sub-borsh nested meta, field-level only, `BorshSerialize` and `BorshDeserialize` contexts
pub const BOUND: Symbol = Symbol("bound", "bound(...)");
/// serialize - sub-bound nested meta attribute
pub const SERIALIZE: Symbol = Symbol("serialize", "serialize = ...");
/// deserialize - sub-bound nested meta attribute
pub const DESERIALIZE: Symbol = Symbol("deserialize", "deserialize = ...");
/// borsh_skip - field-level only attribute, `BorshSerialize`, `BorshDeserialize`, `BorshSchema` contexts
pub const SKIP: Symbol = Symbol("borsh_skip", "");
/// borsh_init - item-level only attribute  `BorshDeserialize` context
pub const INIT: Symbol = Symbol("borsh_init", "borsh_init(...)");
/// schema - sub-borsh nested meta, `BorshSchema` context
pub const SCHEMA: Symbol = Symbol("schema", "schema(...)");
/// params - sub-schema nested meta, field-level only attribute
pub const PARAMS: Symbol = Symbol("params", "params = ...");
/// serialize_with - sub-borsh nested meta, field-level only, `BorshSerialize` context
pub const SERIALIZE_WITH: Symbol = Symbol("serialize_with", "serialize_with = ...");
/// deserialize_with - sub-borsh nested meta, field-level only, `BorshDeserialize` context
pub const DESERIALIZE_WITH: Symbol = Symbol("deserialize_with", "deserialize_with = ...");
/// with_funcs - sub-schema nested meta, field-level only attribute
pub const WITH_FUNCS: Symbol = Symbol("with_funcs", "with_funcs(...)");
/// declaration - sub-with_funcs nested meta, field-level only attribute
pub const DECLARATION: Symbol = Symbol("declaration", "declaration = ...");
/// definitions - sub-with_funcs nested meta, field-level only attribute
pub const DEFINITIONS: Symbol = Symbol("definitions", "definitions = ...");

#[derive(Clone, Copy)]
pub(crate) enum BoundType {
    Serialize,
    Deserialize,
}
impl PartialEq<Symbol> for Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

impl<'a> PartialEq<Symbol> for &'a Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

pub(crate) fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path() == SKIP)
}

pub(crate) fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
    for attr in attrs.iter() {
        if attr.path() == INIT {
            let mut res = None;
            let _ = attr.parse_nested_meta(|meta| {
                res = Some(meta.path);
                Ok(())
            });
            return res;
        }
    }

    None
}
