use proc_macro2::TokenStream as TokenStream2;
use syn::{Ident, ItemUnion};

pub fn process(_input: &ItemUnion, _cratename: Ident) -> syn::Result<TokenStream2> {
    unimplemented!()
}
