use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_quote, ExprPath, Generics, Ident, Path, Type};

use super::{
    attributes::{field, BoundType},
    generics,
};

pub mod enums;
pub mod structs;
pub mod unions;

struct GenericsOutput {
    overrides: Vec<syn::WherePredicate>,
    default_visitor: generics::FindTyParams,
    deserialize_visitor: generics::FindTyParams,
}

impl GenericsOutput {
    fn new(generics: &Generics) -> Self {
        Self {
            overrides: vec![],
            deserialize_visitor: generics::FindTyParams::new(generics),
            default_visitor: generics::FindTyParams::new(generics),
        }
    }

    fn extend<const IS_ASYNC: bool>(self, where_clause: &mut syn::WhereClause, cratename: &Path) {
        let de_trait: Path = if IS_ASYNC {
            parse_quote! { #cratename::de::BorshDeserializeAsync }
        } else {
            parse_quote! { #cratename::de::BorshDeserialize }
        };
        let default_trait: Path = parse_quote! { ::core::default::Default };
        let de_predicates =
            generics::compute_predicates(self.deserialize_visitor.process_for_bounds(), &de_trait);
        let default_predicates =
            generics::compute_predicates(self.default_visitor.process_for_bounds(), &default_trait);
        where_clause.predicates.extend(de_predicates);
        where_clause.predicates.extend(default_predicates);
        where_clause.predicates.extend(self.overrides);
    }
}

fn process_field<const IS_ASYNC: bool>(
    field: &syn::Field,
    cratename: &Path,
    body: &mut TokenStream2,
    generics: &mut GenericsOutput,
) -> syn::Result<()> {
    let parsed = field::Attributes::parse(&field.attrs)?;

    generics.overrides.extend(if IS_ASYNC {
        parsed.collect_async_bounds(BoundType::Deserialize)
    } else {
        parsed.collect_bounds(BoundType::Deserialize)
    });
    let needs_bounds_derive = if IS_ASYNC {
        parsed.needs_async_bounds_derive(BoundType::Deserialize)
    } else {
        parsed.needs_bounds_derive(BoundType::Deserialize)
    };

    let field_name = field.ident.as_ref();
    let delta = if parsed.skip {
        if needs_bounds_derive {
            generics.default_visitor.visit_field(field);
        }
        field_default_output(field_name)
    } else {
        if needs_bounds_derive {
            generics.deserialize_visitor.visit_field(field);
        }
        field_output::<IS_ASYNC>(
            field_name,
            &field.ty,
            cratename,
            if IS_ASYNC {
                parsed.deserialize_with_async
            } else {
                parsed.deserialize_with
            },
        )
    };
    body.extend(delta);
    Ok(())
}

/// function which computes derive output [proc_macro2::TokenStream]
/// of code, which deserializes single field
fn field_output<const IS_ASYNC: bool>(
    field_name: Option<&Ident>,
    field_type: &Type,
    cratename: &Path,
    deserialize_with: Option<ExprPath>,
) -> TokenStream2 {
    let default_path = || {
        let deserialize_trait = Ident::new(
            if IS_ASYNC {
                "BorshDeserializeAsync"
            } else {
                "BorshDeserialize"
            },
            proc_macro2::Span::call_site(),
        );
        parse_quote! { <#field_type as #cratename::#deserialize_trait>::deserialize_reader }
    };

    let path: ExprPath = deserialize_with.unwrap_or_else(default_path);
    let dot_await = IS_ASYNC.then(|| quote! { .await });

    if let Some(field_name) = field_name {
        quote! { #field_name: #path(reader)#dot_await?, }
    } else {
        quote! { #path(reader)#dot_await?, }
    }
}

/// function which computes derive output [proc_macro2::TokenStream]
/// of code, which deserializes single skipped field
fn field_default_output(field_name: Option<&Ident>) -> TokenStream2 {
    if let Some(field_name) = field_name {
        quote! { #field_name: ::core::default::Default::default(), }
    } else {
        quote! { ::core::default::Default::default(), }
    }
}
