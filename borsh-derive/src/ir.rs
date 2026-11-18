//! Parse a `DeriveInput` once into a small IR that all three derives share. A struct is an
//! enum with one untagged variant, so every emitter handles one shape. Attribute errors keep
//! their established wording and spans.
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::{
    ext::IdentExt, meta::ParseNestedMeta, parse::Parse, punctuated::Punctuated, spanned::Spanned,
    Attribute, Data, DeriveInput, Error, Expr, ExprLit, ExprPath, Ident, Lit, LitStr, Member, Path,
    Result, Token, Type, WherePredicate,
};

pub struct Item {
    pub ident: Ident,
    pub generics: syn::Generics,
    pub krate: Path,
    pub is_enum: bool,
    /// Unions: the serialize and deserialize derives panic on them and the schema derive
    /// rejects them. No fields are parsed.
    pub is_union: bool,
    pub variants: Vec<Variant>,
    /// `init = ...`, checked to be a path by the deserialize emitter only.
    pub init: Option<Expr>,
    pub use_discriminant: bool,
    /// The type rustc evaluates discriminants in: the integer `#[repr]`, or `isize`.
    pub repr: Ident,
}

pub struct Variant {
    pub ident: Ident,
    pub discriminant: Option<Expr>,
    /// Named fields. Read by the schema emitter only.
    #[cfg_attr(not(feature = "schema"), allow(dead_code))]
    pub named: bool,
    pub fields: Vec<Field>,
}

pub struct Field {
    /// The field as written, for error spans.
    #[cfg_attr(not(feature = "schema"), allow(dead_code))]
    pub src: syn::Field,
    pub member: Member,
    pub binding: Ident,
    pub skip: bool,
    pub ser_with: Option<ExprPath>,
    pub de_with: Option<ExprPath>,
    pub bound_ser: Option<Vec<WherePredicate>>,
    pub bound_de: Option<Vec<WherePredicate>>,
    /// `schema(params = "T => <T as X>::Y, ...")`.
    pub schema_params: Option<Vec<(Ident, Type)>>,
    /// `schema(with_funcs(declaration = ..., definitions = ...))`.
    pub schema_with: Option<(ExprPath, ExprPath)>,
    /// `with_funcs(...)` was given, even empty: it can't be combined with `skip`.
    pub with_funcs: bool,
}

type Keys = &'static [(&'static str, &'static str)];
const FIELD_KEYS: Keys = &[
    ("bound", "bound(...)"),
    ("deserialize_with", "deserialize_with = ..."),
    #[cfg(feature = "schema")]
    ("schema", "schema(...)"),
    ("serialize_with", "serialize_with = ..."),
    ("skip", "skip"),
];
const BOUND_KEYS: Keys = &[
    ("deserialize", "deserialize = ..."),
    ("serialize", "serialize = ..."),
];
const SCHEMA_KEYS: Keys = &[
    ("params", "params = ..."),
    ("with_funcs", "with_funcs(...)"),
];
const FUNCS_KEYS: Keys = &[
    ("declaration", "declaration = ..."),
    ("definitions", "definitions = ..."),
];

/// Find `meta`'s key in `keys`, reject repeats, then hand the key to `f`.
fn dispatch(
    attr: &str,
    keys: Keys,
    seen: &mut Vec<&str>,
    meta: &ParseNestedMeta,
    f: impl FnOnce(&'static str) -> Result<()>,
) -> Result<()> {
    let Some(&(key, _)) = keys.iter().find(|(k, _)| meta.path.is_ident(k)) else {
        let list = keys.iter().map(|(_, d)| *d).collect::<Vec<_>>().join(", ");
        return Err(meta.error(format_args!(
            "malformed {0} attribute, expected `{0}({1})`",
            attr, list
        )));
    };
    if seen.contains(&key) {
        return Err(meta.error(format_args!("duplicate `{}` attribute", key)));
    }
    seen.push(key);
    f(key)
}

/// Parse `name(key = ..., ...)` with the given keys.
fn parens(
    meta: &ParseNestedMeta,
    attr: &str,
    keys: Keys,
    mut f: impl FnMut(&'static str, &ParseNestedMeta) -> Result<()>,
) -> Result<()> {
    let la = meta.input.lookahead1();
    if !la.peek(syn::token::Paren) {
        return Err(la.error());
    }
    let mut seen = vec![];
    meta.parse_nested_meta(|m| dispatch(attr, keys, &mut seen, &m, |k| f(k, &m)))
}

fn lit_str(attr: &str, key: &str, meta: &ParseNestedMeta) -> Result<LitStr> {
    let e: Expr = meta.value()?.parse()?;
    let mut v = &e;
    while let Expr::Group(g) = v {
        v = &g.expr;
    }
    match v {
        Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) => Ok(s.clone()),
        _ => Err(Error::new_spanned(
            &e,
            format!(
                "expected borsh {} attribute to be a string: `{} = \"...\"`",
                attr, key
            ),
        )),
    }
}

fn lit<T: Parse>(attr: &str, key: &str, meta: &ParseNestedMeta) -> Result<T> {
    let s = lit_str(attr, key, meta)?;
    s.parse().map_err(|e| Error::new_spanned(&s, e))
}

fn lit_list<T: Parse>(attr: &str, key: &str, meta: &ParseNestedMeta) -> Result<Vec<T>> {
    let s = lit_str(attr, key, meta)?;
    let list = s
        .parse_with(Punctuated::<T, Token![,]>::parse_terminated)
        .map_err(|e| Error::new_spanned(&s, e))?;
    Ok(list.into_iter().collect())
}

struct Override(Ident, Type);
impl Parse for Override {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let param = input.parse()?;
        input.parse::<Token![=>]>()?;
        Ok(Override(param, input.parse()?))
    }
}

