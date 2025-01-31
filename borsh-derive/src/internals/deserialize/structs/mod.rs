use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Fields, ItemStruct, Path, Token};

use crate::internals::{attributes::item, deserialize, generics};

pub fn process<const IS_ASYNC: bool>(
    input: ItemStruct,
    cratename: Path,
) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = generics::without_defaults(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = generics::default_where(where_clause);
    let mut body = TokenStream2::new();
    let mut generics_output = deserialize::GenericsOutput::new(&generics);

    let return_value = match &input.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                deserialize::process_field::<IS_ASYNC>(
                    field,
                    &cratename,
                    &mut body,
                    &mut generics_output,
                )?;
            }
            quote! { Self { #body } }
        }
        Fields::Unnamed(fields) => {
            for field in fields.unnamed.iter() {
                deserialize::process_field::<IS_ASYNC>(
                    field,
                    &cratename,
                    &mut body,
                    &mut generics_output,
                )?;
            }
            quote! { Self( #body ) }
        }
        Fields::Unit => quote! { Self {} },
    };
    generics_output.extend::<IS_ASYNC>(&mut where_clause, &cratename);

    let deserialize_trait = Ident::new(
        if IS_ASYNC {
            "BorshDeserializeAsync"
        } else {
            "BorshDeserialize"
        },
        Span::call_site(),
    );
    let read_trait_path = if IS_ASYNC {
        quote! { async_io::AsyncRead }
    } else {
        quote! { io::Read }
    };
    let r#async = IS_ASYNC.then(|| Token![async](Span::call_site()));

    let body = if let Some(method_ident) = item::contains_initialize_with(&input.attrs)? {
        quote! {
            let mut return_value = #return_value;
            return_value.#method_ident();
            ::core::result::Result::Ok(return_value)
        }
    } else {
        quote! { ::core::result::Result::Ok(#return_value) }
    };

    Ok(quote! {
        impl #impl_generics #cratename::de::#deserialize_trait for #name #ty_generics #where_clause {
            #r#async fn deserialize_reader<__R: #cratename::#read_trait_path>(reader: &mut __R) -> ::core::result::Result<Self, #cratename::io::Error> {
                #body
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;
    use crate::internals::test_helpers::{
        default_cratename, local_insta_assert_snapshot, pretty_print_syn_str,
    };

    #[test]
    fn simple_struct() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                x: u64,
                y: String,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn simple_struct_with_custom_crate() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                x: u64,
                y: String,
            }
        };

        let crate_: Path = parse_quote! { reexporter::borsh };

        let actual = process::<false>(item_struct.clone(), crate_.clone()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, crate_).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn simple_generics() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K, V> {
                x: HashMap<K, V>,
                y: String,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn simple_generic_tuple_struct() {
        let item_struct: ItemStruct = parse_quote! {
            struct TupleA<T>(T, u32);
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn bound_generics() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K: Key, V> where V: Value {
                x: HashMap<K, V>,
                y: String,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn recursive_struct() {
        let item_struct: ItemStruct = parse_quote! {
            struct CRecC {
                a: String,
                b: HashMap<String, CRecC>,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_tuple_struct_borsh_skip1() {
        let item_struct: ItemStruct = parse_quote! {
            struct G<K, V, U> (
                #[borsh(skip)]
                HashMap<K, V>,
                U,
            );
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_tuple_struct_borsh_skip2() {
        let item_struct: ItemStruct = parse_quote! {
            struct G<K, V, U> (
                HashMap<K, V>,
                #[borsh(skip)]
                U,
            );
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_named_fields_struct_borsh_skip() {
        let item_struct: ItemStruct = parse_quote! {
            struct G<K, V, U> {
                #[borsh(skip)]
                x: HashMap<K, V>,
                y: U,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_deserialize_bound() {
        let item_struct: ItemStruct = parse_quote! {
            struct C<T: Debug, U> {
                a: String,
                #[borsh(bound(deserialize =
                    "T: PartialOrd + Hash + Eq + borsh::de::BorshDeserialize,
                     U: borsh::de::BorshDeserialize"
                ))]
                b: HashMap<T, U>,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn generic_deserialize_async_bound() {
        let item_struct: ItemStruct = parse_quote! {
            struct C<T: Debug, U> {
                a: String,
                #[borsh(async_bound(deserialize =
                    "T: PartialOrd + Hash + Eq + borsh::de::BorshDeserializeAsync,
                     U: borsh::de::BorshDeserializeAsync"
                ))]
                b: HashMap<T, U>,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn test_override_automatically_added_default_trait() {
        let item_struct: ItemStruct = parse_quote! {
              struct G1<K, V, U>(
                #[borsh(skip,bound(deserialize = ""))]
                HashMap<K, V>,
                U
            );
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn test_override_automatically_added_default_trait_async() {
        let item_struct: ItemStruct = parse_quote! {
              struct G1<K, V, U>(
                #[borsh(skip,async_bound(deserialize = ""))]
                HashMap<K, V>,
                U
            );
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn check_deserialize_with_attr() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K: Ord, V> {
                #[borsh(deserialize_with = "third_party_impl::deserialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn check_deserialize_with_async_attr() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K: Ord, V> {
                #[borsh(deserialize_with_async = "third_party_impl::deserialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn borsh_init_func() {
        let item_struct: ItemStruct = parse_quote! {
            #[borsh(init=initialization_method)]
            struct A {
                x: u64,
                y: String,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }
}
