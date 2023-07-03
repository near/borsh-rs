use core::convert::TryFrom;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Fields, Ident, ItemEnum, WhereClause};

use crate::{attribute_helpers::contains_skip, enum_discriminant_map::discriminant_map};

pub fn enum_ser(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut where_clause = where_clause.map_or_else(
        || WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
        Clone::clone,
    );
    let mut variant_idx_body = TokenStream2::new();
    let mut fields_body = TokenStream2::new();
    let discriminants = discriminant_map(&input.variants);
    for variant in input.variants.iter() {
        let variant_ident = &variant.ident;
        let mut variant_header = TokenStream2::new();
        let mut variant_body = TokenStream2::new();
        let discriminant_value = discriminants.get(variant_ident).unwrap();
        match &variant.fields {
            Fields::Named(fields) => {
                for field in &fields.named {
                    let field_name = field.ident.as_ref().unwrap();
                    if contains_skip(&field.attrs) {
                        variant_header.extend(quote! { _#field_name, });
                        continue;
                    } else {
                        let field_type = &field.ty;
                        where_clause.predicates.push(
                            syn::parse2(quote! {
                                #field_type: #cratename::ser::BorshSerialize
                            })
                            .unwrap(),
                        );
                        variant_header.extend(quote! { #field_name, });
                    }
                    variant_body.extend(quote! {
                         #cratename::BorshSerialize::serialize(#field_name, writer)?;
                    })
                }
                variant_header = quote! { { #variant_header }};
                variant_idx_body.extend(quote!(
                    #name::#variant_ident { .. } => #discriminant_value,
                ));
            }
            Fields::Unnamed(fields) => {
                for (field_idx, field) in fields.unnamed.iter().enumerate() {
                    let field_idx =
                        u32::try_from(field_idx).expect("up to 2^32 fields are supported");
                    if contains_skip(&field.attrs) {
                        let field_ident =
                            Ident::new(format!("_id{}", field_idx).as_str(), Span::call_site());
                        variant_header.extend(quote! { #field_ident, });
                        continue;
                    } else {
                        let field_type = &field.ty;
                        where_clause.predicates.push(
                            syn::parse2(quote! {
                                #field_type: #cratename::ser::BorshSerialize
                            })
                            .unwrap(),
                        );

                        let field_ident =
                            Ident::new(format!("id{}", field_idx).as_str(), Span::call_site());
                        variant_header.extend(quote! { #field_ident, });
                        variant_body.extend(quote! {
                            #cratename::BorshSerialize::serialize(#field_ident, writer)?;
                        })
                    }
                }
                variant_header = quote! { ( #variant_header )};
                variant_idx_body.extend(quote!(
                    #name::#variant_ident(..) => #discriminant_value,
                ));
            }
            Fields::Unit => {
                variant_idx_body.extend(quote!(
                    #name::#variant_ident => #discriminant_value,
                ));
            }
        }
        fields_body.extend(quote!(
            #name::#variant_ident #variant_header => {
                #variant_body
            }
        ))
    }
    Ok(quote! {
        impl #impl_generics #cratename::ser::BorshSerialize for #name #ty_generics #where_clause {
            fn serialize<W: #cratename::__private::maybestd::io::Write>(&self, writer: &mut W) -> ::core::result::Result<(), #cratename::__private::maybestd::io::Error> {
                let variant_idx: u8 = match self {
                    #variant_idx_body
                };
                writer.write_all(&variant_idx.to_le_bytes())?;

                match self {
                    #fields_body
                }
                Ok(())
            }
        }
    })
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use crate::test_helpers::pretty_print_syn_str;

    use super::*;
    use proc_macro2::Span;
    #[test]
    fn borsh_skip_tuple_variant_field() {

        let item_enum: ItemEnum = syn::parse2(quote! {
            enum AATTB {
                B(#[borsh_skip] i32, #[borsh_skip] u32),

                NegatedVariant {
                    beta: u8,
                }
            }
        }).unwrap(); 
        let actual = enum_ser(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn borsh_skip_struct_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {

            enum AB {
                B {
                    #[borsh_skip]
                    c: i32,

                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        }).unwrap(); 

        let actual = enum_ser(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        println!("{}", quote!(#actual));
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
        assert!(false);
        
    }

    #[test]
    fn borsh_skip_struct_variant_all_fields() {
        let item_enum: ItemEnum = syn::parse2(quote! {

            enum AAB {
                B {
                    #[borsh_skip]
                    c: i32,

                    #[borsh_skip]
                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        }).unwrap(); 

        let actual = enum_ser(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        println!("{}", quote!(#actual));
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
        assert!(false);
        
    }
}
