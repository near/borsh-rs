use syn::{Attribute, Path};

pub fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("borsh_skip"))
}

pub fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
    for attr in attrs.iter() {
        if attr.path().is_ident("borsh_init") {
            return Some(attr.path().clone());
        }
    }

    None
}
