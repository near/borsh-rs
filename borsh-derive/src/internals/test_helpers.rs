use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::fmt::Write;

pub fn pretty_print_syn_str(input: &TokenStream) -> syn::Result<String> {
    let input = format!("{}", quote!(#input));
    let syn_file = syn::parse_str::<syn::File>(&input)?;

    Ok(prettyplease::unparse(&syn_file))
}

pub fn debug_print_vec_of_tokenizable<T: ToTokens>(optional: Option<Vec<T>>) -> String {
    let mut s = String::new();
    if let Some(vec) = optional {
        for element in vec {
            writeln!(&mut s, "{}", element.to_token_stream()).unwrap();
        }
    } else {
        write!(&mut s, "None").unwrap();
    }
    s
}

pub fn debug_print_tokenizable<T: ToTokens>(optional: Option<T>) -> String {
    let mut s = String::new();
    if let Some(type_) = optional {
        writeln!(&mut s, "{}", type_.to_token_stream()).unwrap();
    } else {
        write!(&mut s, "None").unwrap();
    }
    s
}
