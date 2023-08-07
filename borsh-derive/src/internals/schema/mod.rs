use std::collections::HashSet;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    punctuated::Punctuated, token::Comma, Attribute, Field, Fields, GenericParam, Generics, Ident,
    Type, WherePredicate,
};

use crate::internals::{
    attributes::{BORSH, SKIP},
    generics::type_contains_some_param,
};

use crate::internals::{attributes::field, attributes::field::schema, generics};

pub mod enums;
pub mod structs;

pub fn filter_field_attrs(
    attrs: impl Iterator<Item = Attribute>,
) -> impl Iterator<Item = Attribute> {
    attrs.filter(|attr| attr.path() == SKIP || attr.path() == BORSH)
}

pub fn declaration(
    ident_str: &str,
    cratename: Ident,
    params_for_bounds: Vec<Type>,
) -> TokenStream2 {
    // Generate function that returns the name of the type.
    let mut declaration_params = vec![];
    for type_param in params_for_bounds {
        declaration_params.push(quote! {
            <#type_param>::declaration()
        });
    }
    if declaration_params.is_empty() {
        quote! {
                #ident_str.to_string()
        }
    } else {
        quote! {
                let params = #cratename::__private::maybestd::vec![#(#declaration_params),*];
                format!(r#"{}<{}>"#, #ident_str, params.join(", "))
        }
    }
}

pub fn filter_used_params(
    generics: &Generics,
    not_skipped_type_params: HashSet<Ident>,
) -> Generics {
    let new_params = generics
        .params
        .clone()
        .into_iter()
        .filter(|param| match param {
            GenericParam::Lifetime(..) | GenericParam::Const(..) => true,
            GenericParam::Type(ty_param) => not_skipped_type_params.contains(&ty_param.ident),
        })
        .collect();

    let mut where_clause = generics.where_clause.clone();
    where_clause = where_clause.map(|mut clause| {
        let new_predicates: Punctuated<WherePredicate, Comma> = clause
            .predicates
            .iter()
            .filter(|predicate| match predicate {
                WherePredicate::Lifetime(..) => true,
                WherePredicate::Type(predicate_type) => {
                    type_contains_some_param(&predicate_type.bounded_ty, &not_skipped_type_params)
                }
                #[cfg_attr(
                    feature = "force_exhaustive_checks",
                    deny(non_exhaustive_omitted_patterns)
                )]
                _ => true,
            })
            .cloned()
            .collect();
        clause.predicates = new_predicates;
        clause
    });
    Generics {
        params: new_params,
        where_clause,
        ..generics.clone()
    }
}

fn visit_field(field: &Field, visitor: &mut generics::FindTyParams) -> syn::Result<()> {
    let skipped = field::contains_skip(&field.attrs);
    let parsed = field::Attributes::parse(&field.attrs, skipped)?;
    let needs_schema_params_derive = parsed.needs_schema_params_derive();
    let schema_attrs = parsed.schema;
    if !skipped {
        if needs_schema_params_derive {
            visitor.visit_field(field);
        }
        // there's no need to override params when field is skipped, because when field is skipped
        // derive for it doesn't attempt to add any bounds, unlike `BorshDeserialize`, which
        // adds `Default` bound on any type parameters in skipped field

        if let Some(schema_attrs) = schema_attrs {
            if let Some(schema_params) = schema_attrs.params {
                for schema::ParameterOverride {
                    order_param,
                    override_type,
                    ..
                } in schema_params
                {
                    visitor.param_associated_type_insert(order_param, override_type);
                }
            }
        }
    }
    Ok(())
}

/// check param usage in fields with respect to `borsh_skip` attribute usage
pub fn visit_struct_fields(
    fields: &Fields,
    visitor: &mut generics::FindTyParams,
) -> syn::Result<()> {
    match &fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                visit_field(field, visitor)?;
            }
        }
        Fields::Unnamed(fields) => {
            for field in &fields.unnamed {
                visit_field(field, visitor)?;
            }
        }
        Fields::Unit => {}
    }
    Ok(())
}

/// check param usage in fields
pub fn visit_struct_fields_unconditional(fields: &Fields, visitor: &mut generics::FindTyParams) {
    match &fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                visitor.visit_field(field);
            }
        }
        Fields::Unnamed(fields) => {
            for field in &fields.unnamed {
                visitor.visit_field(field);
            }
        }
        Fields::Unit => {}
    }
}
