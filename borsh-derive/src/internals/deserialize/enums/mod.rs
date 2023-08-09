use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, Ident, ItemEnum, Path, WhereClause};

use crate::internals::{
    attributes::{field, item, BoundType},
    deserialize, enum_discriminant, generics,
};
use std::convert::TryFrom;

pub fn process(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = generics::without_defaults(&input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = where_clause.map_or_else(
        || WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
        Clone::clone,
    );

    let mut override_predicates = vec![];
    let mut deserialize_params_visitor = generics::FindTyParams::new(&generics);
    let mut default_params_visitor = generics::FindTyParams::new(&generics);

    let init_method = item::contains_initialize_with(&input.attrs);

    let use_discriminant = item::contains_use_discriminant(input)?;

    let mut variant_arms = TokenStream2::new();
    let discriminants = enum_discriminant::map(&input.variants);

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
                    let skipped = field::contains_skip(&field.attrs);
                    let parsed = field::Attributes::parse(&field.attrs, skipped)?;
                    override_predicates.extend(parsed.collect_bounds(BoundType::Deserialize));
                    let needs_bounds_derive = parsed.needs_bounds_derive(BoundType::Deserialize);
                    let field_name = field.ident.as_ref().unwrap();
                    if skipped {
                        if needs_bounds_derive {
                            default_params_visitor.visit_field(field);
                        }
                        variant_header.extend(quote! {
                            #field_name: core::default::Default::default(),
                        });
                    } else {
                        if needs_bounds_derive {
                            deserialize_params_visitor.visit_field(field);
                        }

                        variant_header.extend(deserialize::field_output(
                            Some(field_name),
                            &cratename,
                            parsed.deserialize_with,
                        ));
                    }
                }
                variant_header = quote! { { #variant_header }};
            }
            Fields::Unnamed(fields) => {
                for field in fields.unnamed.iter() {
                    let skipped = field::contains_skip(&field.attrs);
                    let parsed = field::Attributes::parse(&field.attrs, skipped)?;

                    override_predicates.extend(parsed.collect_bounds(BoundType::Deserialize));
                    let needs_bounds_derive = parsed.needs_bounds_derive(BoundType::Deserialize);
                    if skipped {
                        if needs_bounds_derive {
                            default_params_visitor.visit_field(field);
                        }
                        variant_header.extend(quote! { core::default::Default::default(), });
                    } else {
                        if needs_bounds_derive {
                            deserialize_params_visitor.visit_field(field);
                        }
                        variant_header.extend(deserialize::field_output(
                            None,
                            &cratename,
                            parsed.deserialize_with,
                        ));
                    }
                }
                variant_header = quote! { ( #variant_header )};
            }
            Fields::Unit => {}
        }
        let discriminant = if use_discriminant {
            quote! { #discriminant }
        } else {
            quote! { #variant_idx }
        };
        variant_arms.extend(quote! {
            if variant_tag == #discriminant { #name::#variant_ident #variant_header } else
        });
    }

    let init = if let Some(init_method) = init_method {
        let method_ident = syn::Ident::new(&init_method, input.ident.span());
        quote! {
            return_value.#method_ident();
        }
    } else {
        quote! {}
    };

    let de_trait_path: Path = syn::parse2(quote! { #cratename::de::BorshDeserialize }).unwrap();
    let default_trait_path: Path = syn::parse2(quote! { core::default::Default }).unwrap();
    let de_predicates = generics::compute_predicates(
        deserialize_params_visitor.process_for_bounds(),
        &de_trait_path,
    );
    let default_predicates = generics::compute_predicates(
        default_params_visitor.process_for_bounds(),
        &default_trait_path,
    );
    where_clause.predicates.extend(de_predicates);
    where_clause.predicates.extend(default_predicates);
    where_clause.predicates.extend(override_predicates);
    Ok(quote! {
        impl #impl_generics #cratename::de::BorshDeserialize for #name #ty_generics #where_clause {
            fn deserialize_reader<R: borsh::__private::maybestd::io::Read>(reader: &mut R) -> ::core::result::Result<Self, #cratename::__private::maybestd::io::Error> {
                let tag = <u8 as #cratename::de::BorshDeserialize>::deserialize_reader(reader)?;
                <Self as #cratename::de::EnumExt>::deserialize_variant(reader, tag)
            }
        }

        impl #impl_generics #cratename::de::EnumExt for #name #ty_generics #where_clause {
            fn deserialize_variant<R: borsh::__private::maybestd::io::Read>(
                reader: &mut R,
                variant_tag: u8,
            ) -> ::core::result::Result<Self, #cratename::__private::maybestd::io::Error> {
                let mut return_value =
                    #variant_arms {
                    return Err(#cratename::__private::maybestd::io::Error::new(
                        #cratename::__private::maybestd::io::ErrorKind::InvalidData,
                        #cratename::__private::maybestd::format!("Unexpected variant tag: {:?}", variant_tag),
                    ))
                };
                #init
                Ok(return_value)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::internals::test_helpers::{local_insta_assert_snapshot, pretty_print_syn_str};

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
        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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
        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_deserialize_bound() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A<T: Debug, U> {
                C {
                    a: String,
                    #[borsh(bound(deserialize =
                        "T: PartialOrd + Hash + Eq + borsh::de::BorshDeserialize,
                         U: borsh::de::BorshDeserialize"
                    ))]
                    b: HashMap<T, U>,
                },
                D(u32, u32),
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn check_deserialize_with_attr() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum C<K: Ord, V> {
                C3(u64, u64),
                C4 {
                    x: u64,
                    #[borsh(deserialize_with = "third_party_impl::deserialize_third_party")]
                    y: ThirdParty<K, V>
                },
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn borsh_discriminant_false() {
        let item_enum: ItemEnum = syn::parse2(quote! {
           #[borsh(use_discriminant = false)]
            enum X {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        })
        .unwrap();
        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
    #[test]
    fn borsh_discriminant_true() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[borsh(use_discriminant = true)]
            enum X {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        })
        .unwrap();
        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
    #[test]
    fn borsh_init_func() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[borsh(init = initializonmethod, use_discriminant = true)]
            enum A {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        })
        .unwrap();
        let actual = process(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
