//! The three emitters. Kept apart from the `proc_macro` entry points so tests can call
//! them on `proc_macro2` input.
use crate::bounds::Uses;
use crate::ir::{Field, Item, Variant};
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
#[cfg(feature = "schema")]
use syn::{visit::Visit, visit_mut::VisitMut};
use syn::{Ident, Result, Type, WherePredicate};

fn local(name: &str) -> Ident {
    Ident::new(name, Span::mixed_site())
}

/// Wrap impls in `const _: () = { ... };`, declaring a function for each bound name. The
/// block calls them, so they aren't dead code: the output carries no allow attributes,
/// which a user's `#![forbid(...)]` would turn into errors. An enum's discriminant tags are
/// evaluated in an outer block, where those functions aren't in scope.
fn wrap(item: &Item, names: &[Ident], items: TokenStream) -> TokenStream {
    let inner = quote! {
        const _: () = {
            #( const fn #names() {} )*
            #items
            #( #names(); )*
        };
    };
    match item.tag_check() {
        check if check.is_empty() => inner,
        check => quote!(const _: () = { #check #inner };),
    }
}

fn type_params(item: &Item) -> Vec<Ident> {
    item.generics
        .type_params()
        .map(|p| p.ident.clone())
        .collect()
}

/// `#[automatically_derived] impl<..> Trait for Name<..> where <user's>, <inferred>, <extra>`.
fn header(item: &Item, tr: &TokenStream, inferred: Vec<TokenStream>) -> TokenStream {
    let name = &item.ident;
    let (ig, tg, wc) = item.generics.split_for_impl();
    let preds: Vec<TokenStream> = wc
        .iter()
        .flat_map(|w| w.predicates.iter().map(|p| p.to_token_stream()))
        .chain(inferred)
        .collect();
    let wc = (!preds.is_empty()).then(|| quote!(where #(#preds),*));
    quote!(#[automatically_derived] impl #ig #tr for #name #tg #wc)
}

fn bound_all(targets: Vec<Type>, tr: &TokenStream) -> impl Iterator<Item = TokenStream> + '_ {
    targets.into_iter().map(move |t| quote!(#t: #tr))
}

fn fields(item: &Item) -> impl Iterator<Item = &Field> {
    item.variants.iter().flat_map(|v| &v.fields)
}

/// Inferred predicates, in this order: `tr` on the type parameters that encoded fields use,
/// then (when `defaults`) `Default` on those that skipped fields use, then the `bound(...)`
/// predicates. Fields with their own `bound(...)` take no part in inference.
fn predicates(
    item: &Item,
    tr: &TokenStream,
    bound: fn(&Field) -> &Option<Vec<WherePredicate>>,
    defaults: bool,
) -> Vec<TokenStream> {
    let params = type_params(item);
    let (mut uses, mut skipped) = (Uses::new(&params), Uses::new(&params));
    skipped.default_elems = true;
    for f in fields(item).filter(|f| bound(f).is_none()) {
        if !f.skip {
            uses.field(&f.src.ty)
        } else if defaults {
            skipped.field(&f.src.ty)
        }
    }
    let default = quote!(::core::default::Default);
    bound_all(uses.bounds(), tr)
        .chain(bound_all(skipped.bounds(), &default))
        .chain(fields(item).flat_map(|f| bound(f).iter().flatten().map(|p| p.to_token_stream())))
        .collect()
}

/// `ENCODES_TO_ZERO_BYTES` for structs: every field that isn't skipped
/// encodes to zero bytes, and none uses a custom function.
fn zero_bytes(item: &Item, custom: fn(&Field) -> bool, helper: &str) -> TokenStream {
    if item.is_enum {
        return quote!();
    }
    let (k, s, helper) = (
        &item.krate,
        local("__borsh_s"),
        Ident::new(helper, Span::call_site()),
    );
    let live: Vec<&Field> = item.variants[0].fields.iter().filter(|f| !f.skip).collect();
    let value = if live.iter().any(|f| custom(f)) {
        quote!(false)
    } else if live.is_empty() {
        quote!(true)
    } else {
        let terms = live.iter().map(|f| {
            let m = &f.member;
            quote!(#k::__private::#helper(|#s: &Self| ::core::ptr::addr_of!(#s.#m)))
        });
        quote!(#(#terms)&&*)
    };
    quote!(const ENCODES_TO_ZERO_BYTES: ::core::primitive::bool = #value;)
}

pub fn serialize(item: &Item) -> Result<TokenStream> {
    if item.is_union {
        unimplemented!()
    }
    let (k, w, tag) = (&item.krate, local("__borsh_writer"), local("__borsh_tag"));
    let write_tag = item.is_enum.then(|| {
        let ids = item.variants.iter().map(|v| &v.ident);
        let tags = item.tags();
        quote! {
            let #tag: ::core::primitive::u8 = match self { #(Self::#ids { .. } => #tags,)* };
            #k::io::Write::write_all(#w, &[#tag])?;
        }
    });
    let arms = item.variants.iter().map(|v| {
        let pat = item.pattern(v);
        let writes = v.fields.iter().filter(|f| !f.skip).map(|f| {
            let b = &f.binding;
            match &f.ser_with {
                Some(p) => quote!(#p(#b, #w)?;),
                None => quote!(#k::BorshSerialize::serialize(#b, #w)?;),
            }
        });
        quote!(#pat => { #(#writes)* })
    });
    let tr = quote!(#k::ser::BorshSerialize);
    let head = header(item, &tr, predicates(item, &tr, |f| &f.bound_ser, false));
    let zero = zero_bytes(
        item,
        |f| f.ser_with.is_some(),
        "field_encodes_to_zero_bytes_ser",
    );
    let max = item
        .variants
        .iter()
        .map(|v| v.fields.len())
        .max()
        .unwrap_or(0);
    let names: Vec<Ident> = (0..max)
        .map(|i| local(&format!("__borsh_f{}", i)))
        .chain([w.clone(), tag, local("__borsh_s")])
        .collect();
    Ok(wrap(
        item,
        &names,
        quote! {
            #head {
                #zero
                fn serialize<__W: #k::io::Write>(
                    &self,
                    #w: &mut __W
                ) -> ::core::result::Result<(), #k::io::Error> {
                    #write_tag
                    match self { #(#arms)* }
                    ::core::result::Result::Ok(())
                }
            }
        },
    ))
}

pub fn deserialize(item: &Item) -> Result<TokenStream> {
    if item.is_union {
        unimplemented!()
    }
    let k = &item.krate;
    let (r, tag, value) = (
        local("__borsh_reader"),
        local("__borsh_tag"),
        local("__borsh_value"),
    );
    let build = |v: &Variant| {
        item.construct(v, |f| match (&f.de_with, f.skip) {
            (_, true) => quote!(::core::default::Default::default()),
            (Some(p), _) => quote!(#p(#r)?),
            (None, _) => quote!(#k::BorshDeserialize::deserialize_reader(#r)?),
        })
    };
    let (bind, init) = match &item.init {
        Some(e) => {
            let m = syn::parse2::<syn::Path>(e.to_token_stream())?;
            (quote!(let mut #value), quote!(#value.#m();))
        }
        None => (quote!(let #value), quote!()),
    };
    let tags = item.tags();
    let tr = quote!(#k::de::BorshDeserialize);
    let preds = predicates(item, &tr, |f| &f.bound_de, true);
    let sig = quote!(<__R: #k::io::Read>(#r: &mut __R));
    let ret = quote!(::core::result::Result<Self, #k::io::Error>);
    let impls = if item.is_enum {
        let arms = item.variants.iter().zip(&tags).map(|(v, t)| {
            let value = build(v);
            quote!(if #tag == #t { #value } else)
        });
        let head = header(item, &tr, preds.clone());
        let ext = header(item, &quote!(#k::de::EnumExt), preds);
        quote! {
            #head {
                fn deserialize_reader #sig -> #ret {
                    let #tag = <::core::primitive::u8 as #k::de::BorshDeserialize>
                        ::deserialize_reader(#r)?;
                    <Self as #k::de::EnumExt>::deserialize_variant(#r, #tag)
                }
            }
            #ext {
                fn deserialize_variant<__R: #k::io::Read>(
                    #r: &mut __R,
                    #tag: ::core::primitive::u8
                ) -> #ret {
                    #bind = #(#arms)* {
                        return ::core::result::Result::Err(#k::io::Error::new(
                            #k::io::ErrorKind::InvalidData,
                            #k::__private::maybestd::format!("Unexpected variant tag: {:?}", #tag),
                        ));
                    };
                    #init
                    ::core::result::Result::Ok(#value)
                }
            }
        }
    } else {
        let head = header(item, &tr, preds);
        let zero = zero_bytes(
            item,
            |f| f.de_with.is_some(),
            "field_encodes_to_zero_bytes_de",
        );
        let v = build(&item.variants[0]);
        quote! {
            #head {
                #zero
                fn deserialize_reader #sig -> #ret {
                    #bind = #v;
                    #init
                    ::core::result::Result::Ok(#value)
                }
            }
        }
    };
    Ok(wrap(item, &[r, tag, value, local("__borsh_s")], impls))
}

/// Replace the type `Self` with the enum's own type before inferring which generics a
/// variant uses.
#[cfg(feature = "schema")]
struct ReplaceSelf(Type);
#[cfg(feature = "schema")]
impl VisitMut for ReplaceSelf {
    fn visit_type_mut(&mut self, ty: &mut Type) {
        if let Type::Path(p) = ty {
            if p.qself.is_none() && p.path.is_ident("Self") {
                *ty = self.0.clone();
                return;
            }
        }
        syn::visit_mut::visit_type_mut(self, ty);
    }
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}
}

/// Earlier versions moved each enum variant's fields into a helper struct, where `Self` in a
/// macro or in a `schema` attribute named the helper instead of the enum. 1.8.1 compiled
/// such fields into a wrong schema; the fix stack rejected them (skipped ones excepted), and
/// so does this, with the same error.
/// `Self` inside a nested item names that item and is left alone.
#[cfg(feature = "schema")]
fn check_self(item: &Item, self_ty: &Type) -> Result<()> {
    fn mentions_self(tokens: TokenStream) -> bool {
        tokens.into_iter().any(|t| match t {
            proc_macro2::TokenTree::Ident(i) => i == "Self",
            proc_macro2::TokenTree::Group(g) => mentions_self(g.stream()),
            _ => false,
        })
    }
    /// `Self` in a macro or, with `paths`, in any path.
    struct FindSelf(bool, bool);
    impl Visit<'_> for FindSelf {
        fn visit_item(&mut self, _: &syn::Item) {}
        fn visit_macro(&mut self, m: &syn::Macro) {
            self.1 |= mentions_self(m.tokens.clone());
        }
        fn visit_path(&mut self, p: &syn::Path) {
            self.1 |= self.0 && p.segments.iter().any(|s| s.ident == "Self");
            syn::visit::visit_path(self, p);
        }
    }
    for f in fields(item).filter(|f| !f.skip) {
        // Check (and span) the field with `Self` types replaced.
        let mut field = f.src.clone();
        ReplaceSelf(self_ty.clone()).visit_type_mut(&mut field.ty);
        let (mut in_type, mut in_params) = (FindSelf(false, false), FindSelf(true, false));
        in_type.visit_type(&field.ty);
        for (_, ty) in f.schema_params.iter().flatten() {
            in_params.visit_type(ty);
        }
        let funcs = f.schema_with.iter().map(|(a, b)| quote!(#a #b));
        if in_type.1 || in_params.1 || mentions_self(quote!(#(#funcs)*)) {
            let msg = format!(
                "`BorshSchema` can't resolve `Self` in a macro or a `schema` attribute \
                 of an enum variant, use `{}` instead",
                item.ident
            );
            return Err(syn::Error::new_spanned(field, msg));
        }
    }
    Ok(())
}

#[cfg(feature = "schema")]
pub fn schema(item: &Item) -> Result<TokenStream> {
    if item.is_union {
        let msg = "Borsh schema does not support unions yet.";
        return Err(syn::Error::new(Span::call_site(), msg));
    }
    let (k, defs) = (&item.krate, local("__borsh_defs"));
    let (s, p) = (quote!(#k::schema), quote!(#k::__private::maybestd));
    let params = type_params(item);
    let consts: Vec<Ident> = item
        .generics
        .const_params()
        .map(|c| c.ident.clone())
        .collect();
    let name = item.ident.to_string();
    let (_, tg, _) = item.generics.split_for_impl();
    let ident = &item.ident;
    let self_ty: Type = syn::parse2(quote!(#ident #tg))?;
    if item.is_enum {
        check_self(item, &self_ty)?;
    }
    // Field types as bound inference sees them: in enums, `Self` is the enum's type.
    let seen = |f: &Field| {
        let mut ty = f.src.ty.clone();
        if item.is_enum {
            ReplaceSelf(self_ty.clone()).visit_type_mut(&mut ty);
        }
        ty
    };
    let visit = |uses: &mut Uses, f: &Field| match &f.schema_params {
        None => uses.field(&seen(f)),
        Some(list) => list
            .iter()
            .for_each(|(p, t)| uses.assoc(p.clone(), t.clone())),
    };
    let declaration = |name: &str, targets: Vec<Type>| {
        let parts: Vec<TokenStream> = targets
            .iter()
            .map(|t| quote!(<#t as #k::BorshSchema>::declaration()))
            .chain(consts.iter().map(|c| quote!(#p::format!("{:?}", #c))))
            .collect();
        if parts.is_empty() {
            quote!(::core::convert::Into::into(#name))
        } else {
            let parts = quote!(<[#p::string::String]>::join(&[#(#parts),*], ", "));
            quote!(#p::format!(r#"{}<{}>"#, #name, #parts))
        }
    };
    let fields_def = |v: &Variant| {
        let live: Vec<&Field> = v.fields.iter().filter(|f| !f.skip).collect();
        let decls = live.iter().map(|f| match &f.schema_with {
            Some((decl, _)) => quote!(#decl()),
            None => {
                let ty = &f.src.ty;
                quote!(<#ty as #k::BorshSchema>::declaration())
            }
        });
        match v.named {
            true if !live.is_empty() => {
                let names = live.iter().map(|f| f.member.to_token_stream().to_string());
                let named = quote!(#((::core::convert::Into::into(#names), #decls)),*);
                quote!(#s::Fields::NamedFields(#p::vec![#named]))
            }
            false if !live.is_empty() => {
                quote!(#s::Fields::UnnamedFields(#p::vec![#(#decls),*]))
            }
            _ => quote!(#s::Fields::Empty),
        }
    };
    let walks = |v: &Variant| -> Vec<TokenStream> {
        v.fields
            .iter()
            .filter(|f| !f.skip)
            .map(|f| match &f.schema_with {
                Some((_, add)) => quote!(#add(#defs);),
                None => {
                    let ty = &f.src.ty;
                    quote!(<#ty as #k::BorshSchema>::add_definitions_recursively(#defs);)
                }
            })
            .collect()
    };
    let mut uses = Uses::new(&params);
    fields(item)
        .filter(|f| !f.skip)
        .for_each(|f| visit(&mut uses, f));
    let self_decl = declaration(&name, uses.bounds());
    let tags = item.tags();
    let def = local("__borsh_def");
    let body = if item.is_enum {
        let (mut entries, mut adds) = (vec![], vec![]);
        for (v, t) in item.variants.iter().zip(&tags) {
            // Each variant is described as a struct `Enum__Variant` whose generics are the
            // enum's type parameters the variant mentions (all const parameters stay).
            // A parameter used only through a nested projection (`Vec<U::Out>`) counts too,
            // so a `schema(params = "U => <U as Tr>::Out")` on that field names the variant.
            // Without one, the projection isn't in either declaration, the same as a struct.
            let mut all = Uses::new(&params);
            all.phantom = true;
            all.nested_assoc = true;
            v.fields.iter().for_each(|f| all.field(&seen(f)));
            let used = all.used();
            let own: Vec<Ident> = params
                .iter()
                .filter(|p| used.contains(*p))
                .cloned()
                .collect();
            let mut mine = Uses::new(&own);
            v.fields
                .iter()
                .filter(|f| !f.skip)
                .for_each(|f| visit(&mut mine, f));
            let vdecl = declaration(&format!("{}__{}", name, v.ident), mine.bounds());
            let (fields, w, vname) = (fields_def(v), walks(v), v.ident.to_string());
            entries.push(quote! {(
                <::core::primitive::u8 as ::core::convert::From<::core::primitive::u8>>::from(#t)
                    as ::core::primitive::i64,
                ::core::convert::Into::into(#vname),
                #vdecl
            )});
            adds.push(quote! {
                #s::add_definition(#vdecl, #s::Definition::Struct { fields: #fields }, #defs);
                #(#w)*
            });
        }
        quote! {
            let #def = #s::Definition::Enum { tag_width: 1, variants: #p::vec![#(#entries),*] };
            #s::add_struct_definition::<Self>(#def, #defs, |#defs| { #(#adds)* });
        }
    } else {
        let v = &item.variants[0];
        let (fields, w) = (fields_def(v), walks(v));
        quote! {
            let #def = #s::Definition::Struct { fields: #fields };
            #s::add_struct_definition::<Self>(#def, #defs, |#defs| { #(#w)* });
        }
    };
    let tr = quote!(#k::BorshSchema);
    let head = header(item, &tr, bound_all(uses.bounds(), &tr).collect());
    Ok(wrap(
        item,
        &[defs.clone(), def],
        quote! {
            #head {
                fn declaration() -> #s::Declaration { #self_decl }
                fn add_definitions_recursively(
                    #defs: &mut #p::collections::BTreeMap<#s::Declaration, #s::Definition>
                ) {
                    #body
                }
            }
        },
    ))
}

/// The checks on `Self` in enum variant fields, the variant-count limit, and the attribute
/// errors.
#[cfg(feature = "schema")]
#[cfg(test)]
mod tests {
    use quote::quote;

    fn schema(input: proc_macro2::TokenStream) -> syn::Result<String> {
        let input = syn::parse2(quote!(#[borsh(crate = "borsh")] #input)).unwrap();
        super::schema(&crate::ir::parse(&input)?).map(|t| t.to_string())
    }

    const SELF_ERR: &str = "`BorshSchema` can't resolve `Self` in a macro or a `schema` \
        attribute of an enum variant, use `Tree` instead";

    /// The variant count is checked first, on the whole item, with the same message for all
    /// three derives.
    #[test]
    fn too_many_variants() {
        let variants = (0..257u32).map(|i| quote::format_ident!("V{}", i));
        let input: syn::DeriveInput =
            syn::parse2(quote!(#[borsh(crate = "borsh")] enum E { #(#variants),* })).unwrap();
        let err = crate::ir::parse(&input).err().unwrap();
        assert_eq!(err.to_string(), "up to 256 enum variants are supported");
    }

    #[test]
    fn self_in_macro_err() {
        let err = schema(quote!(
            enum Tree {
                Leaf(u8),
                Node(list!(Self)),
            }
        ))
        .unwrap_err();
        assert_eq!(err.to_string(), SELF_ERR);
    }

    #[test]
    fn self_in_with_funcs_err() {
        let err = schema(quote! {
            enum Tree {
                Leaf(u8),
                Node(
                    #[borsh(schema(with_funcs(
                        declaration = "children::declaration::<Self>",
                        definitions = "children::add_definitions_recursively::<Self>"
                    )))]
                    Children,
                ),
            }
        })
        .unwrap_err();
        assert_eq!(err.to_string(), SELF_ERR);
    }

    #[test]
    fn self_in_params_err() {
        let err = schema(quote! {
            enum Tree<T> {
                Leaf(T),
                Node(#[borsh(schema(params = "T => Vec<Self>"))] Wrapper<T>),
            }
        })
        .unwrap_err();
        assert_eq!(err.to_string(), SELF_ERR);
    }

    #[test]
    fn self_outside_macros_ok() {
        let input = quote! {
            enum E<T> {
                Data(T),
                Flag(#[borsh(serialize_with = "write_flag::<Self, _>")] u8),
                Boxed(Box<Self>),
            }
        };
        assert!(schema(input).is_ok());
    }

    #[test]
    fn self_in_local_impl_of_array_len() {
        let input = quote! {
            enum E {
                V([u8; {
                    struct Size;
                    impl Size {
                        const N: usize = 1;
                        const M: usize = Self::N;
                    }
                    Size::M
                }]),
            }
        };
        assert!(schema(input).is_ok());
    }

    #[test]
    fn self_in_nested_items_ok() {
        let out = schema(quote! {
            enum E {
                Macro([u8; {
                    struct Size;
                    impl Size {
                        const N: usize = 1;
                        const M: usize = ident!(Self::N);
                        fn get() -> Self { Size }
                    }
                    Size::M
                }]),
                Param(
                    #[borsh(schema(params = "T => [T; { trait Len { const N: usize = 1; \
                        fn n() -> usize { Self::N } } 1 }]"))]
                    Wrapper<T>,
                ),
            }
        })
        .unwrap();
        assert!(out.contains("fn get () -> Self"), "{}", out);
    }

    /// Each malformed attribute and the message it's rejected with. The messages are
    /// master's, except `init = a + b`, which master reports as "expected `,`".
    #[test]
    fn attribute_errors() {
        use crate::gen::{deserialize, schema, serialize};
        type Emit = fn(&crate::ir::Item) -> syn::Result<proc_macro2::TokenStream>;
        let cases: &[(Emit, proc_macro2::TokenStream, &str)] = &[
            (
                serialize,
                quote!(
                    struct S {
                        #[borsh(skipp)]
                        a: u8,
                    }
                ),
                "malformed borsh attribute, expected `borsh(bound(...), deserialize_with = ..., \
                 schema(...), serialize_with = ..., skip)`",
            ),
            (
                serialize,
                quote!(
                    struct S {
                        #[borsh(skip)]
                        #[borsh(skip)]
                        a: u8,
                    }
                ),
                "duplicate `skip` attribute",
            ),
            (
                serialize,
                quote!(
                    struct S<T> {
                        #[borsh(bound(serialize = 5))]
                        a: T,
                    }
                ),
                "expected borsh bound attribute to be a string: `serialize = \"...\"`",
            ),
            (
                schema,
                quote!(
                    struct S {
                        #[borsh(schema(with_funcs(declaration = "x")))]
                        a: u8,
                    }
                ),
                "both `declaration = ...` and `definitions = ...` have to be specified at the \
                 same time",
            ),
            (
                serialize,
                quote!(
                    struct S {
                        #[borsh(skip, serialize_with = "x")]
                        a: u8,
                    }
                ),
                "`skip` cannot be used at the same time as `serialize_with` or \
                 `deserialize_with`",
            ),
            (
                schema,
                quote!(
                    struct S<T> {
                        #[borsh(skip, schema(params = "T => T"))]
                        a: T,
                    }
                ),
                "`skip` cannot be used at the same time as `schema(params = ...)`",
            ),
            (
                schema,
                quote!(
                    struct S {
                        #[borsh(skip, schema(with_funcs(declaration = "x", definitions = "y")))]
                        a: u8,
                    }
                ),
                "`skip` cannot be used at the same time as `schema(with_funcs(...))`",
            ),
            (
                serialize,
                quote!(
                    #[borsh(use_discriminant = 1)]
                    enum E {
                        A,
                        B,
                    }
                ),
                "`use_discriminant` accepts only `true` or `false`",
            ),
            (
                serialize,
                quote!(
                    #[borsh(use_discriminant = true)]
                    struct S {
                        a: u8,
                    }
                ),
                "borsh(use_discriminant=<bool>) does not support structs",
            ),
            (
                serialize,
                quote!(
                    enum E {
                        A = 1,
                        B,
                    }
                ),
                "You have to specify `#[borsh(use_discriminant=true)]` or \
                 `#[borsh(use_discriminant=false)]` for all enums with explicit discriminant",
            ),
            (
                deserialize,
                quote!(
                    #[borsh(init = a + b)]
                    struct S {
                        a: u8,
                    }
                ),
                "unexpected token",
            ),
            (
                serialize,
                quote!(
                    #[borsh(foo = 1)]
                    struct S {
                        a: u8,
                    }
                ),
                "`crate`, `use_discriminant` or `init` are the only supported attributes for \
                 `borsh`",
            ),
            (
                serialize,
                quote!(
                    struct S<T> {
                        #[borsh(bound(ser = "T: Clone"))]
                        a: T,
                    }
                ),
                "malformed bound attribute, expected `bound(deserialize = ..., serialize = ...)`",
            ),
            (
                serialize,
                quote!(
                    struct S<T> {
                        #[borsh(bound(serialize = "T: :"))]
                        a: T,
                    }
                ),
                "expected `,`",
            ),
            (
                schema,
                quote!(
                    struct S<T> {
                        #[borsh(schema(params = "T -> u8"))]
                        a: T,
                    }
                ),
                "expected `=>`",
            ),
            (
                serialize,
                quote!(
                    struct S<T> {
                        #[borsh(bound)]
                        a: T,
                    }
                ),
                "unexpected end of input, expected parentheses",
            ),
            (
                schema,
                quote!(
                    struct S {
                        #[borsh(schema(foo = "x"))]
                        a: u8,
                    }
                ),
                "malformed schema attribute, expected `schema(params = ..., with_funcs(...))`",
            ),
            (
                serialize,
                quote!(
                    struct S {
                        #[borsh(serialize_with = "1 + 1")]
                        a: u8,
                    }
                ),
                "expected identifier",
            ),
            (
                schema,
                quote!(union U { a: u8 }),
                "Borsh schema does not support unions yet.",
            ),
        ];
        for (emit, input, msg) in cases {
            let input = syn::parse2(quote!(#[borsh(crate = "borsh")] #input)).unwrap();
            let err = crate::ir::parse(&input).and_then(|item| emit(&item));
            assert_eq!(err.err().map(|e| e.to_string()).as_deref(), Some(*msg));
        }
        let input = syn::parse2(quote!(
            #[borsh(crate = "x")]
            #[borsh(crate = "y")]
            struct S;
        ));
        let err = crate::ir::parse(&input.unwrap()).err().unwrap();
        assert_eq!(err.to_string(), "duplicate `crate` attribute");
        let input = syn::parse2(quote!(
            #[borsh(crate = 5)]
            struct S;
        ))
        .unwrap();
        let err = crate::ir::parse(&input).err().unwrap();
        assert_eq!(
            err.to_string(),
            "expected borsh borsh attribute to be a string: `crate = \"...\"`"
        );
    }
}
