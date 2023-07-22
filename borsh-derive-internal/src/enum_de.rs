use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::convert::TryFrom;
use syn::{Fields, Ident, ItemEnum, WhereClause};

use crate::{
    attribute_helpers::{contains_initialize_with, contains_skip, contains_use_discriminant},
    enum_discriminant_map::discriminant_map,
};

pub fn enum_de(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut where_clause = where_clause.map_or_else(
        || WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
        Clone::clone,
    );
    let init_method = contains_initialize_with(&input.attrs);
    let mut variant_arms = TokenStream2::new();

    let use_discriminant = contains_use_discriminant(&input.attrs).map_err(|err| {
        syn::Error::new(
            input.ident.span(),
            format!("error parsing `#[borsh(use_discriminant = ...)]`: {}", err),
        )
    })?;

    let discriminants = discriminant_map(&input.variants);
    let has_explicit_discriminants = input
        .variants
        .iter()
        .any(|variant| variant.discriminant.is_some());
    if has_explicit_discriminants && use_discriminant.is_none() {
        return Err(syn::Error::new(
            input.ident.span(),
            "You have to specify `#[borsh(use_discriminant=true)]` or `#[borsh(use_discriminant=false)]` for all structs that have enum with explicit discriminant",
        ));
    }
    let use_discriminant = use_discriminant.unwrap_or(false);

    for (variant_idx, variant) in input.variants.iter().enumerate() {
        let variant_idx = u8::try_from(variant_idx).map_err(|err| {
            syn::Error::new(
                variant.ident.span(),
                format!("up to 256 enum variants are supported. error{}", err),
            )
        })?;
        let variant_ident = &variant.ident;
        let discriminant = discriminants.get(variant_ident).unwrap();
        let mut variant_header = TokenStream2::new();
        match &variant.fields {
            Fields::Named(fields) => {
                for field in &fields.named {
                    let field_name = field.ident.as_ref().unwrap();
                    if contains_skip(&field.attrs) {
                        variant_header.extend(quote! {
                            #field_name: Default::default(),
                        });
                    } else {
                        let field_type = &field.ty;
                        where_clause.predicates.push(
                            syn::parse2(quote! {
                                #field_type: #cratename::BorshDeserialize
                            })
                            .unwrap(),
                        );

                        variant_header.extend(quote! {
                            #field_name: #cratename::BorshDeserialize::deserialize_reader(reader)?,
                        });
                    }
                }
                variant_header = quote! { { #variant_header }};
            }
            Fields::Unnamed(fields) => {
                for field in fields.unnamed.iter() {
                    if contains_skip(&field.attrs) {
                        variant_header.extend(quote! { Default::default(), });
                    } else {
                        let field_type = &field.ty;
                        where_clause.predicates.push(
                            syn::parse2(quote! {
                                #field_type: #cratename::BorshDeserialize
                            })
                            .unwrap(),
                        );

                        variant_header.extend(
                            quote! { #cratename::BorshDeserialize::deserialize_reader(reader)?, },
                        );
                    }
                }
                variant_header = quote! { ( #variant_header )};
            }
            Fields::Unit => {}
        }

        variant_arms.extend(quote! {
            #variant_idx => #name::#variant_ident #variant_header ,
        });
    }

    let init = if let Some(method_ident) = init_method {
        quote! {
            return_value.#method_ident();
        }
    } else {
        quote! {}
    };

    let variant_name = quote! { variant_idx };
    let return_value_code = quote! {
        let mut return_value = match variant_idx {
            #variant_arms
            _ => return Err(#cratename::__private::maybestd::io::Error::new(
                #cratename::__private::maybestd::io::ErrorKind::InvalidData,
                #cratename::__private::maybestd::format!("Unexpected variant index: {:?}", variant_idx),
            ))
        };
    };

    Ok(
        quote! { impl #impl_generics #cratename::de::BorshDeserialize for #name #ty_generics #where_clause {
                fn deserialize_reader<R: borsh::__private::maybestd::io::Read>(reader: &mut R) -> ::core::result::Result<Self, #cratename::__private::maybestd::io::Error> {
                    let tag = <u8 as #cratename::de::BorshDeserialize>::deserialize_reader(reader)?;
                    <Self as #cratename::de::EnumExt>::deserialize_variant(reader, tag)
                }
            }

            impl #impl_generics #cratename::de::EnumExt for #name #ty_generics #where_clause {
                fn deserialize_variant<R: #cratename::__private::maybestd::io::Read>(
                    reader: &mut R,
                    #variant_name: u8,
                ) -> ::core::result::Result<Self, #cratename::__private::maybestd::io::Error> {
                    #return_value_code
                    #init
                    Ok(return_value)
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::pretty_print_syn_str;

    use super::*;
    use proc_macro2::Span;

    #[test]
    fn borsh_skip_struct_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum AA {
                B {
                    #[borsh_skip]
                    c: i32,
                    d: u32,
                },
                NegatedVariant {
                    beta: u8,
                }
            }
        })
        .unwrap();
        let actual = enum_de(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn borsh_skip_tuple_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum AAT {
                B(#[borsh_skip] i32, u32),

                NegatedVariant {
                    beta: u8,
                }
            }
        })
        .unwrap();
        let actual = enum_de(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
