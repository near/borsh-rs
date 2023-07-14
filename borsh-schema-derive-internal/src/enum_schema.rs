use std::collections::HashSet;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{Fields, Ident, ItemEnum, ItemStruct, Path, Visibility, WhereClause};

use crate::{
    generics::{compute_predicates, without_defaults, FindTyParams},
    schema_helpers::{declaration, filter_field_attrs, filter_used_params},
    struct_schema::{visit_struct_fields, visit_struct_fields_unconditional},
};

fn transform_variant_fields(mut input: Fields) -> Fields {
    match input {
        Fields::Named(ref mut named) => {
            for field in &mut named.named {
                let field_attrs = filter_field_attrs(field.attrs.drain(..)).collect::<Vec<_>>();
                field.attrs = field_attrs;
            }
        }
        Fields::Unnamed(ref mut unnamed) => {
            for field in &mut unnamed.unnamed {
                let field_attrs = filter_field_attrs(field.attrs.drain(..)).collect::<Vec<_>>();
                field.attrs = field_attrs;
            }
        }
        _ => {}
    }
    input
}

pub fn process_enum(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let name_str = name.to_token_stream().to_string();
    let generics = without_defaults(&input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut where_clause = where_clause.map_or_else(
        || WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        },
        Clone::clone,
    );
    let mut enum_schema_params_visitor = FindTyParams::new(&generics);

    // Generate function that returns the schema for variants.
    // Definitions of the variants.
    let mut variants_defs = vec![];
    // Definitions of the inner structs used in variants.
    let mut inner_defs = TokenStream2::new();
    // Recursive calls to `add_definitions_recursively`.
    let mut add_recursive_defs = TokenStream2::new();
    for variant in &input.variants {
        let variant_name_str = variant.ident.to_token_stream().to_string();
        let full_variant_name_str = format!("{}{}", name_str, variant_name_str);
        let full_variant_ident = Ident::new(full_variant_name_str.as_str(), Span::call_site());
        let transformed_fields = transform_variant_fields(variant.fields.clone());

        let mut enum_variant_schema_params_visitor = FindTyParams::new(&generics);
        visit_struct_fields(&variant.fields, &mut enum_schema_params_visitor);
        visit_struct_fields_unconditional(&variant.fields, &mut enum_variant_schema_params_visitor);

        let variant_not_skipped_params = enum_variant_schema_params_visitor
            .process_for_params()
            .into_iter()
            .collect::<HashSet<_>>();
        let inner_struct_generics = filter_used_params(&generics, variant_not_skipped_params);

        let (_impl_generics, inner_struct_ty_generics, _where_clause) =
            inner_struct_generics.split_for_impl();
        let inner_struct = ItemStruct {
            attrs: vec![],
            vis: Visibility::Inherited,
            struct_token: Default::default(),
            ident: full_variant_ident.clone(),
            generics: inner_struct_generics.clone(),
            fields: transformed_fields,
            semi_token: Some(Default::default()),
        };

        inner_defs.extend(quote! {
            #[allow(dead_code)]
            #[derive(#cratename::BorshSchema)]
            #inner_struct
        });
        add_recursive_defs.extend(quote! {
            <#full_variant_ident #inner_struct_ty_generics as #cratename::BorshSchema>::add_definitions_recursively(definitions);
        });
        variants_defs.push(quote! {
            (#variant_name_str.to_string(), <#full_variant_ident #inner_struct_ty_generics>::declaration())
        });
    }

    let type_definitions = quote! {
        fn add_definitions_recursively(definitions: &mut #cratename::__private::maybestd::collections::BTreeMap<#cratename::schema::Declaration, #cratename::schema::Definition>) {
            #inner_defs
            #add_recursive_defs
            let variants = #cratename::__private::maybestd::vec![#(#variants_defs),*];
            let definition = #cratename::schema::Definition::Enum{variants};
            Self::add_definition(Self::declaration(), definition, definitions);
        }
    };
    let trait_path: Path = syn::parse2(quote! { #cratename::BorshSchema }).unwrap();
    let predicates = compute_predicates(
        enum_schema_params_visitor.clone().process_for_bounds(),
        &trait_path,
    );
    where_clause.predicates.extend(predicates);

    let declaration = declaration(
        &name_str,
        cratename.clone(),
        enum_schema_params_visitor.process_for_bounds(),
    );
    Ok(quote! {
        impl #impl_generics #cratename::BorshSchema for #name #ty_generics #where_clause {
            fn declaration() -> #cratename::schema::Declaration {
                #declaration
            }
            #type_definitions
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::pretty_print_syn_str;

    use super::*;

    #[test]
    fn simple_enum() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum A {
                Bacon,
                Eggs
            }
        })
        .unwrap();

        let actual = process_enum(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn single_field_enum() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum A {
                Bacon,
            }
        })
        .unwrap();

        let actual = process_enum(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn complex_enum() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum A {
                Bacon,
                Eggs,
                Salad(Tomatoes, Cucumber, Oil),
                Sausage{wrapper: Wrapper, filling: Filling},
            }
        })
        .unwrap();

        let actual = process_enum(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn complex_enum_generics() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum A<C, W> {
                Bacon,
                Eggs,
                Salad(Tomatoes, C, Oil),
                Sausage{wrapper: W, filling: Filling},
            }
        })
        .unwrap();

        let actual = process_enum(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn trailing_comma_generics() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum Side<B, A>
            where
                A: Display + Debug,
                B: Display + Debug,
            {
                Left(A),
                Right(B),
            }
        })
        .unwrap();

        let actual = process_enum(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn test_filter_foreign_attrs() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum A {
                #[serde(rename = "ab")]
                B {
                    #[serde(rename = "abc")]
                    c: i32,
                    #[borsh_skip]
                    d: u32,
                    l: u64,
                },
                Negative {
                    beta: String,
                }
            }
        })
        .unwrap();

        let actual = process_enum(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn complex_enum_generics_borsh_skip_tuple_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum A<C, W> {
                Bacon,
                Eggs,
                Salad(Tomatoes, #[borsh_skip] C, Oil),
                Sausage{wrapper: W, filling: Filling},
            }
        })
        .unwrap();

        let actual = process_enum(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn complex_enum_generics_borsh_skip_named_field() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            enum A<W, U, C> {
                Bacon,
                Eggs,
                Salad(Tomatoes, C, Oil),
                Sausage{
                    #[borsh_skip]
                    wrapper: W,
                    filling: Filling,
                    unexpected: U,
                },
            }
        })
        .unwrap();

        let actual = process_enum(&item_enum, Ident::new("borsh", Span::call_site())).unwrap();
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

        let actual = process_enum(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_associated_type() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum EnumParametrized<T, K, V>
            where
                K: TraitName,
                K: core::cmp::Ord, 
                V: core::cmp::Ord,
            {
                B {
                    x: BTreeMap<K, V>,
                    y: String,
                    z: K::Associated,
                },
                C(T, u16),
            }
        })
        .unwrap();

        let actual = process_enum(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
    #[test]
    fn generic_associated_type_param_override() {
        let item_struct: ItemEnum = syn::parse2(quote! {
            enum EnumParametrized<T, K, V>
            where
                K: TraitName,
                K: core::cmp::Ord, 
                V: core::cmp::Ord,
            {
                B {
                    x: BTreeMap<K, V>,
                    y: String,
                    #[borsh(schema(params = "K => <K as TraitName>::Associated"))]
                    z: <K as TraitName>::Associated,
                },
                C(T, u16),
            }
        })
        .unwrap();

        let actual = process_enum(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
