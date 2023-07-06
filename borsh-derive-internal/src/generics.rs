use quote::quote;
use syn::{Generics, Path, WherePredicate};

pub fn compute_predicates(generics: &Generics, traitname: &Path) -> Vec<WherePredicate> {
    let mut where_predicates = vec![];
    for type_param in generics.type_params() {
        let type_param_name = &type_param.ident;
        where_predicates.push(
            syn::parse2(quote! {
                #type_param_name: #traitname
            })
            .unwrap(),
        );
    }
    where_predicates
}
