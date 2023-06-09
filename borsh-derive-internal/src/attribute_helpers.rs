use syn::{Attribute, Path};

pub fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("borsh_skip"))
        .count()
        > 0
}

pub fn contains_initialize_with(attrs: &[Attribute]) -> syn::Result<Option<Path>> {
    let mut res = None;
    for attr in attrs.iter() {
        if attr.path().is_ident("borsh_init") {
            let _ = attr.parse_nested_meta(|meta| {
                res = Some(meta.path);
                Ok(())
            });
        }
    }
    Ok(res)
}
