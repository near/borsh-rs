use std::collections::HashMap;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{punctuated::Punctuated, token::Comma, Variant};

/// Calculates the discriminant that will be assigned by the compiler.
/// See: https://doc.rust-lang.org/reference/items/enumerations.html#assigning-discriminant-values
pub fn discriminant_map(
    variants: &Punctuated<Variant, Comma>,
) -> (HashMap<Ident, TokenStream>, bool) {
    let mut map = HashMap::new();

    let mut next_discriminant_if_not_specified = quote! {0};

    let mut has_discriminant = false;

    for variant in variants {
        let this_discriminant = variant.discriminant.clone().map_or_else(
            || quote! { #next_discriminant_if_not_specified },
            |(_, e)| quote! { #e },
        );
        if !this_discriminant.to_string().starts_with("0") {
            has_discriminant = true;
        }
        next_discriminant_if_not_specified = quote! { #this_discriminant + 1 };
        map.insert(variant.ident.clone(), this_discriminant);
    }

    (map, has_discriminant)
}
