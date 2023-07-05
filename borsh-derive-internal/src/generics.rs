use quote::quote;
use syn::{Generics, Ident, WherePredicate};

pub fn compute_predicates(generics: &Generics, cratename: &Ident) -> Vec<WherePredicate> {
    let mut where_predicates = vec![];
    for type_param in generics.type_params() {
        let type_param_name = &type_param.ident;
        where_predicates.push(
            syn::parse2(quote! {
                #type_param_name: #cratename::ser::BorshSerialize
            })
            .unwrap(),
        );
    }
    where_predicates
}
