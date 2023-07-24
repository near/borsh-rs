use std::collections::BTreeMap;

use syn::{meta::ParseNestedMeta, WherePredicate};

use crate::attribute_helpers::{parsing::parse_lit_into_vec, Symbol, DESERIALIZE, SERIALIZE};
use once_cell::sync::Lazy;

pub(crate) enum Variants {
    Serialize(Vec<WherePredicate>),
    Deserialize(Vec<WherePredicate>),
}

type ParseFn = dyn Fn(Symbol, Symbol, &ParseNestedMeta) -> syn::Result<Variants> + Send + Sync;

pub(crate) static BOUNDS_FIELD_PARSE_MAP: Lazy<BTreeMap<Symbol, Box<ParseFn>>> = Lazy::new(|| {
    let mut m = BTreeMap::new();
    // this coercion is required due to some completely mysterious reason
    let f1: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into_vec::<WherePredicate>(attr_name, meta_item_name, meta)
            .map(Variants::Serialize)
    });
    let f2: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into_vec::<WherePredicate>(attr_name, meta_item_name, meta)
            .map(Variants::Deserialize)
    });
    m.insert(SERIALIZE, f1);
    m.insert(DESERIALIZE, f2);
    m
});

#[derive(Default)]
pub(crate) struct Attributes {
    pub serialize: Option<Vec<WherePredicate>>,
    pub deserialize: Option<Vec<WherePredicate>>,
}

impl From<BTreeMap<Symbol, Variants>> for Attributes {
    fn from(mut map: BTreeMap<Symbol, Variants>) -> Self {
        let serialize = map.remove(&SERIALIZE);
        let deserialize = map.remove(&DESERIALIZE);
        let serialize = serialize.and_then(|variant| match variant {
            Variants::Serialize(ser) => Some(ser),
            _ => None,
        });
        let deserialize = deserialize.and_then(|variant| match variant {
            Variants::Deserialize(de) => Some(de),
            _ => None,
        });
        Self {
            serialize,
            deserialize,
        }
    }
}
