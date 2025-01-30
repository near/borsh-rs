use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Fields, ItemEnum, Path, Token, Variant};

use crate::internals::{attributes::item, deserialize, enum_discriminant::Discriminants, generics};

pub fn process<const IS_ASYNC: bool>(
    input: ItemEnum,
    cratename: Path,
) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let use_discriminant = item::contains_use_discriminant(&input)?;
    let generics = generics::without_defaults(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = generics::default_where(where_clause);
    let mut variant_arms = TokenStream2::new();
    let discriminants = Discriminants::new(&input.variants);
    let mut generics_output = deserialize::GenericsOutput::new(&generics);

    for (variant_idx, variant) in input.variants.into_iter().enumerate() {
        let variant_body = process_variant::<IS_ASYNC>(&variant, &cratename, &mut generics_output)?;
        let variant_ident = variant.ident;

        let discriminant_value =
            discriminants.get(&variant_ident, use_discriminant, variant_idx)?;

        // `if` branches are used instead of `match` branches, because `discriminant_value` might be a function call
        variant_arms.extend(quote! {
            if variant_tag == #discriminant_value { #name::#variant_ident #variant_body } else
        });
    }
    let init = item::contains_initialize_with(&input.attrs)?
        .map(|method_ident| quote! { return_value.#method_ident(); });
    let r#mut = init.is_some().then(|| Token![mut](Span::call_site()));
    generics_output.extend::<IS_ASYNC>(&mut where_clause, &cratename);

    let deserialize_trait = Ident::new(
        if IS_ASYNC {
            "BorshDeserializeAsync"
        } else {
            "BorshDeserialize"
        },
        Span::call_site(),
    );
    let enum_ext = Ident::new(
        if IS_ASYNC { "EnumExtAsync" } else { "EnumExt" },
        Span::call_site(),
    );
    let read_trait_path = if IS_ASYNC {
        quote! { async_io::AsyncRead }
    } else {
        quote! { io::Read }
    };
    let r#async = IS_ASYNC.then(|| Token![async](Span::call_site()));
    let dot_await = IS_ASYNC.then(|| quote! { .await });

    Ok(quote! {
        impl #impl_generics #cratename::de::#deserialize_trait for #name #ty_generics #where_clause {
            #r#async fn deserialize_reader<__R: #cratename::#read_trait_path>(reader: &mut __R) -> ::core::result::Result<Self, #cratename::io::Error> {
                let tag = <u8 as #cratename::de::#deserialize_trait>::deserialize_reader(reader)#dot_await?;
                <Self as #cratename::de::#enum_ext>::deserialize_variant(reader, tag)#dot_await
            }
        }

        impl #impl_generics #cratename::de::#enum_ext for #name #ty_generics #where_clause {
            #r#async fn deserialize_variant<__R: #cratename::#read_trait_path>(
                reader: &mut __R,
                variant_tag: u8,
            ) -> ::core::result::Result<Self, #cratename::io::Error> {
                let #r#mut return_value =
                    #variant_arms {
                    return ::core::result::Result::Err(#cratename::io::Error::new(
                        #cratename::io::ErrorKind::InvalidData,
                        #cratename::__private::maybestd::format!("Unexpected variant tag: {:?}", variant_tag),
                    ))
                };
                #init
                ::core::result::Result::Ok(return_value)
            }
        }
    })
}

fn process_variant<const IS_ASYNC: bool>(
    variant: &Variant,
    cratename: &Path,
    generics: &mut deserialize::GenericsOutput,
) -> syn::Result<TokenStream2> {
    let mut body = TokenStream2::new();
    match &variant.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                deserialize::process_field::<IS_ASYNC>(field, cratename, &mut body, generics)?;
            }
            body = quote! { { #body }};
        }
        Fields::Unnamed(fields) => {
            for field in fields.unnamed.iter() {
                deserialize::process_field::<IS_ASYNC>(field, cratename, &mut body, generics)?;
            }
            body = quote! { ( #body )};
        }
        Fields::Unit => {}
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;
    use crate::internals::test_helpers::{
        default_cratename, local_insta_assert_snapshot, pretty_print_syn_str,
    };

    #[test]
    fn borsh_skip_struct_variant_field() {
        let item_enum: ItemEnum = parse_quote! {
            enum AA {
                B {
                    #[borsh(skip)]
                    c: i32,
                    d: u32,
                },
                NegatedVariant {
                    beta: u8,
                }
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn borsh_skip_tuple_variant_field() {
        let item_enum: ItemEnum = parse_quote! {
            enum AAT {
                B(#[borsh(skip)] i32, u32),

                NegatedVariant {
                    beta: u8,
                }
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn simple_enum_with_custom_crate() {
        let item_enum: ItemEnum = parse_quote! {
            enum A {
                B {
                    x: HashMap<u32, String>,
                    y: String,
                },
                C(K, Vec<u64>),
            }
        };

        let crate_: Path = parse_quote! { reexporter::borsh };

        let actual = process::<false>(item_enum.clone(), crate_.clone()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, crate_).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn simple_generics() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K, V, U> {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn bound_generics() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn recursive_enum() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K: Key, V> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<A>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }
    #[test]
    fn generic_borsh_skip_struct_field() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    #[borsh(skip)]
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, Vec<U>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_borsh_skip_tuple_field() {
        let item_enum: ItemEnum = parse_quote! {
            enum A<K: Key, V, U> where V: Value {
                B {
                    x: HashMap<K, V>,
                    y: String,
                },
                C(K, #[borsh(skip)] Vec<U>),
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_deserialize_bound() {
        let item_enum: ItemEnum = parse_quote! {
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
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn check_deserialize_with_attr() {
        let item_enum: ItemEnum = parse_quote! {
            enum C<K: Ord, V> {
                C3(u64, u64),
                C4 {
                    x: u64,
                    #[borsh(deserialize_with = "third_party_impl::deserialize_third_party")]
                    y: ThirdParty<K, V>
                },
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn borsh_discriminant_false() {
        let item_enum: ItemEnum = parse_quote! {
           #[borsh(use_discriminant = false)]
            enum X {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }
    #[test]
    fn borsh_discriminant_true() {
        let item_enum: ItemEnum = parse_quote! {
            #[borsh(use_discriminant = true)]
            enum X {
                A,
                B = 20,
                C,
                D,
                E = 10,
                F,
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }
    #[test]
    fn borsh_init_func() {
        let item_enum: ItemEnum = parse_quote! {
            #[borsh(init = initialization_method)]
            enum A {
                A,
                B,
                C,
                D,
                E,
                F,
            }
        };

        let actual = process::<false>(item_enum.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }
}
