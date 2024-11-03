use core::convert::TryInto;
use std::collections::HashMap;
use std::convert::TryFrom;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{punctuated::Punctuated, spanned::Spanned, token::Comma, Variant};

pub struct Discriminants{
    variants: HashMap<Ident, TokenStream>, 
    discriminant_type : syn::TypePath,
    use_discriminant: bool,
}

impl Discriminants {
    /// Calculates the discriminant that will be assigned by the compiler.
    /// See: https://doc.rust-lang.org/reference/items/enumerations.html#assigning-discriminant-values
    pub fn new(
        variants: &Punctuated<Variant, Comma>,
        maybe_borsh_tag_width: Option<(u8, Span)>,
        maybe_rust_repr: Option<(syn::TypePath,Span)>,
        use_discriminant: bool,
    ) -> syn::Result<Self> {
        let mut map = HashMap::new();
        let mut next_discriminant_if_not_specified = quote! {0};

        for variant in variants {
            let this_discriminant = variant.discriminant.clone().map_or_else(
                || quote! { #next_discriminant_if_not_specified },
                |(_, e)| quote! { #e },
            );

            next_discriminant_if_not_specified = quote! { #this_discriminant + 1 };
            map.insert(variant.ident.clone(), this_discriminant);
        }    

        let mut discriminant_type = syn::parse_str("u8").unwrap();

        if let Some((tag_width, span)) = maybe_borsh_tag_width {
            if !use_discriminant {
                return Err(syn::Error::new(
                    span,
                    "`tag_width` specifier is only allowed when `use_discriminant` is set to true",
                ));
            }
            let Some((rust_repr, span)) = maybe_rust_repr else {
                return Err(syn::Error::new(
                    span,
                    "`tag_width` specifier is only allowed when `repr` is set",
                ));
            };
            match rust_repr.path.get_ident() {
                Some(repr_type) =>  {
                    let repr_size= match repr_type.to_string().as_str() {
                        "u8" => {
                            discriminant_type = syn::parse_str("u8").unwrap();
                            1
                        },
                        "u16" => {
                            discriminant_type = syn::parse_str("u16").unwrap();
                            2
                        },
                        "u32" => {
                            discriminant_type = syn::parse_str("u32").unwrap();
                            4
                        },
                        _ => return Err(syn::Error::new(
                            span,
                            "`tag_width` specifier is only allowed when `repr` is set to a u8, u16, or u32",
                        )),
                    };

                    if repr_size != tag_width {
                        return Err(syn::Error::new(
                            span,
                            "`tag_width` specifier must match the size of the `repr` type",
                        ));
                    }
                }
                None => {
                    return Err(syn::Error::new(
                        span,
                        "`tag_width` specifier is only allowed when `repr` is set to a specific numeric type",
                    ));
                }
            }
        }

        Ok(Self {
            variants : map,
            discriminant_type,
            use_discriminant
        })
    }

    pub fn discriminant_type(&self) -> &syn::TypePath {
        &self.discriminant_type
    }

    pub fn get(
        &self,
        variant_ident: &Ident,
        variant_idx: usize,
    ) -> syn::Result<TokenStream> {
        let result = if self.use_discriminant {
            let discriminant_value = self.variants.get(variant_ident).unwrap();
            quote! { #discriminant_value }
        } else {
            let variant_idx = u8::try_from(variant_idx).map_err(|err| {
                syn::Error::new(
                    variant_ident.span(),
                    format!(
                        "up to {} enum variants are supported: {}",
                        u8::MAX as usize + 1,
                        err
                    ),
                )
            })?;            
            quote! { #variant_idx }
        };
        Ok(result)
    }
}
