use syn::{Attribute, Path};

pub fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("borsh_skip"))
}

pub fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
    let mut res = None;
    for attr in attrs.iter() {
        if attr.path().is_ident("borsh_init") {
            let _ = attr.parse_nested_meta(|meta| {
                res = Some(meta.path);
                Ok(())
            });
        }
    }

    res
}
