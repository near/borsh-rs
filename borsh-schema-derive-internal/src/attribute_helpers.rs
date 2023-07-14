// TODO: remove this unused attribute, when the unsplit is done
#![allow(unused)]
use syn::{Attribute, Field, Path, WherePredicate};
pub mod parsing_helpers;
use parsing_helpers::get_where_predicates;

#[derive(Copy, Clone)]
pub struct Symbol(pub &'static str);

/// top level prefix in nested meta attribute
pub const BORSH: Symbol = Symbol("borsh");
/// sub-BORSH nested meta, field-level only attribute, `BorshSerialize` and `BorshDeserialize` contexts
pub const BOUND: Symbol = Symbol("bound");
/// sub-BOUND nested meta attribute
pub const SERIALIZE: Symbol = Symbol("serialize");
/// sub-BOUND nested meta attribute
pub const DESERIALIZE: Symbol = Symbol("deserialize");
/// field-level only attribute, `BorshSerialize`, `BorshDeserialize`, `BorshSchema` contexts
pub const SKIP: Symbol = Symbol("borsh_skip");
/// item-level only attribute  `BorshDeserialize` context
pub const INIT: Symbol = Symbol("borsh_init");
/// sub-BORSH nested meta, field-level only attribute, `BorshSchema` context
pub const SCHEMA_PARAMS: Symbol = Symbol("schema_params");

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

pub fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path() == SKIP)
}

pub fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
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

type Bounds = Option<Vec<WherePredicate>>;

pub fn parse_bounds(attrs: &[Attribute]) -> Result<(Bounds, Bounds), syn::Error> {
    let (mut ser, mut de): (Bounds, Bounds) = (None, None);
    for attr in attrs {
        if attr.path() != BORSH {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path == BOUND {
                // #[borsh(bound(serialize = "...", deserialize = "..."))]

                let (ser_parsed, de_parsed) = get_where_predicates(&meta)?;
                ser = ser_parsed;
                de = de_parsed;
            }
            Ok(())
        })?;
    }

    Ok((ser, de))
}

pub enum BoundType {
    Serialize,
    Deserialize,
}

pub fn get_bounds(field: &Field, ty: BoundType) -> Result<Bounds, syn::Error> {
    let (ser, de) = parse_bounds(&field.attrs)?;
    match ty {
        BoundType::Serialize => Ok(ser),
        BoundType::Deserialize => Ok(de),
    }
}

pub fn collect_override_bounds(
    field: &Field,
    ty: BoundType,
    output: &mut Vec<WherePredicate>,
) -> Result<bool, syn::Error> {
    let predicates = get_bounds(field, ty)?;
    match predicates {
        Some(predicates) => {
            output.extend(predicates);
            Ok(true)
        }
        None => Ok(false),
    }
}
