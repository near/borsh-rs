use std::collections::BTreeMap;

use crate::attribute_helpers::{parsing::parse_lit_into_vec, Symbol, PARAMS, SCHEMA};
use once_cell::sync::Lazy;
use syn::{meta::ParseNestedMeta, Ident, Token, Type};

pub(crate) enum Variants {
    Params(Vec<ParamsOverride>),
}

type ParseFn = dyn Fn(Symbol, Symbol, &ParseNestedMeta) -> syn::Result<Variants> + Send + Sync;

pub(crate) static SCHEMA_FIELD_PARSE_MAP: Lazy<BTreeMap<Symbol, Box<ParseFn>>> = Lazy::new(|| {
    let mut m = BTreeMap::new();
    // has to be inlined; assigning closure separately doesn't work
    let f: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into_vec::<ParamsOverride>(attr_name, meta_item_name, meta).map(Variants::Params)
    });
    m.insert(PARAMS, f);
    m
});

/**
Struct describes an entry like `order_param => override_type`,  e.g. `K => <K as TraitName>::Associated`
*/
#[derive(Clone, syn_derive::Parse, syn_derive::ToTokens)]
pub struct ParamsOverride {
    pub order_param: Ident,
    arrow_token: Token![=>],
    pub override_type: Type,
}

#[derive(Default)]
pub(crate) struct Attributes {
    pub params: Option<Vec<ParamsOverride>>,
}

impl From<BTreeMap<Symbol, Variants>> for Attributes {
    fn from(mut map: BTreeMap<Symbol, Variants>) -> Self {
        let params = map.remove(&PARAMS);
        let params = params.and_then(|variant| match variant {
            Variants::Params(params) => Some(params),
        });
        Self { params }
    }
}
