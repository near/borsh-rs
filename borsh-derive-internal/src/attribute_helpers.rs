use syn::{Attribute, Path};

pub fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("borsh_skip"))
}

pub fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
    for attr in attrs.iter() {
        if attr.path().is_ident("borsh_init") {
            let mut res = None;
            let _ = attr.parse_nested_meta(|meta| {
                res = Some(meta.path);
                Ok(())
            });
            return res;
        }
    }

    None
}
