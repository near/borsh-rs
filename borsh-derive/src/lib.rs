extern crate proc_macro;
use borsh_derive_internal::*;
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::crate_name;
use proc_macro_crate::FoundCrate;
use quote::ToTokens;
use syn::{Ident, ItemEnum, ItemStruct, ItemUnion};

#[cfg(feature = "schema")]
use borsh_schema_derive_internal::{process_enum, process_struct};
use syn::{parse2, Meta, MetaNameValue};
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(BorshSerialize, attributes(borsh_skip, borsh))]
pub fn borsh_serialize(input: TokenStream) -> TokenStream {
    let name = &crate_name("borsh").unwrap();
    let name = match name {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    let cratename = Ident::new(name, Span::call_site());

    let for_derive_input = input.clone();
    let derive_input = parse_macro_input!(for_derive_input as DeriveInput);

    let use_discriminant = match check_use_discriminant(derive_input) {
        Ok(value) => value,
        Err(value) => return value,
    };

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        struct_ser(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        enum_ser(&input, cratename, use_discriminant)
    } else if let Ok(input) = syn::parse::<ItemUnion>(input) {
        union_ser(&input, cratename)
    } else {
        // Derive macros can only be defined on structs, enums, and unions.
        unreachable!()
    };
    TokenStream::from(match res {
        Ok(res) => res,
        Err(err) => err.to_compile_error(),
    })
}

#[proc_macro_derive(BorshDeserialize, attributes(borsh_skip, borsh_init, borsh))]
pub fn borsh_deserialize(input: TokenStream) -> TokenStream {
    let name = &crate_name("borsh").unwrap();
    let name = match name {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    let cratename = Ident::new(name, Span::call_site());

    let for_derive_input = input.clone();
    let derive_input = parse_macro_input!(for_derive_input as DeriveInput);

    let use_discriminant = match check_use_discriminant(derive_input) {
        Ok(value) => value,
        Err(value) => return value,
    };

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        struct_de(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        enum_de(&input, cratename, use_discriminant)
    } else if let Ok(input) = syn::parse::<ItemUnion>(input) {
        union_de(&input, cratename)
    } else {
        // Derive macros can only be defined on structs, enums, and unions.
        unreachable!()
    };
    TokenStream::from(match res {
        Ok(res) => res,
        Err(err) => err.to_compile_error(),
    })
}

fn check_use_discriminant(derive_input: DeriveInput) -> Result<Option<bool>, TokenStream> {
    for attr in &derive_input.attrs {
        if attr.path().is_ident("borsh") {
            if let Meta::List(list) = attr.meta.clone() {
                let tokens = list.tokens;
                let meta: Meta = parse2(tokens).map_err(|err| err.to_compile_error())?;

                if let Meta::NameValue(value) = meta {
                    let MetaNameValue {
                        eq_token: _, value, ..
                    } = value;
                    let value = value.to_token_stream().to_string();
                    return match value.as_str() {
                        "true" => Ok(Some(true)),
                        "false" => Ok(Some(false)),
                        _ => {
                            return Err(TokenStream::from(
                                syn::Error::new(
                                    derive_input.ident.span(),
                                    "`use_discriminant` accept only `true` or `false`",
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

#[cfg(feature = "schema")]
#[proc_macro_derive(BorshSchema, attributes(borsh_skip, use_discriminant))]
pub fn borsh_schema(input: TokenStream) -> TokenStream {
    let name = &crate_name("borsh").unwrap();
    let name = match name {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    let cratename = Ident::new(name, Span::call_site());

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        process_struct(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        process_enum(&input, cratename)
    } else if syn::parse::<ItemUnion>(input).is_ok() {
        Err(syn::Error::new(
            Span::call_site(),
            "Borsh schema does not support unions yet.",
        ))
    } else {
        // Derive macros can only be defined on structs, enums, and unions.
        unreachable!()
    };
    TokenStream::from(match res {
        Ok(res) => res,
        Err(err) => err.to_compile_error(),
    })
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
        let actual = check_use_discriminant(item_enum).unwrap();

        assert!(actual.is_some());
        assert!(!actual.unwrap());
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
        let actual = check_use_discriminant(item_enum).unwrap();

        assert!(actual.is_some());
        assert!(actual.unwrap());
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
        let actual = check_use_discriminant(item_enum);
        assert!(actual.is_err());
    }
}
