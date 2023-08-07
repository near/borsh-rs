use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, Ident, ItemStruct, Path, WhereClause};

use crate::internals::{
    attributes::{field, item, BoundType},
    deserialize, generics,
};

pub fn process(input: &ItemStruct, cratename: Ident) -> syn::Result<TokenStream2> {
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
    let return_value = match &input.fields {
        Fields::Named(fields) => {
            let mut body = TokenStream2::new();
            for field in &fields.named {
                let skipped = field::contains_skip(&field.attrs);
                let parsed = field::Attributes::parse(&field.attrs, skipped)?;

                override_predicates.extend(parsed.collect_bounds(BoundType::Deserialize));
                let needs_bounds_derive = parsed.needs_bounds_derive(BoundType::Deserialize);

                let field_name = field.ident.as_ref().unwrap();
                let delta = if skipped {
                    if needs_bounds_derive {
                        default_params_visitor.visit_field(field);
                    }
                    quote! {
                        #field_name: core::default::Default::default(),
                    }
                } else {
                    if needs_bounds_derive {
                        deserialize_params_visitor.visit_field(field);
                    }
                    deserialize::field_output(Some(field_name), &cratename, parsed.deserialize_with)
                };
                body.extend(delta);
            }
            quote! {
                Self { #body }
            }
        }
        Fields::Unnamed(fields) => {
            let mut body = TokenStream2::new();
            for (_field_idx, field) in fields.unnamed.iter().enumerate() {
                let skipped = field::contains_skip(&field.attrs);
                let parsed = field::Attributes::parse(&field.attrs, skipped)?;
                override_predicates.extend(parsed.collect_bounds(BoundType::Deserialize));
                let needs_bounds_derive = parsed.needs_bounds_derive(BoundType::Deserialize);

                let delta = if skipped {
                    if needs_bounds_derive {
                        default_params_visitor.visit_field(field);
                    }
                    quote! { core::default::Default::default(), }
                } else {
                    if needs_bounds_derive {
                        deserialize_params_visitor.visit_field(field);
                    }
                    deserialize::field_output(None, &cratename, parsed.deserialize_with)
                };
                body.extend(delta);
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
    if let Some(method_ident) = init_method {
        Ok(quote! {
            impl #impl_generics #cratename::de::BorshDeserialize for #name #ty_generics #where_clause {
                fn deserialize_reader<R: borsh::__private::maybestd::io::Read>(reader: &mut R) -> ::core::result::Result<Self, #cratename::__private::maybestd::io::Error> {
                    let mut return_value = #return_value;
                    return_value.#method_ident();
                    Ok(return_value)
                }
            }
        })
    } else {
        Ok(quote! {
            impl #impl_generics #cratename::de::BorshDeserialize for #name #ty_generics #where_clause {
                fn deserialize_reader<R: borsh::__private::maybestd::io::Read>(reader: &mut R) -> ::core::result::Result<Self, #cratename::__private::maybestd::io::Error> {
                    Ok(#return_value)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::internals::test_helpers::{local_insta_assert_snapshot, pretty_print_syn_str};
    use proc_macro2::Span;

    use super::*;

    #[test]
    fn simple_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn simple_generic_tuple_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct TupleA<T>(T, u32);
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
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

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
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

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_tuple_struct_borsh_skip1() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct G<K, V, U> (
                #[borsh_skip]
                HashMap<K, V>,
                U,
            );
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_tuple_struct_borsh_skip2() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct G<K, V, U> (
                HashMap<K, V>,
                #[borsh_skip]
                U,
            );
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_named_fields_struct_borsh_skip() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct G<K, V, U> {
                #[borsh_skip]
                x: HashMap<K, V>,
                y: U,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_deserialize_bound() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct C<T: Debug, U> {
                a: String,
                #[borsh(bound(deserialize =
                    "T: PartialOrd + Hash + Eq + borsh::de::BorshDeserialize,
                     U: borsh::de::BorshDeserialize"
                ))]
                b: HashMap<T, U>,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn test_override_automatically_added_default_trait() {
        let item_struct: ItemStruct = syn::parse2(quote! {
              struct G1<K, V, U>(
                #[borsh_skip]
                #[borsh(bound(deserialize = ""))]
                HashMap<K, V>,
                U
            );
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn check_deserialize_with_attr() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K: Ord, V> {
                #[borsh(deserialize_with = "third_party_impl::deserialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
