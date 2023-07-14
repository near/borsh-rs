use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    punctuated::Punctuated, token::Comma, Attribute, GenericParam, Generics, Ident, Type,
    WherePredicate,
};

use crate::{
    attribute_helpers::{BORSH, SKIP},
    generics::type_contains_some_param,
};

pub fn filter_field_attrs(
    attrs: impl Iterator<Item = Attribute>,
) -> impl Iterator<Item = Attribute> {
    attrs.filter(|attr| attr.path() == SKIP || attr.path() == BORSH)
}

pub fn declaration(
    ident_str: &str,
    cratename: Ident,
    params_for_bounds: Vec<Type>,
) -> TokenStream2 {
    // Generate function that returns the name of the type.
    let mut declaration_params = vec![];
    for type_param in params_for_bounds {
        declaration_params.push(quote! {
            <#type_param>::declaration()
        });
    }
    if declaration_params.is_empty() {
        quote! {
                #ident_str.to_string()
        }
    } else {
        quote! {
                let params = #cratename::__private::maybestd::vec![#(#declaration_params),*];
                format!(r#"{}<{}>"#, #ident_str, params.join(", "))
        }
    }
}

pub fn filter_used_params(
    generics: &Generics,
    not_skipped_type_params: HashSet<Ident>,
) -> Generics {
    let new_params = generics
        .params
        .clone()
        .into_iter()
        .filter(|param| match param {
            GenericParam::Lifetime(..) | GenericParam::Const(..) => true,
            GenericParam::Type(ty_param) => not_skipped_type_params.contains(&ty_param.ident),
        })
        .collect();

    let mut where_clause = generics.where_clause.clone();
    where_clause = where_clause.map(|mut clause| {
        let new_predicates: Punctuated<WherePredicate, Comma> = clause
            .predicates
            .iter()
            .filter(|predicate| match predicate {
                WherePredicate::Lifetime(..) => true,
                WherePredicate::Type(predicate_type) => {
                    type_contains_some_param(&predicate_type.bounded_ty, &not_skipped_type_params)
                }
                #[cfg_attr(
                    feature = "force_exhaustive_checks",
                    deny(non_exhaustive_omitted_patterns)
                )]
                _ => true,
            })
            .cloned()
            .collect();
        clause.predicates = new_predicates;
        clause
    });
    Generics {
        params: new_params,
        where_clause,
        ..generics.clone()
    }
}
