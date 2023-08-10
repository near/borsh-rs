use crate::internals::attributes::{BORSH, INIT, SKIP, USE_DISCRIMINANT};
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{spanned::Spanned, Attribute, DeriveInput, Expr, ItemEnum, Path};

pub fn check_item_attributes(derive_input: &DeriveInput) -> Result<(), TokenStream> {
    // TODO remove in next PR
    let has_borsh_skip_on_top = derive_input.attrs.iter().any(|attr| attr.path() == SKIP);

    if has_borsh_skip_on_top {
        return Err(syn::Error::new(
            derive_input.ident.span(),
            "`borsh_skip` is not allowed as derive input attribute",
        )
        .to_compile_error());
    }

    let attr = derive_input.attrs.iter().find(|attr| attr.path() == BORSH);
    if let Some(attr) = attr {
        attr.parse_nested_meta(|meta| {
            if meta.path != USE_DISCRIMINANT && meta.path != INIT {
                return Err(syn::Error::new(
                    meta.path.span(),
                    "`use_discriminant` or `init` are only supported attributes for `borsh`",
                ));
            }
            if meta.path == USE_DISCRIMINANT {
                let _expr: Expr = meta.value()?.parse()?;
                if let syn::Data::Struct(ref _data) = derive_input.data {
                    return Err(syn::Error::new(
                        derive_input.ident.span(),
                        "borsh(use_discriminant=<bool>) does not support structs",
                    ));
                }
            }
            if meta.path == INIT {
                let _expr: Expr = meta.value()?.parse()?;
            }

            Ok(())
        })
        .map_err(|err| err.to_compile_error())?;
    }
    Ok(())
}

pub fn contains_use_discriminant(input: &ItemEnum) -> Result<bool, syn::Error> {
    if input.variants.len() > 256 {
        return Err(syn::Error::new(
            input.span(),
            "up to 256 enum variants are supported",
        ));
    }

    let attrs = &input.attrs;
    let mut use_discriminant = None;
    let attr = attrs.iter().find(|attr| attr.path() == BORSH);
    if let Some(attr) = attr {
        attr.parse_nested_meta(|meta| {
            if meta.path == USE_DISCRIMINANT {
                let value_expr: Expr = meta.value()?.parse()?;
                let value = value_expr.to_token_stream().to_string();
                match value.as_str() {
                    "true" => {
                        use_discriminant = Some(true);
                    }
                    "false" => use_discriminant = Some(false),
                    _ => {
                        return Err(syn::Error::new(
                            value_expr.span(),
                            "`use_discriminant` accepts only `true` or `false`",
                        ));
                    }
                };
            }

            if meta.path == INIT {
                let _value_expr: Expr = meta.value()?.parse()?;
            }
            Ok(())
        })?;
    }
    let has_explicit_discriminants = input
        .variants
        .iter()
        .any(|variant| variant.discriminant.is_some());
    if has_explicit_discriminants && use_discriminant.is_none() {
        return Err(syn::Error::new(
                input.ident.span(),
                "You have to specify `#[borsh(use_discriminant=true)]` or `#[borsh(use_discriminant=false)]` for all enums with explicit discriminant",
            ));
    }
    Ok(use_discriminant.unwrap_or(false))
}

pub(crate) fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
    let mut res = None;
    let attr = attrs.iter().find(|attr| attr.path() == BORSH);
    if let Some(attr) = attr {
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path == INIT {
                let value_expr: Path = meta.value()?.parse()?;
                res = Some(value_expr);
            } else if meta.path == USE_DISCRIMINANT {
                let _value_expr: Expr = meta.value()?.parse()?;
            };
            Ok(())
        });
    }

    res
}

#[cfg(test)]
mod tests {
    use crate::internals::test_helpers::{
        local_insta_assert_debug_snapshot, local_insta_assert_snapshot,
    };
    use quote::{quote, ToTokens};
    use syn::ItemEnum;

