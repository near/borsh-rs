use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, Ident, ItemEnum, Variant};

use crate::internals::{attributes::item, deserialize, enum_discriminant::Discriminants, generics};

pub fn process(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = generics::without_defaults(&input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = generics::default_where(where_clause);
    let mut variant_arms = TokenStream2::new();
    let use_discriminant = item::contains_use_discriminant(input)?;
    let discriminants = Discriminants::new(&input.variants);
    let mut generics_output = deserialize::GenericsOutput::new(&generics);

    for (variant_idx, variant) in input.variants.iter().enumerate() {
        let variant_body = process_variant(variant, &cratename, &mut generics_output)?;
        let variant_ident = &variant.ident;

        let discriminant_value = discriminants.get(variant_ident, use_discriminant, variant_idx)?;
        variant_arms.extend(quote! {
            if variant_tag == #discriminant_value { #name::#variant_ident #variant_body } else
        });
    }
    let init = if let Some(method_ident) = item::contains_initialize_with(&input.attrs) {
        quote! {
            return_value.#method_ident();
        }
    } else {
        quote! {}
    };
    generics_output.extend(&mut where_clause, &cratename);

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

fn process_variant(
    variant: &Variant,
    cratename: &Ident,
    generics: &mut deserialize::GenericsOutput,
) -> syn::Result<TokenStream2> {
    let mut body = TokenStream2::new();
    match &variant.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                deserialize::process_field(field, cratename, &mut body, generics)?;
            }
            body = quote! { { #body }};
        }
        Fields::Unnamed(fields) => {
            for field in fields.unnamed.iter() {
                deserialize::process_field(field, cratename, &mut body, generics)?;
            }
            body = quote! { ( #body )};
        }
        Fields::Unit => {}
    }
    Ok(body)
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
}
