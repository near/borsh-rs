extern crate proc_macro2;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{parse2, Attribute, DeriveInput, Meta, MetaNameValue, Path};

// item level attribute for enums
const USE_DISCRIMINANT: &str = "use_discriminant";

// field only attribute
const BORSH_INIT: &str = "borsh_init";
const BORSH_SKIP: &str = "borsh_skip";
const BORSH_TOP: &str = "borsh";

pub fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(BORSH_SKIP))
}

pub fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
    for attr in attrs.iter() {
        if attr.path().is_ident(BORSH_INIT) {
            let mut res = None;
            let _ = attr.parse_nested_meta(|meta| {
                res = Some(meta.path);
                Ok(())
            });
            return res;
        }
    }

    None
}

pub fn check_item_attributes(derive_input: &DeriveInput) -> Result<(), proc_macro2::TokenStream> {
    for attr in derive_input.attrs.clone() {
        if attr.path().is_ident(BORSH_TOP) {
            if let syn::Data::Struct(ref _data) = derive_input.data {
                return Err(TokenStream::from(
                    syn::Error::new(
                        derive_input.ident.span(),
                        "borsh (use_discriminant=<bool>) does not support structs",
                    )
                    .to_compile_error(),
                ));
            }
        }
    }
    Ok(())
}

pub fn contains_use_discriminant(
    attrs: &[Attribute],
) -> Result<Option<bool>, proc_macro2::TokenStream> {
    for attr in attrs {
        if attr.path().is_ident(BORSH_TOP) {
            let span = attr.path().get_ident().unwrap().span();
            if let Meta::List(list) = attr.meta.clone() {
                let tokens = list.tokens;
                let meta: Meta = parse2(tokens).map_err(|err| err.to_compile_error())?;

                if let Meta::NameValue(value) = meta {
                    let MetaNameValue { path, value, .. } = value;
                    if !path.is_ident(USE_DISCRIMINANT) {
                        return Err(TokenStream::from(
                            syn::Error::new(
                                span,
                                "`use_discriminant` is the only supported attribute for `borsh`",
                            )
                            .to_compile_error(),
                        ));
                    }

                    let value = value.to_token_stream().to_string();
                    return match value.as_str() {
                        "true" => Ok(Some(true)),
                        "false" => Ok(Some(false)),
                        _ => {
                            return Err(TokenStream::from(
                                syn::Error::new(
                                    span,
                                    "`use_discriminant` accepts only `true` or `false`",
                                )
                                .to_compile_error(),
                            ));
                        }
                    };
                }
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {

    use super::*;
    use quote::quote;
    #[test]
    fn test_check_use_discriminant() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = false)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum.attrs);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }

    #[test]
    fn test_check_use_discriminant_true() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = true)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum.attrs);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }

    #[test]
    #[should_panic]
    fn test_check_use_discriminant_wrong_value() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = 111)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum.attrs);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    #[should_panic]
    fn test_check_use_discriminant_on_struct() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = false)]
            struct AWithUseDiscriminantFalse {
                x: X,
                y: Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum.attrs);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    #[should_panic]
    fn test_check_use_borsh_skip_on_whole_struct() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = false)]
            #[borsh_skip=x]
            struct AWithUseDiscriminantFalse {
                x: X,
                y: Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum.attrs);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    #[should_panic]
    fn test_check_use_borsh_invalid_on_whole_struct() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(invalid)]
            struct AWithUseDiscriminantFalse {
                x: X,
                y: Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum.attrs);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
}
