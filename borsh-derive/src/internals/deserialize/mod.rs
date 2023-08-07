use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ExprPath, Ident};

pub mod enums;
pub mod structs;
pub mod unions;

/// function which computes derive output [proc_macro2::TokenStream]
/// of code, which deserializes single field
pub fn field_deserialization_output(
    field_name: Option<&Ident>,
    cratename: &Ident,
    deserialize_with: Option<ExprPath>,
) -> TokenStream2 {
    let default_path: ExprPath =
        syn::parse2(quote! { #cratename::BorshDeserialize::deserialize_reader }).unwrap();
    let path: ExprPath = deserialize_with.unwrap_or(default_path);
    if let Some(field_name) = field_name {
        quote! {
            #field_name: #path(reader)?,
        }
    } else {
        quote! {
            #path(reader)?,
        }
    }
}
