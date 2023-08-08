use core::convert::TryFrom;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ExprPath;
use syn::{Expr, Ident, Index};

pub enum FieldID {
    StructNamed(Ident),
    StructUnnamed(Index),
    EnumVariantNamed(Ident),
    EnumVariantUnnamed(Index),
}

impl FieldID {
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
    pub fn new_struct_index(field_idx: usize) -> syn::Result<Self> {
        let index = Self::index(field_idx)?;
        let result = Self::StructUnnamed(index);
        Ok(result)
    }

    pub fn new_enum_index(field_idx: usize) -> syn::Result<Self> {
        let index = Self::index(field_idx)?;
        let result = Self::EnumVariantUnnamed(index);
        Ok(result)
    }
    fn serialize_arg(&self) -> Expr {
        match self {
            Self::StructNamed(name) => syn::parse2(quote! { &self.#name }).unwrap(),
            Self::StructUnnamed(index) => syn::parse2(quote! { &self.#index }).unwrap(),
            Self::EnumVariantNamed(name) => syn::parse2(quote! { #name }).unwrap(),
            Self::EnumVariantUnnamed(index) => {
                let field_ident =
                    Ident::new(format!("id{}", index.index).as_str(), Span::mixed_site());
                syn::parse2(quote! { #field_ident }).unwrap()
            }
        }
    }

    /// function which computes derive output [proc_macro2::TokenStream]
    /// of code, which serializes single field
    pub fn serialize_output(
        &self,
        cratename: &Ident,
        serialize_with: Option<ExprPath>,
    ) -> TokenStream2 {
        let arg: Expr = self.serialize_arg();
        if let Some(func) = serialize_with {
            quote! {
                #func(#arg, writer)?;
            }
        } else {
            quote! {
                #cratename::BorshSerialize::serialize(#arg, writer)?;
            }
        }
    }

    pub fn enum_variant_header(&self, skipped: bool) -> Option<TokenStream2> {
        match self {
            Self::StructNamed(..) | Self::StructUnnamed(..) => None,
            Self::EnumVariantNamed(name) => {
                if !skipped {
                    Some(quote! { #name, })
                } else {
                    None
                }
            }
            Self::EnumVariantUnnamed(index) => {
                let field_ident = if skipped {
                    Ident::new(format!("_id{}", index.index).as_str(), Span::mixed_site())
                } else {
                    Ident::new(format!("id{}", index.index).as_str(), Span::mixed_site())
                };
                Some(quote! { #field_ident, })
            }
        }
    }
}
