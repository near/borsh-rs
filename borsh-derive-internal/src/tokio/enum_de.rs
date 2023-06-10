use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, Ident, ItemEnum, WhereClause};

use crate::{
    attribute_helpers::{contains_initialize_with, contains_skip},
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
    let init_method = contains_initialize_with(&input.attrs)?;
    let mut variant_arms = TokenStream2::new();
    let discriminants = discriminant_map(&input.variants);
    for variant in input.variants.iter() {
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
                                #field_type: #cratename::tokio::AsyncBorshDeserialize
                            })
                            .unwrap(),
                        );

                        variant_header.extend(quote! {
                            #field_name: #cratename::AsyncBorshDeserialize::deserialize_reader(reader).await?,
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
                                #field_type: #cratename::tokio::AsyncBorshDeserialize
                            })
                            .unwrap(),
                        );

                        variant_header.extend(
                            quote! { #cratename::tokio::AsyncBorshDeserialize::deserialize_reader(reader).await?, },
                        );
                    }
                }
                variant_header = quote! { ( #variant_header )};
            }
            Fields::Unit => {}
        }
        variant_arms.extend(quote! {
            if variant_tag == #discriminant { #name::#variant_ident #variant_header } else
        });
    }

    let init = if let Some(method_ident) = init_method {
        quote! {
            return_value.#method_ident();
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[async_trait::async_trait]
        impl #impl_generics #cratename::tokio::de::AsyncBorshDeserialize for #name #ty_generics #where_clause {
            async fn deserialize_reader<R: borsh::maybestd::io::Read>(reader: &mut R) -> ::core::result::Result<Self, #cratename::maybestd::io::Error> {
                let tag = <u8 as #cratename::de::BorshDeserialize>::deserialize_reader(reader).await?;
                <Self as #cratename::de::EnumExt>::deserialize_variant(reader, tag).await
            }
        }

        #[async_trait::async_trait]
        impl #impl_generics #cratename::de::EnumExt for #name #ty_generics #where_clause {
            async fn deserialize_variant<R: #cratename::tokio::de::AsyncReader>(
                reader: &mut R,
                variant_tag: u8,
            ) -> ::core::result::Result<Self, #cratename::maybestd::io::Error> {
                let mut return_value =
                    #variant_arms {
                    return Err(#cratename::maybestd::io::Error::new(
                        #cratename::maybestd::io::ErrorKind::InvalidInput,
                        #cratename::maybestd::format!("Unexpected variant tag: {:?}", variant_tag),
                    ))
                };
                #init
                Ok(return_value)
            }
        }
    })
}
