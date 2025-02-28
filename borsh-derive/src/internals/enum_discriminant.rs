use std::{collections::HashMap, convert::TryFrom};

use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{punctuated::Punctuated, token::Comma, Variant};

pub struct Discriminants(HashMap<Ident, TokenStream>);
impl Discriminants {
    /// Calculates the discriminant that will be assigned by the compiler.
    /// See: https://doc.rust-lang.org/reference/items/enumerations.html#assigning-discriminant-values
    pub fn new(variants: &Punctuated<Variant, Comma>) -> Self {
        let mut map = HashMap::new();
        let mut next_discriminant_if_not_specified = quote! {0};

        for variant in variants {
            let this_discriminant = variant.discriminant.clone().map_or_else(
                || next_discriminant_if_not_specified,
                |(_, e)| e.into_token_stream(),
            );

            next_discriminant_if_not_specified = quote! { #this_discriminant + 1 };
            map.insert(variant.ident.clone(), this_discriminant);
        }

        Self(map)
    }

    pub fn get(
        &self,
        variant_ident: &Ident,
        use_discriminant: bool,
        variant_idx: usize,
    ) -> syn::Result<TokenStream> {
        let variant_idx = u8::try_from(variant_idx).map_err(|err| {
            syn::Error::new(
                variant_ident.span(),
                format!("up to 256 enum variants are supported: {}", err),
            )
        })?;
        let result = if use_discriminant {
            self.0.get(variant_ident).unwrap().clone() // discriminant value
        } else {
            quote! { #variant_idx }
        };
        Ok(result)
    }
}
