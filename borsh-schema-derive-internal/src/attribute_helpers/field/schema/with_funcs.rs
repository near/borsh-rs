use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use syn::meta::ParseNestedMeta;

use crate::attribute_helpers::{parsing::parse_lit_into, Symbol, DECLARATION, DEFINITIONS};

pub(crate) enum Variants {
    Declaration(syn::ExprPath),
    Definitions(syn::ExprPath),
}

type ParseFn = dyn Fn(Symbol, Symbol, &ParseNestedMeta) -> syn::Result<Variants> + Send + Sync;

pub(crate) static WITH_FUNCS_FIELD_PARSE_MAP: Lazy<BTreeMap<Symbol, Box<ParseFn>>> =
    Lazy::new(|| {
        let mut m = BTreeMap::new();
        // has to be inlined; assigning closure separately doesn't work
        let f1: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
            parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
                .map(Variants::Declaration)
        });

        let f2: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
            parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
                .map(Variants::Definitions)
        });

        m.insert(DECLARATION, f1);
        m.insert(DEFINITIONS, f2);
        m
    });

pub(crate) struct WithFuncs {
    pub declaration: Option<syn::ExprPath>,
    pub definitions: Option<syn::ExprPath>,
}

impl From<BTreeMap<Symbol, Variants>> for WithFuncs {
    fn from(mut map: BTreeMap<Symbol, Variants>) -> Self {
        let declaration = map.remove(&DECLARATION);
        let definitions = map.remove(&DEFINITIONS);
        let declaration = declaration.map(|variant| match variant {
            Variants::Declaration(declaration) => declaration,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });
        let definitions = definitions.map(|variant| match variant {
            Variants::Definitions(definitions) => definitions,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });
        Self {
            declaration,
            definitions,
        }
    }
}