/// One key of a field's `#[borsh(...)]`.
fn field_key(out: &mut Field, key: &str, m: &ParseNestedMeta) -> Result<()> {
    match key {
        "skip" => out.skip = true,
        "serialize_with" => out.ser_with = Some(lit("borsh", key, m)?),
        "deserialize_with" => out.de_with = Some(lit("borsh", key, m)?),
        "bound" => parens(m, "bound", BOUND_KEYS, |k, m| {
            let preds = Some(lit_list("bound", k, m)?);
            match k {
                "serialize" => out.bound_ser = preds,
                _ => out.bound_de = preds,
            }
            Ok(())
        })?,
        _ => parens(m, "schema", SCHEMA_KEYS, |k, m| schema_key(out, k, m))?,
    }
    Ok(())
}

/// One key of `schema(...)`.
fn schema_key(out: &mut Field, key: &str, m: &ParseNestedMeta) -> Result<()> {
    if key == "params" {
        let list: Vec<Override> = lit_list("schema", key, m)?;
        out.schema_params = Some(list.into_iter().map(|o| (o.0, o.1)).collect());
        return Ok(());
    }
    out.with_funcs = true;
    let (mut decl, mut defs) = (None, None);
    parens(m, "with_funcs", FUNCS_KEYS, |k, m| {
        let path = Some(lit("with_funcs", k, m)?);
        match k {
            "declaration" => decl = path,
            _ => defs = path,
        }
        Ok(())
    })?;
    match (decl, defs) {
        (Some(a), Some(b)) => out.schema_with = Some((a, b)),
        (None, None) => {}
        _ => {
            let msg = "both `declaration = ...` and `definitions = ...` \
                       have to be specified at the same time";
            return Err(Error::new_spanned(&m.path, msg));
        }
    }
    Ok(())
}

fn field(i: usize, f: &syn::Field) -> Result<Field> {
    let mut out = Field {
        src: f.clone(),
        member: f
            .ident
            .clone()
            .map_or_else(|| Member::Unnamed(i.into()), Member::Named),
        binding: Ident::new(&format!("__borsh_f{}", i), Span::mixed_site()),
        skip: false,
        ser_with: None,
        de_with: None,
        bound_ser: None,
        bound_de: None,
        schema_params: None,
        schema_with: None,
        with_funcs: false,
    };
    let attrs: Vec<&Attribute> = f
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("borsh"))
        .collect();
    let mut seen = vec![];
    for a in &attrs {
        a.parse_nested_meta(|m| {
            dispatch("borsh", FIELD_KEYS, &mut seen, &m, |k| {
                field_key(&mut out, k, &m)
            })
        })?;
    }
    // `skip` was given in one of `attrs`, so a clash has a first attribute to point at.
    let clash = if !out.skip {
        None
    } else if out.ser_with.is_some() || out.de_with.is_some() {
        Some("serialize_with` or `deserialize_with")
    } else if out.schema_params.is_some() {
        Some("schema(params = ...)")
    } else if out.with_funcs {
        Some("schema(with_funcs(...))")
    } else {
        None
    };
    if let (Some(first), Some(what)) = (attrs.first(), clash) {
        let msg = format!("`skip` cannot be used at the same time as `{}`", what);
        return Err(Error::new_spanned(first, msg));
    }
    Ok(out)
}

