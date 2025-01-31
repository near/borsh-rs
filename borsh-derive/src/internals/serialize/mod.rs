use std::convert::TryFrom;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_quote, Expr, ExprPath, Generics, Ident, Index, Path, Type};

use super::generics;

pub mod enums;
pub mod structs;
pub mod unions;

struct GenericsOutput {
    overrides: Vec<syn::WherePredicate>,
    serialize_visitor: generics::FindTyParams,
}

impl GenericsOutput {
    fn new(generics: &Generics) -> Self {
        Self {
            overrides: vec![],
            serialize_visitor: generics::FindTyParams::new(generics),
        }
    }
    fn extend<const IS_ASYNC: bool>(self, where_clause: &mut syn::WhereClause, cratename: &Path) {
        let trait_path: Path = if IS_ASYNC {
            parse_quote! { #cratename::ser::BorshSerializeAsync }
        } else {
            parse_quote! { #cratename::ser::BorshSerialize }
        };
        let predicates =
            generics::compute_predicates(self.serialize_visitor.process_for_bounds(), &trait_path);
        where_clause.predicates.extend(predicates);
        where_clause.predicates.extend(self.overrides);
    }
}

pub enum FieldId {
    Struct(Ident),
    StructUnnamed(Index),
    Enum(Ident),
    EnumUnnamed(Index),
}

impl FieldId {
    fn index(field_idx: usize) -> syn::Result<Index> {
        let index = u32::try_from(field_idx).map_err(|err| {
            syn::Error::new(
                Span::call_site(),
                format!("up to 2^32 fields are supported {}", err),
            )
        })?;
        Ok(Index {
            index,
            span: Span::call_site(),
        })
    }
    pub fn new_struct_unnamed(field_idx: usize) -> syn::Result<Self> {
        let index = Self::index(field_idx)?;
        let result = Self::StructUnnamed(index);
        Ok(result)
    }
    pub fn new_enum_unnamed(field_idx: usize) -> syn::Result<Self> {
        let index = Self::index(field_idx)?;
        let result = Self::EnumUnnamed(index);
        Ok(result)
    }
}

impl FieldId {
    fn serialize_arg(&self) -> Expr {
        match self {
            Self::Struct(name) => parse_quote! { &self.#name },
            Self::StructUnnamed(index) => parse_quote! { &self.#index },
            Self::Enum(name) => parse_quote! { #name },
            Self::EnumUnnamed(ind) => {
                let field = Ident::new(&format!("id{}", ind.index), Span::mixed_site());
                parse_quote! { #field }
            }
        }
    }

    /// function which computes derive output [proc_macro2::TokenStream]
    /// of code, which serializes single field
    pub fn serialize_output<const IS_ASYNC: bool>(
        &self,
        field_type: &Type,
        cratename: &Path,
        serialize_with: Option<ExprPath>,
    ) -> TokenStream2 {
        let arg: Expr = self.serialize_arg();
        let dot_await = IS_ASYNC.then(|| quote! { .await });
        if let Some(func) = serialize_with {
            quote! { #func(#arg, writer)#dot_await?; }
        } else {
            let serialize_trait = Ident::new(
                if IS_ASYNC {
                    "BorshSerializeAsync"
                } else {
                    "BorshSerialize"
                },
                Span::call_site(),
            );
            quote! { <#field_type as #cratename::#serialize_trait>::serialize(#arg, writer)#dot_await?; }
        }
    }

    pub fn enum_variant_header(&self, skipped: bool) -> Option<TokenStream2> {
        match self {
            Self::Struct(..) | Self::StructUnnamed(..) => unreachable!("no variant header"),
            Self::Enum(name) => (!skipped).then_some(quote! { #name, }),
            Self::EnumUnnamed(index) => {
                let field_ident = Ident::new(
                    &if skipped {
                        format!("_id{}", index.index)
                    } else {
                        format!("id{}", index.index)
                    },
                    Span::mixed_site(),
                );
                Some(quote! { #field_ident, })
            }
        }
    }
}
