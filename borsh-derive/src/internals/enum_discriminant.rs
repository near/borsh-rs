use core::convert::TryInto;
use std::collections::HashMap;
use std::convert::TryFrom;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{punctuated::Punctuated, spanned::Spanned, token::Comma, Variant};

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

        fn bytes_needed(value: usize) -> usize {
            let bits_needed = std::mem::size_of::<usize>() * 8 - value.leading_zeros() as usize;
            (bits_needed + 7) / 8
        }

        let min_tag_width: u8 = bytes_needed(variants.len())
            .try_into()
            .expect("variants cannot be bigger u64");

        let mut min = 0;
        let mut max = 0;

        for variant in variants {
            if let Some(discriminant) = &variant.discriminant {
                let value = discriminant.1.to_token_stream().to_string();
                let value = value.parse::<i128>().unwrap();
                min = value.min(min);
                max = value.max(max);
            }

            let this_discriminant = variant.discriminant.clone().map_or_else(
                || quote! { #next_discriminant_if_not_specified },
                |(_, e)| quote! { #e },
            );

            next_discriminant_if_not_specified = quote! { #this_discriminant + 1 };
            map.insert(variant.ident.clone(), this_discriminant);
        }

        let min: u64 = min.abs().try_into().expect("variants cannot be bigger u64");
        let max: u64 = max.try_into().expect("variants cannot be bigger u64");
        if min > 0 {
            return Err(syn::Error::new(
                variants.span(),
                "negative variants are not supported, please write manual impl",
            ));
        }
        let max_bytes: u8 = bytes_needed(max as usize).try_into().expect("");
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

        let tag_with = maybe_borsh_tag_width
            .map(|(tag_width, _)| tag_width)
            .unwrap_or_else(|| (max_bytes).max(min_tag_width));

        let tag_width_type = if tag_with <= 1 {
            "u8"
        } else if tag_with <= 2 {
            "u16"
        } else if tag_with <= 4 {
            "u32"
        } else if tag_with <= 8 {
            "u64"
        } else {
            unreachable!("we eliminated such error earlier")
        };
        let discriminant_type = syn::parse_str(tag_width_type).expect("numeric");

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
