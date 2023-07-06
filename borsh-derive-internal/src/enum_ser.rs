use core::convert::TryFrom;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Fields, FieldsNamed, FieldsUnnamed, Ident, ItemEnum, Path, WhereClause};

use crate::{
    attribute_helpers::contains_skip, enum_discriminant_map::discriminant_map,
    generics::compute_predicates,
};

pub fn enum_ser(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let enum_ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut where_clause = where_clause.map_or_else(
        || WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
        Clone::clone,
    );

    let trait_path: Path = syn::parse2(quote! { #cratename::ser::BorshSerialize }).unwrap();
    let predicates = compute_predicates(&input.generics, &trait_path);

    predicates
        .into_iter()
        .for_each(|predicate| where_clause.predicates.push(predicate));

    let mut all_variants_idx_body = TokenStream2::new();
    let mut fields_body = TokenStream2::new();
    let discriminants = discriminant_map(&input.variants);
    for variant in input.variants.iter() {
        let variant_ident = &variant.ident;
        let discriminant_value = discriminants.get(variant_ident).unwrap();
        let VariantParts {
            variant_header,
            variant_body,
            variant_idx_body,
        } = match &variant.fields {
            Fields::Named(fields) => named_fields(
                &cratename,
                enum_ident,
                variant_ident,
                discriminant_value,
                fields,
            )?,
            Fields::Unnamed(fields) => unnamed_fields(
                &cratename,
                enum_ident,
                variant_ident,
                discriminant_value,
                fields,
            )?,
            Fields::Unit => {
                let variant_idx_body = quote!(
                    #enum_ident::#variant_ident => #discriminant_value,
                );
                VariantParts {
                    variant_header: TokenStream2::new(),
                    variant_body: TokenStream2::new(),
                    variant_idx_body,
                }
            }
        };
        all_variants_idx_body.extend(variant_idx_body);
        fields_body.extend(quote!(
            #enum_ident::#variant_ident #variant_header => {
                #variant_body
            }
        ))
    }
    Ok(quote! {
        impl #impl_generics #cratename::ser::BorshSerialize for #enum_ident #ty_generics #where_clause {
            fn serialize<W: #cratename::__private::maybestd::io::Write>(&self, writer: &mut W) -> ::core::result::Result<(), #cratename::__private::maybestd::io::Error> {
                let variant_idx: u8 = match self {
                    #all_variants_idx_body
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

struct VariantParts {
    variant_header: TokenStream2,
    variant_body: TokenStream2,
    variant_idx_body: TokenStream2,
}
fn named_fields(
    cratename: &Ident,
    enum_ident: &Ident,
    variant_ident: &Ident,
    discriminant_value: &TokenStream2,
    fields: &FieldsNamed,
) -> syn::Result<VariantParts> {
    let mut variant_header = TokenStream2::new();
    let mut variant_body = TokenStream2::new();
    for field in &fields.named {
        if !contains_skip(&field.attrs) {
            let field_ident = field.ident.clone().unwrap();

            variant_header.extend(quote! { #field_ident, });

            variant_body.extend(quote! {
                 #cratename::BorshSerialize::serialize(#field_ident, writer)?;
            })
        }
    }
    // `..` pattern matching works even if all fields were specified
    variant_header = quote! { { #variant_header .. }};
    let variant_idx_body = quote!(
        #enum_ident::#variant_ident { .. } => #discriminant_value,
    );
    Ok(VariantParts {
        variant_header,
        variant_body,
        variant_idx_body,
    })
}

fn unnamed_fields(
    cratename: &Ident,
    enum_ident: &Ident,
    variant_ident: &Ident,
    discriminant_value: &TokenStream2,
    fields: &FieldsUnnamed,
) -> syn::Result<VariantParts> {
    let mut variant_header = TokenStream2::new();
    let mut variant_body = TokenStream2::new();
    for (field_idx, field) in fields.unnamed.iter().enumerate() {
        let field_idx = u32::try_from(field_idx).expect("up to 2^32 fields are supported");
        if contains_skip(&field.attrs) {
            let field_ident = Ident::new(format!("_id{}", field_idx).as_str(), Span::mixed_site());
            variant_header.extend(quote! { #field_ident, });
        } else {
            let field_ident = Ident::new(format!("id{}", field_idx).as_str(), Span::mixed_site());

            variant_header.extend(quote! { #field_ident, });

            variant_body.extend(quote! {
                #cratename::BorshSerialize::serialize(#field_ident, writer)?;
            })
        }
    }
    variant_header = quote! { ( #variant_header )};
    let variant_idx_body = quote!(
        #enum_ident::#variant_ident(..) => #discriminant_value,
    );
    Ok(VariantParts {
        variant_header,
        variant_body,
        variant_idx_body,
    })
}

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
        })
        .unwrap();
        let actual = enum_ser(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn struct_variant_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum AB {
                B {
                    c: i32,
                    d: u32,
                },

                NegatedVariant {
                    beta: String,
                }
            }
        })
        .unwrap();

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
        })
        .unwrap();

        let actual = enum_ser(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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
        })
        .unwrap();

        let actual = enum_ser(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn simple_generics() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K, V, U> {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        })
        .unwrap();

        let actual = enum_ser(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn bound_generics() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        })
        .unwrap();

        let actual = enum_ser(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn recursive_enum() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<A>),
            }
        })
        .unwrap();

        let actual = enum_ser(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
