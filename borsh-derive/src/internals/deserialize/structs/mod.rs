use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, ItemStruct, Path};

use crate::internals::{
    attributes::item,
    deserialize::{self, Flavor},
    generics,
};

pub fn process(input: &ItemStruct, cratename: Path) -> syn::Result<TokenStream2> {
    process_flavor(input, cratename, Flavor::Plain)
}

pub fn process_bounded(input: &ItemStruct, cratename: Path) -> syn::Result<TokenStream2> {
    process_flavor(input, cratename, Flavor::Bounded)
}

fn process_flavor(
    input: &ItemStruct,
    cratename: Path,
    flavor: Flavor,
) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = generics::without_defaults(&input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = generics::default_where(where_clause);
    let mut body = TokenStream2::new();
    let mut generics_output = deserialize::GenericsOutput::new(&generics, flavor);

    let return_value = match &input.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                deserialize::process_field(field, &cratename, &mut body, &mut generics_output)?;
            }
            quote! {
                Self { #body }
            }
        }
        Fields::Unnamed(fields) => {
            for field in fields.unnamed.iter() {
                deserialize::process_field(field, &cratename, &mut body, &mut generics_output)?;
            }
            quote! {
                Self( #body )
            }
        }
        Fields::Unit => {
            quote! {
                Self {}
            }
        }
    };
    generics_output.extend(&mut where_clause, &cratename);

    let init = item::contains_initialize_with(&input.attrs)?;
    let budget = deserialize::budget_ident(&generics);
    match (flavor, init) {
        (Flavor::Plain, Some(method_ident)) => Ok(quote! {
            #[automatically_derived]
            impl #impl_generics #cratename::de::BorshDeserialize for #name #ty_generics #where_clause {
                fn deserialize_reader<__R: #cratename::io::Read>(reader: &mut __R) -> ::core::result::Result<Self, #cratename::io::Error> {
                    let mut return_value = #return_value;
                    return_value.#method_ident();
                    Ok(return_value)
                }
            }
        }),
        (Flavor::Plain, None) => Ok(quote! {
            #[automatically_derived]
            impl #impl_generics #cratename::de::BorshDeserialize for #name #ty_generics #where_clause {
                fn deserialize_reader<__R: #cratename::io::Read>(reader: &mut __R) -> ::core::result::Result<Self, #cratename::io::Error> {
                    Ok(#return_value)
                }
            }
        }),
        (Flavor::Bounded, Some(method_ident)) => Ok(quote! {
            #[automatically_derived]
            impl #impl_generics #cratename::de::BorshDeserializeBounded for #name #ty_generics #where_clause {
                fn deserialize_bounded_alloc<__R: #cratename::io::Read>(
                    reader: &mut __R,
                    #budget: &mut #cratename::de::Budget,
                ) -> ::core::result::Result<Self, #cratename::io::Error> {
                    let mut return_value = #return_value;
                    return_value.#method_ident();
                    Ok(return_value)
                }
            }
        }),
        (Flavor::Bounded, None) => Ok(quote! {
            #[automatically_derived]
            impl #impl_generics #cratename::de::BorshDeserializeBounded for #name #ty_generics #where_clause {
                fn deserialize_bounded_alloc<__R: #cratename::io::Read>(
                    reader: &mut __R,
                    #budget: &mut #cratename::de::Budget,
                ) -> ::core::result::Result<Self, #cratename::io::Error> {
                    Ok(#return_value)
                }
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::internals::test_helpers::{
        default_cratename, local_insta_assert_debug_snapshot, local_insta_assert_snapshot,
        pretty_print_syn_str,
    };

    use super::*;

    fn assert_bounded(name: &str, item: &ItemStruct, cratename: Path) {
        let actual = process_bounded(item, cratename).unwrap();
        let actual = pretty_print_syn_str(&actual).unwrap();
        let without_bounded = actual
            .replace("BorshDeserializeBounded", "")
            .replace("EnumExtBounded", "");
        assert!(!without_bounded.contains("BorshDeserialize"), "{}", actual);
        assert!(!without_bounded.contains("EnumExt"), "{}", actual);
        assert!(!actual.contains("deserialize_reader"), "{}", actual);
        local_insta_assert_snapshot!(format!("bounded_{}", name), actual);
    }

    #[test]
    fn simple_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
        assert_bounded("simple_struct", &item_struct, default_cratename());
    }

    #[test]
    fn simple_struct_with_custom_crate() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let crate_: Path = syn::parse2(quote! { reexporter::borsh }).unwrap();
        let actual = process(&item_struct, crate_).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
        assert_bounded(
            "simple_struct_with_custom_crate",
            &item_struct,
            syn::parse2(quote! { reexporter::borsh }).unwrap(),
        );
    }

    #[test]
    fn simple_generics() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K, V> {
                x: HashMap<K, V>,
                y: String,
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn simple_generic_tuple_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct TupleA<T>(T, u32);
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn bound_generics() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K: Key, V> where V: Value {
                x: HashMap<K, V>,
                y: String,
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn recursive_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct CRecC {
                a: String,
                b: HashMap<String, CRecC>,
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_tuple_struct_borsh_skip1() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct G<K, V, U> (
                #[borsh(skip)]
                HashMap<K, V>,
                U,
            );
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_tuple_struct_borsh_skip2() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct G<K, V, U> (
                HashMap<K, V>,
                #[borsh(skip)]
                U,
            );
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_named_fields_struct_borsh_skip() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct G<K, V, U> {
                #[borsh(skip)]
                x: HashMap<K, V>,
                y: U,
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_deserialize_bound() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct C<T: Debug, U> {
                a: String,
                #[borsh(bound(deserialize =
                    "T: PartialOrd + Hash + Eq + borsh::de::BorshDeserialize,
                     U: borsh::de::BorshDeserialize",
                    deserialize_bounded =
                    "T: PartialOrd + Hash + Eq + borsh::de::BorshDeserializeBounded,
                     U: borsh::de::BorshDeserializeBounded"
                ))]
                b: HashMap<T, U>,
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
        assert_bounded(
            "generic_deserialize_bound",
            &item_struct,
            default_cratename(),
        );
    }

    #[test]
    fn test_override_automatically_added_default_trait() {
        let item_struct: ItemStruct = syn::parse2(quote! {
              struct G1<K, V, U>(
                #[borsh(skip,bound(deserialize = "", deserialize_bounded = ""))]
                HashMap<K, V>,
                U
            );
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn check_deserialize_with_attr() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K: Ord, V> {
                #[borsh(
                    deserialize_with = "third_party_impl::deserialize_third_party",
                    deserialize_with_bounded = "third_party_impl::deserialize_third_party_bounded",
                )]
                x: ThirdParty<K, V>,
                y: u64,
            }
        })
        .unwrap();

        let actual = process(&item_struct, default_cratename()).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
        assert_bounded(
            "check_deserialize_with_attr",
            &item_struct,
            default_cratename(),
        );
    }
    #[test]
    fn borsh_init_func() {
        let item_enum: ItemStruct = syn::parse2(quote! {
            #[borsh(init=initialization_method)]
            struct A {
                x: u64,
                y: String,
            }
        })
        .unwrap();
        let actual = process(&item_enum, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
        assert_bounded("borsh_init_func", &item_enum, default_cratename());
    }

    #[test]
    fn bounded_rejects_deserialize_with_alone() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K, V> {
                #[borsh(deserialize_with = "third_party_impl::deserialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        })
        .unwrap();

        // Plain decoding accepts it; only the bounded derive needs the counterpart.
        process(&item_struct, default_cratename()).unwrap();
        let err = match process_bounded(&item_struct, default_cratename()) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    fn plain_output_ignores_bounded_keys() {
        let with_keys: ItemStruct = syn::parse2(quote! {
            struct A<K: Ord, V, T, U> {
                #[borsh(
                    deserialize_with = "third_party_impl::deserialize_third_party",
                    deserialize_with_bounded = "third_party_impl::deserialize_third_party_bounded",
                    serialize_with = "third_party_impl::serialize_third_party",
                )]
                x: ThirdParty<K, V>,
                #[borsh(bound(deserialize = "T: Ord", deserialize_bounded = "T: Eq"))]
                y: BTreeMap<T, U>,
            }
        })
        .unwrap();
        let without_keys: ItemStruct = syn::parse2(quote! {
            struct A<K: Ord, V, T, U> {
                #[borsh(
                    deserialize_with = "third_party_impl::deserialize_third_party",
                    serialize_with = "third_party_impl::serialize_third_party",
                )]
                x: ThirdParty<K, V>,
                #[borsh(bound(deserialize = "T: Ord"))]
                y: BTreeMap<T, U>,
            }
        })
        .unwrap();

        assert_eq!(
            process(&with_keys, default_cratename())
                .unwrap()
                .to_string(),
            process(&without_keys, default_cratename())
                .unwrap()
                .to_string()
        );
        assert_eq!(
            crate::internals::serialize::structs::process(&with_keys, default_cratename())
                .unwrap()
                .to_string(),
            crate::internals::serialize::structs::process(&without_keys, default_cratename())
                .unwrap()
                .to_string()
        );
    }

    #[test]
    fn bounded_budget_avoids_generic_names() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<__budget, const __budget_: usize> {
                x: __budget,
                y: [u8; __budget_],
            }
        })
        .unwrap();

        let actual = process_bounded(&item_struct, default_cratename())
            .unwrap()
            .to_string();
        assert!(actual.contains("__budget__ : & mut"), "{}", actual);

        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<const r#__budget: usize>([u8; r#__budget]);
        })
        .unwrap();
        let actual = process_bounded(&item_struct, default_cratename())
            .unwrap()
            .to_string();
        assert!(actual.contains("__budget_ : & mut"), "{}", actual);
    }
}
