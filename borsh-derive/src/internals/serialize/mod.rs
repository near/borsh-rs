pub mod enums;
pub mod structs;
pub mod unions;

use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{ExprPath, Ident};

/// function which computes derive output [proc_macro2::TokenStream]
/// of code, which serializes single field
fn field_serialization_output<T: ToTokens>(
    arg: &T,
    cratename: &Ident,
    serialize_with: Option<ExprPath>,
) -> TokenStream2 {
    if let Some(func) = serialize_with {
        quote! {
            #func(#arg, writer)?;
        }
    } else {
        quote! {
            #cratename::BorshSerialize::serialize(#arg, writer)?;
        }
    }
}
