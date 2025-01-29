use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use syn::{Ident, Path};

use super::cratename::BORSH;

pub fn pretty_print_syn_str(input: TokenStream) -> syn::Result<String> {
    let syn_file = syn::parse2::<syn::File>(input)?;

    Ok(prettyplease::unparse(&syn_file))
}

pub fn debug_print_vec_of_tokenizable<T: ToTokens>(optional: Option<Vec<T>>) -> String {
    if let Some(vec) = optional {
        let mut s = String::new();
        for element in vec {
            s.push_str(&element.to_token_stream().to_string());
            s.push('\n');
        }
        s
    } else {
        "None".to_owned()
    }
}

pub fn debug_print_tokenizable<T: ToTokens>(optional: Option<T>) -> String {
    if let Some(type_) = optional {
        let mut s = type_.to_token_stream().to_string();
        s.push('\n');
        s
    } else {
        "None".to_owned()
    }
}

macro_rules! local_insta_assert_debug_snapshot {
    ($value:expr) => {{

        insta::with_settings!({prepend_module_to_snapshot => false}, {
            insta::assert_debug_snapshot!($value);
        });
    }};
}

macro_rules! local_insta_assert_snapshot {
    ($value:expr) => {{

        insta::with_settings!({prepend_module_to_snapshot => false}, {
            insta::assert_snapshot!($value);
        });
    }};
}

pub(crate) fn default_cratename() -> Path {
    let cratename = Ident::new(BORSH, Span::call_site());
    cratename.into()
}

pub(crate) use local_insta_assert_debug_snapshot;
pub(crate) use local_insta_assert_snapshot;
