use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{ExprPath, Generics, Ident, Path};

use super::{
    attributes::{field, BoundType, DESERIALIZE_WITH, DESERIALIZE_WITH_BOUNDED},
    generics,
};

pub mod enums;
pub mod structs;
pub mod unions;

#[derive(Clone, Copy)]
enum Flavor {
    /// `BorshDeserialize`
    Plain,
    Bounded,
}

impl Flavor {
    fn bound_type(self) -> BoundType {
        match self {
            Flavor::Plain => BoundType::Deserialize,
            Flavor::Bounded => BoundType::DeserializeBounded,
        }
    }

    fn trait_path(self, cratename: &Path) -> Path {
        let path = match self {
            Flavor::Plain => quote! { #cratename::de::BorshDeserialize },
            Flavor::Bounded => quote! { #cratename::de::BorshDeserializeBounded },
        };
        syn::parse2(path).unwrap()
    }

    fn default_field_fn(self, cratename: &Path) -> ExprPath {
        let path = match self {
            Flavor::Plain => quote! { #cratename::BorshDeserialize::deserialize_reader },
            Flavor::Bounded => {
                quote! { #cratename::BorshDeserializeBounded::deserialize_bounded_alloc }
            }
        };
        syn::parse2(path).unwrap()
    }

    fn call_args(self, budget: &Ident) -> TokenStream2 {
        match self {
            Flavor::Plain => quote! { reader },
            Flavor::Bounded => quote! { reader, #budget },
        }
    }

    /// Reject plain-only custom decoders: they would bypass the memory limit.
    fn field_fn(self, parsed: &field::Attributes) -> syn::Result<Option<ExprPath>> {
        match self {
            Flavor::Plain => Ok(parsed.deserialize_with.clone()),
            Flavor::Bounded => match (&parsed.deserialize_with_bounded, &parsed.deserialize_with) {
                (Some(bounded), _) => Ok(Some(bounded.clone())),
                (None, Some(plain)) => Err(syn::Error::new_spanned(
                    plain,
                    format!(
                        "`{}` has no bounded counterpart; add `{} = \"...\"`",
                        DESERIALIZE_WITH.0, DESERIALIZE_WITH_BOUNDED.0
                    ),
                )),
                (None, None) => Ok(None),
            },
        }
    }
}

/// Avoid generic parameter names, which could turn the budget binding into a const pattern.
/// A mixed-site span prevents the binding from capturing user paths such as
/// `deserialize_with_bounded = "budget"`.
fn budget_ident(generics: &Generics) -> Ident {
    use syn::ext::IdentExt;

    // `r#__budget` names the same parameter as `__budget`.
    let mut name = String::from("__budget");
    while generics
        .type_params()
        .any(|param| param.ident.unraw() == name)
        || generics
            .const_params()
            .any(|param| param.ident.unraw() == name)
    {
        name.push('_');
    }
    Ident::new(&name, Span::mixed_site())
}

struct GenericsOutput {
    flavor: Flavor,
    budget: Ident,
    overrides: Vec<syn::WherePredicate>,
    default_visitor: generics::FindTyParams,
    deserialize_visitor: generics::FindTyParams,
}

impl GenericsOutput {
    fn new(generics: &Generics, flavor: Flavor) -> Self {
        Self {
            flavor,
            budget: budget_ident(generics),
            overrides: vec![],
            deserialize_visitor: generics::FindTyParams::new(generics),
            default_visitor: generics::FindTyParams::new_for_default(generics),
        }
    }
    fn extend(self, where_clause: &mut syn::WhereClause, cratename: &Path) {
        let de_trait = self.flavor.trait_path(cratename);
        let default_trait: Path = syn::parse2(quote! { core::default::Default }).unwrap();
        let de_predicates =
            generics::compute_predicates(self.deserialize_visitor.process_for_bounds(), &de_trait);
        let default_predicates =
            generics::compute_predicates(self.default_visitor.process_for_bounds(), &default_trait);
        where_clause.predicates.extend(de_predicates);
        where_clause.predicates.extend(default_predicates);
        where_clause.predicates.extend(self.overrides);
    }
}

fn process_field(
    field: &syn::Field,
    cratename: &Path,
    body: &mut TokenStream2,
    generics: &mut GenericsOutput,
) -> syn::Result<()> {
    let parsed = field::Attributes::parse(&field.attrs)?;
    let flavor = generics.flavor;

    generics
        .overrides
        .extend(parsed.collect_bounds(flavor.bound_type()));
    let needs_bounds_derive = parsed.needs_bounds_derive(flavor.bound_type());

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
        let with = flavor.field_fn(&parsed)?;
        field_output(field_name, cratename, with, flavor, &generics.budget)
    };
    body.extend(delta);
    Ok(())
}

/// function which computes derive output [proc_macro2::TokenStream]
/// of code, which deserializes single field
fn field_output(
    field_name: Option<&Ident>,
    cratename: &Path,
    deserialize_with: Option<ExprPath>,
    flavor: Flavor,
    budget: &Ident,
) -> TokenStream2 {
    let path: ExprPath = deserialize_with.unwrap_or_else(|| flavor.default_field_fn(cratename));
    let args = flavor.call_args(budget);
    if let Some(field_name) = field_name {
        quote! {
            #field_name: #path(#args)?,
        }
    } else {
        quote! {
            #path(#args)?,
        }
    }
}

/// function which computes derive output [proc_macro2::TokenStream]
/// of code, which deserializes single skipped field
fn field_default_output(field_name: Option<&Ident>) -> TokenStream2 {
    if let Some(field_name) = field_name {
        quote! {
            #field_name: core::default::Default::default(),
        }
    } else {
        quote! { core::default::Default::default(), }
    }
}
