use core::convert::TryFrom;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Expr, Fields, Ident, Index, ItemStruct, Path, WhereClause};

use crate::internals::{
    attributes::{field, BoundType},
    generics, serialize,
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
    let mut serialize_params_visitor = generics::FindTyParams::new(&generics);
    let mut body = TokenStream2::new();
    match &input.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                let skipped = field::contains_skip(&field.attrs);
                let parsed = field::Attributes::parse(&field.attrs, skipped)?;
                let needs_bounds_derive = parsed.needs_bounds_derive(BoundType::Serialize);
                override_predicates.extend(parsed.collect_bounds(BoundType::Serialize));
                if skipped {
                    continue;
                }
                let field_name = field.ident.as_ref().unwrap();

                let arg: Expr = syn::parse2(quote! { &self.#field_name }).unwrap();
                let delta = serialize::field_output(&arg, &cratename, parsed.serialize_with);

                body.extend(delta);
                if needs_bounds_derive {
                    serialize_params_visitor.visit_field(field);
                }
            }
        }
        Fields::Unnamed(fields) => {
            for (field_idx, field) in fields.unnamed.iter().enumerate() {
                let skipped = field::contains_skip(&field.attrs);
                let parsed = field::Attributes::parse(&field.attrs, skipped)?;
                let needs_bounds_derive = parsed.needs_bounds_derive(BoundType::Serialize);
                override_predicates.extend(parsed.collect_bounds(BoundType::Serialize));
                if !skipped {
                    let field_idx = Index {
                        index: u32::try_from(field_idx).expect("up to 2^32 fields are supported"),
                        span: Span::call_site(),
                    };
                    let arg: Expr = syn::parse2(quote! { &self.#field_idx }).unwrap();
                    let delta = serialize::field_output(&arg, &cratename, parsed.serialize_with);
                    body.extend(delta);
                    if needs_bounds_derive {
                        serialize_params_visitor.visit_field(field);
                    }
                }
            }
        }
        Fields::Unit => {}
    }
    let trait_path: Path = syn::parse2(quote! { #cratename::ser::BorshSerialize }).unwrap();
    let predicates =
        generics::compute_predicates(serialize_params_visitor.process_for_bounds(), &trait_path);
    where_clause.predicates.extend(predicates);
    where_clause.predicates.extend(override_predicates);
    Ok(quote! {
        impl #impl_generics #cratename::ser::BorshSerialize for #name #ty_generics #where_clause {
            fn serialize<W: #cratename::__private::maybestd::io::Write>(&self, writer: &mut W) -> ::core::result::Result<(), #cratename::__private::maybestd::io::Error> {
                #body
                Ok(())
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::internals::test_helpers::{
        local_insta_assert_debug_snapshot, local_insta_assert_snapshot, pretty_print_syn_str,
    };

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
    fn generic_associated_type() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<T, V>
            where
                T: TraitName,
            {
                field: T::Associated,
                another: V,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_serialize_bound() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct C<T: Debug, U> {
                a: String,
                #[borsh(bound(serialize =
                    "T: borsh::ser::BorshSerialize + PartialOrd,
                     U: borsh::ser::BorshSerialize"
                ))]
                b: HashMap<T, U>,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn override_generic_associated_type_wrong_derive() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<T, V> where T: TraitName {
                #[borsh(bound(serialize =
                    "<T as TraitName>::Associated: borsh::ser::BorshSerialize"
                ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn check_serialize_with_attr() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K: Ord, V> {
                #[borsh(serialize_with = "third_party_impl::serialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        local_insta_assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn check_serialize_with_skip_conflict() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K: Ord, V> {
                #[borsh_skip]
                #[borsh(serialize_with = "third_party_impl::serialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        })
        .unwrap();

        let actual = process(&item_struct, Ident::new("borsh", Span::call_site()));

        let err = match actual {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }
}
