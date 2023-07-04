use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{
    parse_quote, AttrStyle, Attribute, Field, FieldMutability, Fields, FieldsUnnamed, Ident,
    ItemEnum, ItemStruct, Meta, Visibility,
};

use crate::helpers::{declaration, filter_skip, quote_where_clause};

pub fn process_enum(input: &ItemEnum, cratename: Ident) -> syn::Result<TokenStream2> {
    let mut input = input.clone();
    let name = &input.ident;
    let name_str = name.to_token_stream().to_string();
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    // Generate function that returns the name of the type.
    let (declaration, where_clause_additions) =
        declaration(&name_str, &input.generics, cratename.clone());

    // Generate function that returns the schema for variants.
    // Definitions of the variants.
    let mut variants_defs = vec![];
    // Definitions of the anonymous structs used in variants.
    let mut anonymous_defs = TokenStream2::new();
    // Recursive calls to `add_definitions_recursively`.
    let mut add_recursive_defs = TokenStream2::new();
    for variant in &mut input.variants {
        let variant_name_str = variant.ident.to_token_stream().to_string();
        let full_variant_name_str = format!("{}{}", name_str, variant_name_str);
        let full_variant_ident = Ident::new(full_variant_name_str.as_str(), Span::call_site());
        let mut anonymous_struct = ItemStruct {
            attrs: vec![],
            vis: Visibility::Inherited,
            struct_token: Default::default(),
            ident: full_variant_ident.clone(),
            generics: (*generics).clone(),
            fields: variant.fields.clone(),
            semi_token: Some(Default::default()),
        };
        let generic_params = generics
            .type_params()
            .fold(TokenStream2::new(), |acc, generic| {
                let ident = &generic.ident;
                quote! {
                    #acc
                    #ident ,
                }
            });

        match &mut anonymous_struct.fields {
            Fields::Named(named) => {
                for field in &mut named.named {
                    field.attrs = filter_skip(&field.attrs);
                }
            }
            Fields::Unnamed(unnamed) => {
                for field in &mut unnamed.unnamed {
                    field.attrs = filter_skip(&field.attrs);
                }
            }
            _ => {}
        }
        if !generic_params.is_empty() {
            let attr = Attribute {
                pound_token: Default::default(),
                style: AttrStyle::Outer,
                bracket_token: Default::default(),
                meta: Meta::Path(parse_quote! {borsh_skip}),
            };
            // Whether we should convert the struct from unit struct to regular struct.
            let mut unit_to_regular = false;
            match &mut anonymous_struct.fields {
                Fields::Named(named) => {
                    named.named.push(Field {
                        mutability: FieldMutability::None,
                        attrs: vec![attr.clone()],
                        vis: Visibility::Inherited,
                        ident: Some(Ident::new("borsh_schema_phantom_data", Span::call_site())),
                        colon_token: None,
                        ty: parse_quote! {::core::marker::PhantomData<(#generic_params)>},
                    });
                }
                Fields::Unnamed(unnamed) => {
                    unnamed.unnamed.push(Field {
                        mutability: FieldMutability::None,
                        attrs: vec![attr.clone()],
                        vis: Visibility::Inherited,
                        ident: None,
                        colon_token: None,
                        ty: parse_quote! {::core::marker::PhantomData<(#generic_params)>},
                    });
                }
                Fields::Unit => {
                    unit_to_regular = true;
                }
            }
            if unit_to_regular {
                let mut fields = FieldsUnnamed {
                    paren_token: Default::default(),
                    unnamed: Default::default(),
                };
                fields.unnamed.push(Field {
                    mutability: FieldMutability::None,
                    attrs: vec![attr],
                    vis: Visibility::Inherited,
                    ident: None,
                    colon_token: None,
                    ty: parse_quote! {::core::marker::PhantomData<(#generic_params)>},
                });
                anonymous_struct.fields = Fields::Unnamed(fields);
            }
        }
        anonymous_defs.extend(quote! {
            #[allow(dead_code)]
            #[derive(#cratename::BorshSchema)]
            #anonymous_struct
        });
        add_recursive_defs.extend(quote! {
            <#full_variant_ident #ty_generics as #cratename::BorshSchema>::add_definitions_recursively(definitions);
        });
        variants_defs.push(quote! {
            (#variant_name_str.to_string(), <#full_variant_ident #ty_generics>::declaration())
        });
    }

    let type_definitions = quote! {
        fn add_definitions_recursively(definitions: &mut #cratename::__private::maybestd::collections::BTreeMap<#cratename::schema::Declaration, #cratename::schema::Definition>) {
            #anonymous_defs
            #add_recursive_defs
            let variants = #cratename::__private::maybestd::vec![#(#variants_defs),*];
            let definition = #cratename::schema::Definition::Enum{variants};
            Self::add_definition(Self::declaration(), definition, definitions);
        }
    };
    let where_clause = quote_where_clause(where_clause, where_clause_additions);
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
            enum Side<A, B>
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
}