fn variant(ident: Ident, discriminant: Option<Expr>, fields: &syn::Fields) -> Result<Variant> {
    let named = matches!(fields, syn::Fields::Named(_));
    let fields = fields
        .iter()
        .enumerate()
        .map(|(i, f)| field(i, f))
        .collect::<Result<_>>()?;
    Ok(Variant {
        ident,
        discriminant,
        named,
        fields,
    })
}

/// The crate path: `#[borsh(crate = "...")]`, else the name `borsh` has in the user's
/// Cargo.toml (it may be renamed).
fn crate_from_cargo() -> Path {
    use proc_macro_crate::{crate_name, FoundCrate};
    let found = crate_name("borsh")
        .unwrap_or_else(|err| panic!("`proc_macro_crate::crate_name` call error: {:#?}", err));
    let name = match &found {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    Ident::new(name, Span::call_site()).into()
}

pub fn parse(input: &DeriveInput) -> Result<Item> {
    let (mut krate, mut init, mut use_discriminant, mut seen) = (None, None, None, vec![]);
    for a in input.attrs.iter().filter(|a| a.path().is_ident("borsh")) {
        a.parse_nested_meta(|m| {
            let keys = ["crate", "use_discriminant", "init"];
            let Some(key) = keys.into_iter().find(|k| m.path.is_ident(k)) else {
                let msg = "`crate`, `use_discriminant` or `init` are the only supported \
                           attributes for `borsh`";
                return Err(Error::new(m.path.span(), msg));
            };
            if seen.contains(&key) {
                return Err(m.error(format_args!("duplicate `{}` attribute", key)));
            }
            seen.push(key);
            match key {
                "crate" => krate = Some(lit::<Path>("borsh", "crate", &m)?),
                "init" => init = Some(m.value()?.parse::<Expr>()?),
                _ => {
                    let e: Expr = m.value()?.parse()?;
                    if let Data::Struct(_) = input.data {
                        let msg = "borsh(use_discriminant=<bool>) does not support structs";
                        return Err(Error::new(input.ident.span(), msg));
                    }
                    use_discriminant = Some(e);
                }
            }
            Ok(())
        })?;
    }
    let mut item = Item {
        ident: input.ident.clone(),
        generics: input.generics.clone(),
        krate: krate.unwrap_or_else(crate_from_cargo),
        is_enum: false,
        is_union: matches!(input.data, Data::Union(_)),
        variants: vec![],
        init,
        use_discriminant: false,
        repr: Ident::new("isize", Span::call_site()),
    };
    match &input.data {
        Data::Struct(s) => item
            .variants
            .push(variant(input.ident.clone(), None, &s.fields)?),
        Data::Enum(e) => {
            item.is_enum = true;
            if e.variants.len() > 256 {
                let msg = "up to 256 enum variants are supported";
                return Err(Error::new(input.span(), msg));
            }
            item.use_discriminant = match use_discriminant {
                Some(e) => match e.to_token_stream().to_string().as_str() {
                    "true" => true,
                    "false" => false,
                    _ => {
                        return Err(Error::new(
                            e.span(),
                            "`use_discriminant` accepts only `true` or `false`",
                        ))
                    }
                },
                None if e.variants.iter().any(|v| v.discriminant.is_some()) => {
                    let msg = "You have to specify `#[borsh(use_discriminant=true)]` or \
                               `#[borsh(use_discriminant=false)]` for all enums with explicit \
                               discriminant";
                    return Err(Error::new(input.ident.span(), msg));
                }
                None => false,
            };
            let ints = [
                "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128",
                "isize",
            ];
            for a in input.attrs.iter().filter(|a| a.path().is_ident("repr")) {
                let metas = a.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated);
                // `#[repr(r#u8)]` is `#[repr(u8)]`.
                let ids = metas
                    .into_iter()
                    .flatten()
                    .filter_map(|m| m.path().get_ident().map(IdentExt::unraw));
                if let Some(int) = ids.into_iter().find(|i| ints.iter().any(|t| i == t)) {
                    item.repr = int;
                }
            }
            for v in &e.variants {
                item.variants.push(variant(
                    v.ident.clone(),
                    v.discriminant.as_ref().map(|d| d.1.clone()),
                    &v.fields,
                )?);
            }
        }
        Data::Union(_) => {}
    }
    Ok(item)
}

