use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{punctuated::Punctuated, Attribute, GenericParam, Generics, Ident, Type, TypeParam};

use crate::attribute_helpers::{SKIP, BORSH};


pub fn filter_field_attrs(attrs: impl Iterator<Item = Attribute>) -> impl Iterator<Item = Attribute> {
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
        .map(|param| match param {
            param @ GenericParam::Lifetime(..) | param @ GenericParam::Const(..) => param,
            GenericParam::Type(ty_param) => GenericParam::Type(TypeParam {
                bounds: Punctuated::default(),
                ..ty_param
            }),
        })
        .collect();

    Generics {
        params: new_params,
        where_clause: None,
        ..generics.clone()
    }
}
