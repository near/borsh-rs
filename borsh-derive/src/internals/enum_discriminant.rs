use std::collections::HashMap;
use std::convert::TryFrom;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{punctuated::Punctuated, token::Comma, Variant};

pub struct Discriminants((HashMap<Ident, TokenStream>, syn::TypePath));
impl Discriminants {
    /// Calculates the discriminant that will be assigned by the compiler.
    /// See: https://doc.rust-lang.org/reference/items/enumerations.html#assigning-discriminant-values
    pub fn new(
        variants: &Punctuated<Variant, Comma>,
        maybe_borsh_tag_width: Option<(u8, Span)>,
    ) -> syn::Result<Self> {
        let mut map = HashMap::new();
        let mut next_discriminant_if_not_specified = quote! {0};

        let min_tag_width = variants.len().next_power_of_two().trailing_zeros();

        for variant in variants {
            if let Some(discriminant) = variant.discriminant {
                let value = discriminant.1.to_token_stream().to_string();
                let value = value.parse::<i128>().unwrap();

            } 
            let this_discriminant = variant.discriminant.clone().map_or_else(
                || quote! { #next_discriminant_if_not_specified },
                |(_, e)| quote! { #e },
            );

            next_discriminant_if_not_specified = quote! { #this_discriminant + 1 };
            map.insert(variant.ident.clone(), this_discriminant);
        }

        if let Some((borsh_tag_width, span)) = maybe_borsh_tag_width {
            if borsh_tag_width < min_tag_width {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "borsh_tag_width={} is too small for the number of variants={}",
                        borsh_tag_width,
                        variants.len()
                    ),
                ));
            }
        }

        let discriminant_type =
            maybe_discriminant_type.unwrap_or(syn::parse_str("u8").expect("numeric"));

        Ok(Self((map, discriminant_type)))
    }

    pub fn discriminant_type(&self) -> &syn::TypePath {
        &self.0 .1
    }

    pub fn get(
        &self,
        variant_ident: &Ident,
        use_discriminant: bool,
        variant_idx: usize,
    ) -> syn::Result<TokenStream> {
        let variant_idx: u8 = u8::try_from(variant_idx).map_err(|err| {
            syn::Error::new(
                variant_ident.span(),
                format!(
                    "up to {} enum variants are supported: {}",
                    u8::MAX as usize + 1,
                    err
                ),
            )
        })?;
        let result = if use_discriminant {
            let discriminant_value = self.0 .0.get(variant_ident).unwrap();
            quote! { #discriminant_value }
        } else {
            quote! { #variant_idx }
        };
        Ok(result)
    }
}
