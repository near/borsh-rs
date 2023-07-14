use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{Field, Fields, Ident, ItemStruct, Path, WhereClause};

use crate::{
    attribute_helpers::{contains_skip, parse_schema_attrs, parsing_helpers::SchemaParamsOverride},
    generics::{compute_predicates, without_defaults, FindTyParams},
    schema_helpers::declaration,
};

fn visit_field(field: &Field, visitor: &mut FindTyParams) -> syn::Result<()> {
    if !contains_skip(&field.attrs) {
        // there's no need to override params when field is skipped, because when field is skipped
        // derive for it doesn't attempt to add any bounds, unlike `BorshDeserialize`, which
        // adds `Default` bound on any type parameters in skipped field
        let schema_attrs = parse_schema_attrs(&field.attrs)?;
        if let Some(schema_params) = schema_attrs {
            for SchemaParamsOverride {
                order_param,
                override_type,
                ..
            } in schema_params
            {
                visitor.param_associated_type_insert(order_param, override_type);
            }
        } else {
            visitor.visit_field(field);
        }
    }
    Ok(())
}

/// check param usage in fields with respect to `borsh_skip` attribute usage
pub fn visit_struct_fields(fields: &Fields, visitor: &mut FindTyParams) -> syn::Result<()> {
    match &fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                visit_field(field, visitor)?;
            }
        }
        Fields::Unnamed(fields) => {
            for field in &fields.unnamed {
                visit_field(field, visitor)?;
            }
        }
        Fields::Unit => {}
    }
    Ok(())
}

/// check param usage in fields
pub fn visit_struct_fields_unconditional(fields: &Fields, visitor: &mut FindTyParams) {
    match &fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                visitor.visit_field(field);
            }
        }
        Fields::Unnamed(fields) => {
            for field in &fields.unnamed {
                visitor.visit_field(field);
            }
        }
        Fields::Unit => {}
    }
}

pub fn process_struct(input: &ItemStruct, cratename: Ident) -> syn::Result<TokenStream2> {
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

    let mut schema_params_visitor = FindTyParams::new(&generics);

    // Generate function that returns the schema of required types.
    let mut fields_vec = vec![];
    let mut struct_fields = TokenStream2::new();
    let mut add_definitions_recursively_rec = TokenStream2::new();
    visit_struct_fields(&input.fields, &mut schema_params_visitor)?;
    match &input.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                if contains_skip(&field.attrs) {
                    continue;
                }
                let field_name = field.ident.as_ref().unwrap().to_token_stream().to_string();
                let field_type = &field.ty;
                fields_vec.push(quote! {
                    (#field_name.to_string(), <#field_type as #cratename::BorshSchema>::declaration())
                });
                add_definitions_recursively_rec.extend(quote! {
                    <#field_type as #cratename::BorshSchema>::add_definitions_recursively(definitions);
                });
            }
            if !fields_vec.is_empty() {
                struct_fields = quote! {
                    let fields = #cratename::schema::Fields::NamedFields(#cratename::__private::maybestd::vec![#(#fields_vec),*]);
                };
            }
        }
        Fields::Unnamed(fields) => {
            for field in &fields.unnamed {
                if contains_skip(&field.attrs) {
                    continue;
                }
                let field_type = &field.ty;
                fields_vec.push(quote! {
                    <#field_type as #cratename::BorshSchema>::declaration()
                });
                add_definitions_recursively_rec.extend(quote! {
                    <#field_type as #cratename::BorshSchema>::add_definitions_recursively(definitions);
                });
            }
            if !fields_vec.is_empty() {
                struct_fields = quote! {
                    let fields = #cratename::schema::Fields::UnnamedFields(#cratename::__private::maybestd::vec![#(#fields_vec),*]);
                };
            }
        }
        Fields::Unit => {}
    }

    if fields_vec.is_empty() {
        struct_fields = quote! {
            let fields = #cratename::schema::Fields::Empty;
        };
    }

    let add_definitions_recursively = quote! {
        fn add_definitions_recursively(definitions: &mut #cratename::__private::maybestd::collections::BTreeMap<#cratename::schema::Declaration, #cratename::schema::Definition>) {
            #struct_fields
            let definition = #cratename::schema::Definition::Struct { fields };

            let no_recursion_flag = definitions.get(&Self::declaration()).is_none();
            Self::add_definition(Self::declaration(), definition, definitions);
            if no_recursion_flag {
                #add_definitions_recursively_rec
            }
        }
    };

    let trait_path: Path = syn::parse2(quote! { #cratename::BorshSchema }).unwrap();
    let predicates = compute_predicates(
        schema_params_visitor.clone().process_for_bounds(),
        &trait_path,
    );
    where_clause.predicates.extend(predicates);

    // Generate function that returns the name of the type.
    let declaration = declaration(
        &name_str,
        cratename.clone(),
        schema_params_visitor.process_for_bounds(),
    );
    Ok(quote! {
        impl #impl_generics #cratename::BorshSchema for #name #ty_generics #where_clause {
            fn declaration() -> #cratename::schema::Declaration {
                #declaration
            }
            #add_definitions_recursively
        }
    })
}

#[cfg(test)]
mod tests {
    use proc_macro2::Span;

    use crate::test_helpers::pretty_print_syn_str;

    use super::*;

    #[test]
    fn unit_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A;
        })
        .unwrap();

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn wrapper_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<T>(T);
        })
        .unwrap();

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn tuple_struct() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A(u64, String);
        })
        .unwrap();

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn tuple_struct_params() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K, V>(K, V);
        })
        .unwrap();

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
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

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
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

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn trailing_comma_generics() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A<K, V>
            where
                K: Display + Debug,
            {
                x: HashMap<K, V>,
                y: String,
            }
        })
        .unwrap();

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn tuple_struct_whole_skip() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A(#[borsh_skip] String);
        })
        .unwrap();

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn tuple_struct_partial_skip() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A(#[borsh_skip] u64, String);
        })
        .unwrap();

        let actual = process_struct(
            &item_struct,
            Ident::new("borsh", proc_macro2::Span::call_site()),
        )
        .unwrap();
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

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

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

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_tuple_struct_borsh_skip3() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct G<U, K, V> (
                #[borsh_skip]
                HashMap<K, V>,
                U,
                K,
            );
        })
        .unwrap();

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_tuple_struct_borsh_skip4() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct ASalad<C>(Tomatoes, #[borsh_skip] C, Oil);
        })
        .unwrap();

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

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

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

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

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_associated_type() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T: Debug>
            where
                T: TraitName,
            {
                field: T::Associated,
                another: V,
            }
        })
        .unwrap();

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_associated_type_param_override() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh(schema(params =
                    "T => <T as TraitName>::Associated"
               ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        })
        .unwrap();

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_associated_type_param_override2() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh(schema(params =
                    "T => T, T => <T as TraitName>::Associated"
               ))]
                field: (<T as TraitName>::Associated, T),
                another: V,
            }
        })
        .unwrap();

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }

    #[test]
    fn generic_associated_type_param_override_ignored() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh_skip]
                #[borsh(schema(params =
                    "T => <T as TraitName>::Associated"
               ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        })
        .unwrap();

        let actual = process_struct(&item_struct, Ident::new("borsh", Span::call_site())).unwrap();

        insta::assert_snapshot!(pretty_print_syn_str(&actual).unwrap());
    }
}
