use proc_macro2::TokenStream;
use quote::quote;

pub fn pretty_print_syn_str(input: &TokenStream) -> syn::Result<String> {
    let input = format!("{}", quote!(#input));
    let syn_file = syn::parse_str::<syn::File>(&input)?;

    Ok(prettyplease::unparse(&syn_file))
}
