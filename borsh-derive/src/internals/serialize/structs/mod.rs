use cfg_if::cfg_if;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{Fields, ItemStruct, Lifetime, Path, Token};

use crate::internals::{
    attributes::{field, BoundType},
    generics, serialize,
};

pub fn process<const IS_ASYNC: bool>(
    input: ItemStruct,
    cratename: Path,
) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let generics = generics::without_defaults(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = generics::default_where(where_clause);
    let mut body = TokenStream2::new();
    let mut generics_output = serialize::GenericsOutput::new(&generics);
    match &input.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                let field_id = serialize::FieldId::Struct(field.ident.clone().unwrap());

                process_field::<IS_ASYNC>(
                    field,
                    field_id,
                    &cratename,
                    &mut generics_output,
                    &mut body,
                )?;
            }
        }
        Fields::Unnamed(fields) => {
            for (field_idx, field) in fields.unnamed.iter().enumerate() {
                let field_id = serialize::FieldId::new_struct_unnamed(field_idx)?;

                process_field::<IS_ASYNC>(
                    field,
                    field_id,
                    &cratename,
                    &mut generics_output,
                    &mut body,
                )?;
            }
        }
        Fields::Unit => {}
    }
    generics_output.extend::<IS_ASYNC>(&mut where_clause, &cratename);

    let serialize_trait = Ident::new(
        if IS_ASYNC {
            "BorshSerializeAsync"
        } else {
            "BorshSerialize"
        },
        Span::call_site(),
    );
    let writer_trait_path = if IS_ASYNC {
        quote! { async_io::AsyncWrite }
    } else {
        quote! { io::Write }
    };
    let r#async = IS_ASYNC.then(|| Token![async](Span::call_site()));
    let lifetime = IS_ASYNC.then(|| Lifetime::new("'async_variant", Span::call_site()));
    let lt_comma = IS_ASYNC.then(|| Token![,](Span::call_site()));

    Ok(quote! {
        impl #impl_generics #cratename::ser::#serialize_trait for #name #ty_generics #where_clause {
            #r#async fn serialize<#lifetime #lt_comma __W: #cratename::#writer_trait_path>(
                &#lifetime self,
                writer: &#lifetime mut __W,
            ) -> ::core::result::Result<(), #cratename::io::Error> {
                #body
                ::core::result::Result::Ok(())
            }
        }
    })
}

fn process_field<const IS_ASYNC: bool>(
    field: &syn::Field,
    field_id: serialize::FieldId,
    cratename: &Path,
    generics: &mut serialize::GenericsOutput,
    body: &mut TokenStream2,
) -> syn::Result<()> {
    let parsed = field::Attributes::parse(&field.attrs)?;

    let needs_bounds_derive = parsed.needs_bounds_derive::<IS_ASYNC>(BoundType::Serialize);
    generics
        .overrides
        .extend(parsed.collect_bounds::<IS_ASYNC>(BoundType::Serialize));

    if !parsed.skip {
        let delta = field_id.serialize_output::<IS_ASYNC>(
            &field.ty,
            cratename,
            if IS_ASYNC {
                cfg_if! {
                    if #[cfg(feature = "async")] {
                        parsed.serialize_with_async
                    } else {
                        None
                    }
                }
            } else {
                parsed.serialize_with
            },
        );
        body.extend(delta);

        if needs_bounds_derive {
            generics.serialize_visitor.visit_field(field);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;
    use crate::internals::test_helpers::{
        default_cratename, local_insta_assert_debug_snapshot, local_insta_assert_snapshot,
        pretty_print_syn_str,
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

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
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

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, crate_).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
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

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
    }

    #[test]
    fn simple_generic_tuple_struct() {
        let item_struct: ItemStruct = parse_quote! {
            struct TupleA<T>(T, u32);
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
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

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
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

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
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

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
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

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
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

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
    }

    #[test]
    fn generic_associated_type() {
        let item_struct: ItemStruct = parse_quote! {
            struct Parametrized<T, V>
            where
                T: TraitName,
            {
                field: T::Associated,
                another: V,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
    }

    #[test]
    fn generic_serialize_bound() {
        let item_struct: ItemStruct = parse_quote! {
            struct C<T: Debug, U> {
                a: String,
                #[borsh(bound(serialize =
                    "T: borsh::ser::BorshSerialize + PartialOrd,
                     U: borsh::ser::BorshSerialize"
                ))]
                b: HashMap<T, U>,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
    }

    #[test]
    #[cfg(feature = "async")]
    fn generic_serialize_async_bound() {
        let item_struct: ItemStruct = parse_quote! {
            struct C<T: Debug, U> {
                a: String,
                #[borsh(async_bound(serialize =
                    "T: borsh::ser::BorshSerializeAsync + PartialOrd,
                     U: borsh::ser::BorshSerializeAsync"
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
    fn override_generic_associated_type_wrong_derive() {
        let item_struct: ItemStruct = parse_quote! {
            struct Parametrized<T, V> where T: TraitName {
                #[borsh(bound(serialize =
                    "<T as TraitName>::Associated: borsh::ser::BorshSerialize"
                ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
    }

    #[test]
    #[cfg(feature = "async")]
    fn async_override_generic_associated_type_wrong_derive() {
        let item_struct: ItemStruct = parse_quote! {
            struct Parametrized<T, V> where T: TraitName {
                #[borsh(async_bound(serialize =
                    "<T as TraitName>::Associated: borsh::ser::BorshSerializeAsync"
                ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        let actual = process::<true>(item_struct, default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
    }

    #[test]
    fn check_serialize_with_attr() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K: Ord, V> {
                #[borsh(serialize_with = "third_party_impl::serialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename()).unwrap();
        local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());

        #[cfg(feature = "async")]
        {
            let actual = process::<true>(item_struct, default_cratename()).unwrap();
            local_insta_assert_snapshot!(pretty_print_syn_str(actual).unwrap());
        }
    }

    #[test]
    #[cfg(feature = "async")]
    fn check_serialize_with_async_attr() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K: Ord, V> {
                #[borsh(serialize_with_async = "third_party_impl::serialize_third_party")]
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
    #[cfg(not(feature = "async"))]
    fn check_serialize_with_skip_conflict() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K: Ord, V> {
                #[borsh(skip, serialize_with = "third_party_impl::serialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename());

        let err = match actual {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    #[cfg(feature = "async")]
    fn check_serialize_with_skip_conflict_feature_async() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K: Ord, V> {
                #[borsh(skip, serialize_with = "third_party_impl::serialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename());

        let err = match actual {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);

        let actual = process::<true>(item_struct, default_cratename());

        let err = match actual {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    #[cfg(feature = "async")]
    fn check_serialize_with_async_skip_conflict() {
        let item_struct: ItemStruct = parse_quote! {
            struct A<K: Ord, V> {
                #[borsh(skip, serialize_with_async = "third_party_impl::serialize_third_party")]
                x: ThirdParty<K, V>,
                y: u64,
            }
        };

        let actual = process::<false>(item_struct.clone(), default_cratename());

        let err = match actual {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);

        let actual = process::<true>(item_struct, default_cratename());

        let err = match actual {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }
}
