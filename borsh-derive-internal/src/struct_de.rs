use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Fields, Ident, ItemStruct, Path, WhereClause};

use crate::{
    attribute_helpers::{contains_initialize_with, contains_skip},
    generics::{compute_predicates, without_defaults, FindTyParams},
};

pub fn struct_de(input: &ItemStruct, cratename: Ident) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut where_clause = where_clause.map_or_else(
        || WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
        Clone::clone,
    );

    let generics = without_defaults(&input.generics);
    let mut deserialize_params_visitor = FindTyParams::new(&generics);
    let mut default_params_visitor = FindTyParams::new(&generics);

    let init_method = contains_initialize_with(&input.attrs);
    let return_value = match &input.fields {
        Fields::Named(fields) => {
            let mut body = TokenStream2::new();
            for field in &fields.named {
                let field_name = field.ident.as_ref().unwrap();
                let delta = if contains_skip(&field.attrs) {
                    default_params_visitor.visit_field(field);
                    quote! {
                        #field_name: core::default::Default::default(),
                    }
                } else {
                    deserialize_params_visitor.visit_field(field);
                    quote! {
                        #field_name: #cratename::BorshDeserialize::deserialize_reader(reader)?,
                    }
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
                let delta = if contains_skip(&field.attrs) {
                    default_params_visitor.visit_field(field);
                    quote! { core::default::Default::default(), }
                } else {
                    deserialize_params_visitor.visit_field(field);
                    quote! {
                        #cratename::BorshDeserialize::deserialize_reader(reader)?,
                    }
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
    let de_predicates = compute_predicates(deserialize_params_visitor.process(), &de_trait_path);
    let default_predicates =
        compute_predicates(default_params_visitor.process(), &default_trait_path);
    where_clause.predicates.extend(de_predicates);
    where_clause.predicates.extend(default_predicates);
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
    use crate::test_helpers::pretty_print_syn_str;
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

        let actual = struct_de(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = struct_de(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn simple_generic_tuple_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct TupleA<T>(T, u32);
        })
        .unwrap();

        let actual = struct_de(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = struct_de(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = struct_de(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = struct_de(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = struct_de(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = struct_de(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
