use proc_macro2::TokenStream as TokenStream2;
use syn::{ItemUnion, Path};

pub fn process<const IS_ASYNC: bool>(
    _input: ItemUnion,
    _cratename: Path,
) -> syn::Result<TokenStream2> {
    unimplemented!()
}
