//! Which type parameters a set of field types uses. These rules decide the inferred `where`
//! clauses (and schema declarations), so changing them changes which types compile:
//! - a parameter `T` used anywhere in a field type is bounded as `T: Trait`;
//! - a field type that *starts with* a parameter (`T::Assoc`, `T::A::B`) is bounded whole;
//! - `PhantomData<..>` contributes nothing, except to schema helper-parameter lists;
//! - macros contribute nothing;
//! - for `Default` bounds on skipped fields, the element types of `Vec`, `Option`, `HashMap`
//!   and `BTreeMap` are ignored, since those containers default without them.
use quote::ToTokens;
use std::collections::HashSet;
use syn::{GenericArgument, Ident, Path, PathArguments, ReturnType, Type, TypeParamBound};

pub struct Uses<'a> {
    params: &'a [Ident],
    relevant: HashSet<Ident>,
    assoc: Vec<(Ident, Type)>,
    pub phantom: bool,
    pub default_elems: bool,
    /// Count `T::Assoc` anywhere in a type as a use of `T`. Off for bound inference (which
    /// bounds only top-level projections); on when deciding a schema variant's parameters.
    pub nested_assoc: bool,
}

impl<'a> Uses<'a> {
    pub fn new(params: &'a [Ident]) -> Self {
        Uses {
            params,
            relevant: HashSet::new(),
            assoc: vec![],
            phantom: false,
            default_elems: false,
            nested_assoc: false,
        }
    }

    /// Record a field's type.
    pub fn field(&mut self, ty: &Type) {
        let mut top = ty;
        while let Type::Group(g) = top {
            top = &g.elem;
        }
        if let Type::Path(p) = top {
            if p.path.segments.len() > 1 && self.params.contains(&p.path.segments[0].ident) {
                self.assoc(p.path.segments[0].ident.clone(), ty.clone());
            }
        }
        self.ty(ty);
    }

    /// Record `ty` as a whole-type bound target attributed to `param`.
    pub fn assoc(&mut self, param: Ident, ty: Type) {
        self.assoc.push((param, ty));
    }

    /// Bound targets, in parameter order: each used parameter, then the whole types recorded
    /// for it, without repeats.
    pub fn bounds(&self) -> Vec<Type> {
        let (mut out, mut seen) = (vec![], HashSet::new());
        for p in self.params {
            let mine = self
                .assoc
                .iter()
                .filter(|(q, _)| q == p)
                .map(|(_, t)| t.clone());
            let own = self.relevant.contains(p).then(|| {
                Type::Path(syn::TypePath {
                    attrs: vec![],
                    qself: None,
                    path: p.clone().into(),
                })
            });
            for t in own.into_iter().chain(mine) {
                if seen.insert(t.to_token_stream().to_string()) {
                    out.push(t);
                }
            }
        }
        out
    }

    /// Parameters used at all (directly or through a whole-type target).
    #[cfg(feature = "schema")]
    pub fn used(&self) -> HashSet<Ident> {
        self.params
            .iter()
            .filter(|p| self.relevant.contains(*p) || self.assoc.iter().any(|(q, _)| q == *p))
            .cloned()
            .collect()
    }

    fn ty(&mut self, ty: &Type) {
        match ty {
            Type::Array(t) => self.ty(&t.elem),
            Type::Group(t) => self.ty(&t.elem),
            Type::Paren(t) => self.ty(&t.elem),
            Type::Ptr(t) => self.ty(&t.elem),
            Type::Reference(t) => self.ty(&t.elem),
            Type::Slice(t) => self.ty(&t.elem),
            Type::Tuple(t) => t.elems.iter().for_each(|e| self.ty(e)),
            Type::FnPtr(t) => {
                t.inputs.iter().for_each(|a| self.ty(&a.ty));
                self.ret(&t.output);
            }
            Type::ImplTrait(t) => t.bounds.iter().for_each(|b| self.bound(b)),
            Type::TraitObject(t) => t.bounds.iter().for_each(|b| self.bound(b)),
            Type::Path(t) => {
                if let Some(q) = &t.qself {
                    self.ty(&q.ty);
                }
                self.path(&t.path);
            }
            _ => {}
        }
    }

    fn ret(&mut self, r: &ReturnType) {
        if let ReturnType::Type(_, t) = r {
            self.ty(t);
        }
    }

    fn bound(&mut self, b: &TypeParamBound) {
        if let TypeParamBound::Trait(t) = b {
            self.path(&t.path);
        }
    }

    fn path(&mut self, path: &Path) {
        if let Some(last) = path.segments.last() {
            if self.default_elems {
                let skip = match last.ident.to_string().as_str() {
                    "Vec" | "Option" => 1,
                    "HashMap" | "BTreeMap" => 2,
                    _ => 0,
                };
                if let (true, PathArguments::AngleBracketed(a)) = (skip > 0, &last.arguments) {
                    for arg in a.args.iter().skip(skip) {
                        if let GenericArgument::Type(t) = arg {
                            self.ty(t);
                        }
                    }
                    return;
                }
            }
            if last.ident == "PhantomData" && !self.phantom {
                return;
            }
        }
        if path.leading_colon.is_none()
            && (path.segments.len() == 1 || self.nested_assoc)
            && self.params.contains(&path.segments[0].ident)
        {
            self.relevant.insert(path.segments[0].ident.clone());
        }
        for seg in &path.segments {
            match &seg.arguments {
                PathArguments::None => {}
                PathArguments::AngleBracketed(a) => a.args.iter().for_each(|arg| match arg {
                    GenericArgument::Type(t) => self.ty(t),
                    GenericArgument::AssocType(t) => self.ty(&t.ty),
                    _ => {}
                }),
                PathArguments::Parenthesized(a) => {
                    a.inputs.iter().for_each(|t| self.ty(&t.ty));
                    self.ret(&a.output);
                }
            }
        }
    }
}
