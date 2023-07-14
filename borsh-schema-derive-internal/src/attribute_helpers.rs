// TODO: remove this unused attribute, when the unsplit is done
#![allow(unused)]
use syn::{Attribute, Field, Path, WherePredicate};
pub mod parsing_helpers;
use parsing_helpers::get_where_predicates;

use self::parsing_helpers::{get_schema_attrs, SchemaParamsOverride};

#[derive(Copy, Clone)]
pub struct Symbol(pub &'static str);

/// borsh - top level prefix in nested meta attribute
pub const BORSH: Symbol = Symbol("borsh");
/// bound - sub-borsh nested meta, field-level only, `BorshSerialize` and `BorshDeserialize` contexts
pub const BOUND: Symbol = Symbol("bound");
/// serialize - sub-bound nested meta attribute
pub const SERIALIZE: Symbol = Symbol("serialize");
/// deserialize - sub-bound nested meta attribute
pub const DESERIALIZE: Symbol = Symbol("deserialize");
/// borsh_skip - field-level only attribute, `BorshSerialize`, `BorshDeserialize`, `BorshSchema` contexts
pub const SKIP: Symbol = Symbol("borsh_skip");
/// borsh_init - item-level only attribute  `BorshDeserialize` context
pub const INIT: Symbol = Symbol("borsh_init");
/// schema - sub-borsh nested meta, `BorshSchema` context
pub const SCHEMA: Symbol = Symbol("schema");
/// params - sub-schema nested meta, field-level only attribute
pub const PARAMS: Symbol = Symbol("params");

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

pub(crate) type Bounds = Option<Vec<WherePredicate>>;
pub(crate) type SchemaParams = Option<Vec<SchemaParamsOverride>>;

fn parse_bounds(attrs: &[Attribute]) -> Result<(Bounds, Bounds), syn::Error> {
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

pub(crate) fn parse_schema_attrs(attrs: &[Attribute]) -> Result<SchemaParams, syn::Error> {
    let mut params: SchemaParams = None;
    for attr in attrs {
        if attr.path() != BORSH {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path == SCHEMA {
                // #[borsh(schema(params = "..."))]

                let params_parsed = get_schema_attrs(&meta)?;
                params = params_parsed;
            }
            Ok(())
        })?;
    }

    Ok(params)
}
pub(crate) enum BoundType {
    Serialize,
    Deserialize,
}

pub(crate) fn get_bounds(field: &Field, ty: BoundType) -> Result<Bounds, syn::Error> {
    let (ser, de) = parse_bounds(&field.attrs)?;
    match ty {
        BoundType::Serialize => Ok(ser),
        BoundType::Deserialize => Ok(de),
    }
}

pub(crate) fn collect_override_bounds(
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
