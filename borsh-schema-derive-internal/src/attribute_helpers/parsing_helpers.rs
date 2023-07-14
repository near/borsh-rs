// TODO: remove this unused attribute, when the unsplit is done
#![allow(unused)]
use std::iter::FromIterator;

use syn::{meta::ParseNestedMeta, punctuated::Punctuated, token::Paren, Expr, Lit, LitStr, Token};

use super::{Bounds, Symbol, BOUND, DESERIALIZE, SERIALIZE};
fn get_lit_str2(
    attr_name: Symbol,
    meta_item_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<LitStr>> {
    let expr: Expr = meta.value()?.parse()?;
    let mut value = &expr;
    while let Expr::Group(e) = value {
        value = &e.expr;
    }
    if let Expr::Lit(syn::ExprLit {
        lit: Lit::Str(lit), ..
    }) = value
    {
        Ok(Some(lit.clone()))
    } else {
        Err(syn::Error::new_spanned(
            expr,
            format!(
                "expected borsh {} attribute to be a string: `{} = \"...\"`",
                attr_name.0, meta_item_name.0
            ),
        ))
    }
}

fn parse_lit_into_where(
    attr_name: Symbol,
    meta_item_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Vec<syn::WherePredicate>> {
    let string = match get_lit_str2(attr_name, meta_item_name, meta)? {
        Some(string) => string,
        None => return Ok(Vec::new()),
    };

    match string.parse_with(Punctuated::<syn::WherePredicate, Token![,]>::parse_terminated) {
        Ok(predicates) => Ok(Vec::from_iter(predicates)),
        Err(err) => Err(syn::Error::new_spanned(string, err)),
    }
}

fn get_ser_and_de<T, F, R>(
    attr_name: Symbol,
    meta: &ParseNestedMeta,
    f: F,
) -> syn::Result<(Option<T>, Option<T>)>
where
    T: Clone,
    F: Fn(Symbol, Symbol, &ParseNestedMeta) -> syn::Result<R>,
    R: Into<Option<T>>,
{
    let mut ser_meta: Option<T> = None;
    let mut de_meta = None;

    let lookahead = meta.input.lookahead1();
    if lookahead.peek(Paren) {
        meta.parse_nested_meta(|meta| {
            if meta.path == SERIALIZE {
                if let Some(v) = f(attr_name, SERIALIZE, &meta)?.into() {
                    ser_meta = Some(v);
                }
            } else if meta.path == DESERIALIZE {
                if let Some(v) = f(attr_name, DESERIALIZE, &meta)?.into() {
                    de_meta = Some(v);
                }
            } else {
                return Err(meta.error(format_args!(
                    "malformed {0} attribute, expected `{0}(serialize = ..., deserialize = ...)`",
                    attr_name.0,
                )));
            }
            Ok(())
        })?;
    } else {
        return Err(lookahead.error());
    }

    Ok((ser_meta, de_meta))
}
pub fn get_where_predicates(meta: &ParseNestedMeta) -> syn::Result<(Bounds, Bounds)> {
    get_ser_and_de(BOUND, meta, parse_lit_into_where)
}