extern crate proc_macro;
use borsh_derive_internal::*;
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::crate_name;
use proc_macro_crate::FoundCrate;
use quote::ToTokens;
use syn::{Ident, ItemEnum, ItemStruct, ItemUnion};

use borsh_derive_internal::*;
#[cfg(feature = "schema")]
use borsh_schema_derive_internal::*;
use quote::quote;
use syn::Attribute;
use syn::{parse_macro_input, parse_quote, DeriveInput};

#[proc_macro_derive(BorshSerialize, attributes(borsh_skip, use_discriminant))]
pub fn borsh_serialize(input: TokenStream) -> TokenStream {
    let name = &crate_name("borsh").unwrap();
    let name = match name {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    let cratename = Ident::new(name, Span::call_site());

    let for_derive_input = input.clone();
    let derive_input = parse_macro_input!(for_derive_input as DeriveInput);

    // Read the additional data
    let mut use_discriminant = false;
    for attr in &derive_input.attrs {
        if attr.path().is_ident("use_discriminant") {
            for token in attr.to_token_stream().clone() {
                if token.to_string() == "true" {
                    use_discriminant = true;
                }
                if token.to_string() == "false" {
                    use_discriminant = false;
                }
            }
        }
    }

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

#[proc_macro_derive(BorshDeserialize, attributes(borsh_skip, borsh_init, use_discriminant))]
pub fn borsh_deserialize(input: TokenStream) -> TokenStream {
    let name = &crate_name("borsh").unwrap();
    let name = match name {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    let cratename = Ident::new(name, Span::call_site());

    let for_derive_input = input.clone();
    let derive_input = parse_macro_input!(for_derive_input as DeriveInput);

    // Read the additional data
    let mut use_discriminant = false;
    for attr in &derive_input.attrs {
        if attr.path().is_ident("use_discriminant") {
            for token in attr.to_token_stream() {
                if token.to_string() == "true" {
                    use_discriminant = true;
                }
                if token.to_string() == "false" {
                    use_discriminant = false;
                }
            }
            println!("use_discriminant: {}", use_discriminant);
        }
    }

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

#[proc_macro_attribute]
pub fn borsh(args: TokenStream, input: TokenStream) -> TokenStream {
    let tokens = args.clone();
    let attribute_args = syn::parse_macro_input!(tokens);

    let attr = MacroAttribute::from_attribute_args(
        "use_discriminant",
        attribute_args,
        syn::AttrStyle::Outer,
    );

    let mut use_discriminant = false;
    let mut found = false;
    let name_values_attributes = attr.into_name_values().unwrap();

    for (name, value) in &name_values_attributes {
        if name == "use_discriminant" {
            if value.to_string() == "true" {
                use_discriminant = true;
                found = true;
            }
        }
        // println!("{:7} => {}", name, value);
    }

    if found {
        let attr: Attribute = parse_quote!(#[use_discriminant = #use_discriminant]);

        let input = parse_macro_input!(input as DeriveInput);
        let mut new_item = input.clone();
        new_item.attrs.push(attr);

        let expanded = quote! { #new_item };
        TokenStream::from(expanded)
    } else {
        input
    }
}
