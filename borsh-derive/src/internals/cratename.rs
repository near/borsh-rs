use proc_macro2::Span;
#[cfg(feature = "proc-macro-crate")]
use proc_macro_crate::{crate_name, FoundCrate};
use syn::{Attribute, Error, Ident, Path};

use super::attributes::item;

pub(crate) const BORSH: &str = "borsh";

pub(crate) fn get(attrs: &[Attribute]) -> Result<Path, Error> {
    let path = item::get_crate(attrs)?;
    match path {
        Some(path) => Ok(path),
        None => {
            #[cfg(feature = "proc-macro-crate")]
            let ident = get_from_cargo();
            #[cfg(not(feature = "proc-macro-crate"))]
            let ident = static_string();
            Ok(ident.into())
        }
    }
}

#[cfg(feature = "proc-macro-crate")]
pub(crate) fn get_from_cargo() -> Ident {
    let name = &crate_name(BORSH).unwrap();
    let name = match name {
        FoundCrate::Itself => BORSH,
        FoundCrate::Name(name) => name.as_str(),
    };
    Ident::new(name, Span::call_site())
}

#[cfg(not(feature = "proc-macro-crate"))]
pub(crate) fn static_string() -> Ident {
    Ident::new(BORSH, Span::call_site())
}
