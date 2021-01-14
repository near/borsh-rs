use proc_macro2::TokenStream as TokenStream2;
use syn::ItemUnion;

pub fn union_ser(_input: &ItemUnion) -> syn::Result<TokenStream2> {
    unimplemented!()
}