impl Item {
    fn path(&self, v: &Variant) -> TokenStream {
        let i = &v.ident;
        if self.is_enum {
            quote!(Self::#i)
        } else {
            quote!(Self)
        }
    }

    /// `Self::V { a: __borsh_f0, .. }`, binding the fields that aren't skipped.
    pub fn pattern(&self, v: &Variant) -> TokenStream {
        let path = self.path(v);
        let binds = v.fields.iter().filter(|f| !f.skip).map(|f| {
            let (m, b) = (&f.member, &f.binding);
            quote!(#m: #b)
        });
        quote!(#path { #(#binds,)* .. })
    }

    /// Construct a variant from per-field expressions, evaluated in declaration order.
    pub fn construct(
        &self,
        v: &Variant,
        mut value: impl FnMut(&Field) -> TokenStream,
    ) -> TokenStream {
        let path = self.path(v);
        let fields = v.fields.iter().map(|f| {
            let (m, e) = (&f.member, value(f));
            quote!(#m: #e)
        });
        quote!(#path { #(#fields,)* })
    }

    /// Each variant's discriminant as an expression of the `repr` type, evaluated in that type as
    /// rustc does (`B = 16 << 4` is 256, not a `u8` shift). An explicit one is parenthesized so it
    /// stays one expression wherever it is pasted; an implicit one is the previous one `+ 1`.
    /// `identity` types it without a `let`, whose name a constant in scope could turn into a
    /// pattern.
    fn discriminants(&self) -> Vec<TokenStream> {
        let repr = &self.repr;
        let mut next = quote!(0);
        let mut values = vec![];
        for v in &self.variants {
            let discriminant = v.discriminant.as_ref().map_or(next, |e| quote!((#e)));
            next = quote!(#discriminant + 1);
            values
                .push(quote!(::core::convert::identity::<::core::primitive::#repr>(#discriminant)));
        }
        values
    }

    /// Each variant's tag expression: the variant index or, with `use_discriminant`, its entry
    /// in the `__BORSH_TAGS` array that `tag_check` defines. The entry is read in a `const`, so
    /// it is a constant: indexing at run time can copy the whole array into each use's stack
    /// frame in unoptimized builds.
    pub fn tags(&self) -> Vec<TokenStream> {
        (0..self.variants.len())
            .map(|i| match self.use_discriminant {
                true => quote!({
                    const __BORSH_TAG: ::core::primitive::u8 = __BORSH_TAGS[#i];
                    __BORSH_TAG
                }),
                false => {
                    let i = i as u8;
                    quote!(#i)
                }
            })
            .collect()
    }

    /// With `use_discriminant`, the `__BORSH_TAGS` array: each discriminant cast to `u8`,
    /// failing to compile (E0080) unless it is in `0..=255`, so a tag can't wrap or collide at
    /// run time. Lints can't do this: they are capped in dependencies.
    ///
    /// The derive's emitters put their impls in a block nested inside the one holding this, so
    /// the names they declare can't change what a discriminant refers to.
    pub fn tag_check(&self) -> TokenStream {
        if !self.use_discriminant {
            return quote!();
        }
        let (name, n) = (&self.ident, self.variants.len());
        let checks = self
            .variants
            .iter()
            .zip(self.discriminants())
            .map(|(v, value)| {
                let msg = format!(
                    "borsh: the discriminant of `{}::{}` doesn't fit in a u8 tag (0..=255)",
                    name, v.ident
                );
                let panic = quote_spanned!(v.ident.span()=> ::core::panic!(#msg));
                quote! {{
                    if (#value as ::core::primitive::i128) < 0
                        || (#value as ::core::primitive::i128) > 255
                    {
                        #panic;
                    }
                    (#value as ::core::primitive::u8)
                }}
            });
        let ty = quote!([::core::primitive::u8; #n]);
        if self.generics.params.is_empty() {
            // In an impl for the enum, so a discriminant naming `Self` resolves. The reserved
            // names keep the helper trait from shadowing the enum or making `Self::NAME`
            // ambiguous.
            quote! {
                trait __BorshTags {
                    const __BORSH_TAGS: #ty;
                }
                impl __BorshTags for #name {
                    const __BORSH_TAGS: #ty = [#(#checks),*];
                }
                const __BORSH_TAGS: #ty = <#name as __BorshTags>::__BORSH_TAGS;
            }
        } else {
            // A generic enum's discriminants can't name `Self`.
            quote!(const __BORSH_TAGS: #ty = [#(#checks),*];)
        }
    }
}