    use super::*;
    #[test]
    fn test_check_use_discriminant() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = false)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum);
        assert!(!actual.unwrap());
    }

    #[test]
    fn test_check_use_discriminant_true() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = true)]
            enum AWithUseDiscriminantTrue {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum);
        assert!(actual.unwrap());
    }

    #[test]
    fn test_check_use_discriminant_wrong_value() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = 111)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum);
        let err = match actual {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }
    #[test]
    fn test_check_use_discriminant_on_struct_check_attrs() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = false)]
            struct AWithUseDiscriminantFalse {
                x: X,
                y: Y,
            }
        })
        .unwrap();
        let actual = check_item_attributes(&item_enum);
        local_insta_assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    fn test_check_use_borsh_skip_on_whole_struct_check_attrs() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh_skip]
            struct AWithUseDiscriminantFalse {
                 x: X,
                 y: Y,
            }
        })
        .unwrap();
        let actual = check_item_attributes(&item_enum);
        local_insta_assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    fn test_check_use_borsh_invalid_on_whole_struct_check_attrs() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(invalid)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = check_item_attributes(&item_enum);
        local_insta_assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    fn test_init_function_check_attrs() {
        let item_struct = syn::parse2::<DeriveInput>(quote! {
            #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
            #[borsh(init = initialization_method)]
            struct A<'a> {
                x: u64,
            }
        })
        .unwrap();

        let actual = check_item_attributes(&item_struct);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_init_function_with_use_discriminant_check_attrs() {
        let item_struct = syn::parse2::<DeriveInput>(quote! {
            #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
            #[borsh(use_discriminant=true, init = initialization_method)]
            enum A {
                B,
                C,
                D= 10,
            }
        })
        .unwrap();

        let actual = check_item_attributes(&item_struct);
        assert!(actual.is_ok());
    }
    #[test]
    fn test_init_contains_discriminant_reversed_check_attrs() {
        let item_struct = syn::parse2::<DeriveInput>(quote! {
            #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
            #[borsh(init = initialization_method, use_discriminant=true)]
            enum A {
                B,
                C,
                D= 10,
            }
        })
        .unwrap();

        let actual = check_item_attributes(&item_struct);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_init_function_wrong_format_check_attrs() {
        let item_struct: DeriveInput = syn::parse2(quote! {
        #[derive(BorshDeserialize, Debug)]
        #[borsh(init_func = initialization_method)]
        struct A<'a> {
            x: u64,
            b: B,
            y: f32,
            z: String,
            v: Vec<String>,

        }
            })
        .unwrap();
        let actual = check_item_attributes(&item_struct);
        local_insta_assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    fn test_contains_initialize_with_function() {
        let item_struct = syn::parse2::<DeriveInput>(quote! {
            #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
            #[borsh(init = initialization_method)]
            struct A<'a> {
                x: u64,
            }
        })
        .unwrap();

        let actual = contains_initialize_with(&item_struct.attrs);
        assert_eq!(
            actual.unwrap().to_token_stream().to_string(),
            "initialization_method"
        );
    }

    #[test]
    fn test_contains_initialize_with_contains_discriminant() {
        let item_struct = syn::parse2::<ItemEnum>(quote! {
            #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
            #[borsh(init = initialization_method, use_discriminant=true)]
            enum A {
                B,
                C,
                D,
            }
        })
        .unwrap();

        let actual = contains_initialize_with(&item_struct.attrs);
        assert_eq!(
            actual.unwrap().to_token_stream().to_string(),
            "initialization_method"
        );
        let actual = contains_use_discriminant(&item_struct);
        assert!(actual.unwrap());
    }

    #[test]
    fn test_contains_initialize_with_contains_discriminant_reversed() {
        let item_struct = syn::parse2::<ItemEnum>(quote! {
            #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
            #[borsh(use_discriminant=true, init = initialization_method)]
            enum A {
                B,
                C,
                D,
            }
        })
        .unwrap();

        let actual = contains_initialize_with(&item_struct.attrs);
        assert_eq!(
            actual.unwrap().to_token_stream().to_string(),
            "initialization_method"
        );
        let actual = contains_use_discriminant(&item_struct);
        assert!(actual.unwrap());
    }
}
