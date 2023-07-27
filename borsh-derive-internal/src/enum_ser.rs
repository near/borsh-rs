use core::convert::TryFrom;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Fields, FieldsNamed, FieldsUnnamed, Ident, ItemEnum, Path, WhereClause, WherePredicate};

use crate::{
    attribute_helpers::{
        collect_override_bounds, contains_skip, contains_use_discriminant, BoundType,
    },
    enum_discriminant_map::discriminant_map,
    generics::{compute_predicates, without_defaults, FindTyParams},
};

pub fn enum_ser(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let enum_ident = &input.ident;
    let generics = without_defaults(&input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = where_clause.map_or_else(
        || WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
        Clone::clone,
    );

    let mut serialize_params_visitor = FindTyParams::new(&generics);
    let mut override_predicates = vec![];
    let use_discriminant = contains_use_discriminant(&input.attrs).map_err(|err| {
        syn::Error::new(
            input.ident.span(),
            format!("error parsing `#[borsh(use_discriminant = ...)]`: {}", err),
        )
    })?;

    let mut all_variants_idx_body = TokenStream2::new();
    let mut fields_body = TokenStream2::new();
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

    assert!(
        input.variants.len() < 256,
        "up to 256 enum variants are supported"
    );
    dbg!("test");
    for (variant_idx, variant) in input.variants.iter().enumerate() {
        let variant_idx = u8::try_from(variant_idx).expect("up to 256 enum variants are supported");
        let variant_ident = &variant.ident;
        let discriminant_value = discriminants.get(variant_ident).unwrap();

        let VariantParts {
            variant_header,
            variant_body,
            variant_idx_body,
        } = match &variant.fields {
            Fields::Named(fields) => {
                let variant_idx_body = if use_discriminant {
                    quote!(
                        #enum_ident::#variant_ident {..} => #discriminant_value,
                    )
                } else {
                    quote!(
                        #enum_ident::#variant_ident {..} => #variant_idx,
                    )
                };
                named_fields(
                    &cratename,
                    enum_ident,
                    variant_ident,
                    discriminant_value,
                    fields,
                    &mut serialize_params_visitor,
                    &mut override_predicates,
                    variant_idx_body,
                )?
            }
            Fields::Unnamed(fields) => {
                let variant_idx_body = if use_discriminant {
                    quote!(
                        #enum_ident::#variant_ident(..) => #discriminant_value,
                    )
                } else {
                    quote!(
                        #enum_ident::#variant_ident(..) => #variant_idx,
                    )
                };
                unnamed_fields(
                    &cratename,
                    enum_ident,
                    variant_ident,
                    discriminant_value,
                    fields,
                    &mut serialize_params_visitor,
                    &mut override_predicates,
                    variant_idx_body,
                )?
            }
            Fields::Unit => {
                let variant_idx_body = if use_discriminant {
                    quote!(
                        #enum_ident::#variant_ident => #discriminant_value,
                    )
                } else {
                    quote!(
                        #enum_ident::#variant_ident => #variant_idx,
                    )
                };
                dbg!(&variant_idx_body.to_string());

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
    let trait_path: Path = syn::parse2(quote! { #cratename::ser::BorshSerialize }).unwrap();
    let predicates = compute_predicates(serialize_params_visitor.process_for_bounds(), &trait_path);
    where_clause.predicates.extend(predicates);
    where_clause.predicates.extend(override_predicates);
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
    params_visitor: &mut FindTyParams,
    override_output: &mut Vec<WherePredicate>,
    variant_idx_body: TokenStream2,
) -> syn::Result<VariantParts> {
    let mut variant_header = TokenStream2::new();
    let mut variant_body = TokenStream2::new();
    for field in &fields.named {
        let bounds_override =
            collect_override_bounds(field, BoundType::Serialize, override_output)?;
        if !contains_skip(&field.attrs) {
            let field_ident = field.ident.clone().unwrap();

            variant_header.extend(quote! { #field_ident, });

            variant_body.extend(quote! {
                 #cratename::BorshSerialize::serialize(#field_ident, writer)?;
            });
            if !bounds_override {
                params_visitor.visit_field(field);
            }
        }
    }
    // `..` pattern matching works even if all fields were specified
    variant_header = quote! { { #variant_header .. }};
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
    params_visitor: &mut FindTyParams,
    override_output: &mut Vec<WherePredicate>,
    variant_idx_body: TokenStream2,
) -> syn::Result<VariantParts> {
    let mut variant_header = TokenStream2::new();
    let mut variant_body = TokenStream2::new();
    for (field_idx, field) in fields.unnamed.iter().enumerate() {
        let bounds_override =
            collect_override_bounds(field, BoundType::Serialize, override_output)?;
        let field_idx = u32::try_from(field_idx).expect("up to 2^32 fields are supported");
        if contains_skip(&field.attrs) {
            let field_ident = Ident::new(format!("_id{}", field_idx).as_str(), Span::mixed_site());
            variant_header.extend(quote! { #field_ident, });
        } else {
            let field_ident = Ident::new(format!("id{}", field_idx).as_str(), Span::mixed_site());

            variant_header.extend(quote! { #field_ident, });

            variant_body.extend(quote! {
                #cratename::BorshSerialize::serialize(#field_ident, writer)?;
            });
            if !bounds_override {
                params_visitor.visit_field(field);
            }
        }
    }
    variant_header = quote! { ( #variant_header )};
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

    #[test]
    fn generic_borsh_skip_struct_field() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    #[borsh_skip]
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
    fn generic_borsh_skip_tuple_field() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, #[borsh_skip] Vec<U>),
            }
        })
        .unwrap();

        let actual = enum_ser(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_serialize_bound() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<T: Debug, U> {
                C {
                    a: String,
                    #[borsh(bound(serialize =
                        "T: borsh::ser::BorshSerialize + PartialOrd,
                         U: borsh::ser::BorshSerialize"
                    ))]
                    b: HashMap<T, U>,
                },
                D(u32, u32),
            }
        })
        .unwrap();

        let actual = enum_ser(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
