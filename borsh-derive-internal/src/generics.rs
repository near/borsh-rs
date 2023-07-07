use std::collections::HashSet;

use quote::{quote, ToTokens};
use syn::{
    punctuated::Pair, Field, GenericArgument, Generics, Ident, Macro, Path, PathArguments,
    PathSegment, ReturnType, Type, TypeParamBound, TypePath, WherePredicate,
};

pub fn compute_predicates(params: Vec<TypePath>, traitname: &Path) -> Vec<WherePredicate> {
    params
        .into_iter()
        .map(|param| {
            syn::parse2(quote! {
                #param: #traitname
            })
            .unwrap()
        })
        .collect()
}

// Remove the default from every type parameter because in the generated impls
// they look like associated types: "error: associated type bindings are not
// allowed here".
pub fn without_defaults(generics: &Generics) -> Generics {
    syn::Generics {
        params: generics
            .params
            .iter()
            .map(|param| match param {
                syn::GenericParam::Type(param) => syn::GenericParam::Type(syn::TypeParam {
                    eq_token: None,
                    default: None,
                    ..param.clone()
                }),
                _ => param.clone(),
            })
            .collect(),
        ..generics.clone()
    }
}

/// a Visitor-like struct, which helps determine, if a type parameter is found in field
pub struct FindTyParams<'ast> {
    // Set of all generic type parameters on the current struct . Initialized up front.
    all_type_params: HashSet<Ident>,

    // Set of generic type parameters used in fields for which filter
    // returns true . Filled in as the visitor sees them.
    relevant_type_params: HashSet<Ident>,

    // Fields whose type is an associated type of one of the generic type
    // parameters.
    associated_type_usage: Vec<&'ast TypePath>,
}

fn ungroup(mut ty: &Type) -> &Type {
    while let Type::Group(group) = ty {
        ty = &group.elem;
    }
    ty
}

impl<'ast> FindTyParams<'ast> {
    pub fn new(generics: &Generics) -> Self {
        let all_type_params = generics
            .type_params()
            .map(|param| param.ident.clone())
            .collect();

        FindTyParams {
            all_type_params,
            relevant_type_params: HashSet::new(),
            associated_type_usage: Vec::new(),
        }
    }
    pub fn process(self) -> Vec<TypePath> {
        let relevant_type_params = self.relevant_type_params;
        let associated_type_usage = self.associated_type_usage;
        let mut new_predicates: Vec<TypePath> = relevant_type_params
            .into_iter()
            .map(|id| TypePath {
                qself: None,
                path: id.into(),
            })
            .chain(associated_type_usage.into_iter().cloned())
            .collect();
        new_predicates.sort_by_key(|type_path| type_path.to_token_stream().to_string());
        new_predicates
    }
}
impl<'ast> FindTyParams<'ast> {
    pub fn visit_field(&mut self, field: &'ast Field) {
        if let Type::Path(ty) = ungroup(&field.ty) {
            if let Some(Pair::Punctuated(t, _)) = ty.path.segments.pairs().next() {
                if self.all_type_params.contains(&t.ident) {
                    self.associated_type_usage.push(ty);
                }
            }
        }
        self.visit_type(&field.ty);
    }

    fn visit_return_type(&mut self, return_type: &'ast ReturnType) {
        match return_type {
            ReturnType::Default => {}
            ReturnType::Type(_, output) => self.visit_type(output),
        }
    }

    fn visit_path_segment(&mut self, segment: &'ast PathSegment) {
        self.visit_path_arguments(&segment.arguments);
    }

    fn visit_path_arguments(&mut self, arguments: &'ast PathArguments) {
        match arguments {
            PathArguments::None => {}
            PathArguments::AngleBracketed(arguments) => {
                for arg in &arguments.args {
                    match arg {
                        GenericArgument::Type(arg) => self.visit_type(arg),
                        GenericArgument::AssocType(arg) => self.visit_type(&arg.ty),
                        GenericArgument::Lifetime(_)
                        | GenericArgument::Const(_)
                        | GenericArgument::AssocConst(_)
                        | GenericArgument::Constraint(_) => {}
                        #[cfg_attr(all(test, exhaustive), deny(non_exhaustive_omitted_patterns))]
                        _ => {}
                    }
                }
            }
            PathArguments::Parenthesized(arguments) => {
                for argument in &arguments.inputs {
                    self.visit_type(argument);
                }
                self.visit_return_type(&arguments.output);
            }
        }
    }

    fn visit_path(&mut self, path: &'ast Path) {
        if let Some(seg) = path.segments.last() {
            if seg.ident == "PhantomData" {
                // Hardcoded exception, because PhantomData<T> implements
                // Serialize and Deserialize and Schema whether or not T implements it.
                return;
            }
        }
        if path.leading_colon.is_none() && path.segments.len() == 1 {
            let id = &path.segments[0].ident;
            if self.all_type_params.contains(id) {
                self.relevant_type_params.insert(id.clone());
            }
        }
        for segment in &path.segments {
            self.visit_path_segment(segment);
        }
    }

    fn visit_type_param_bound(&mut self, bound: &'ast TypeParamBound) {
        match bound {
            TypeParamBound::Trait(bound) => self.visit_path(&bound.path),
            TypeParamBound::Lifetime(_) | TypeParamBound::Verbatim(_) => {}
            #[cfg_attr(all(test, exhaustive), deny(non_exhaustive_omitted_patterns))]
            _ => {}
        }
    }
    // Type parameter should not be considered used by a macro path.
    //
    //     struct TypeMacro<T> {
    //         mac: T!(),
    //         marker: PhantomData<T>,
    //     }
    fn visit_macro(&mut self, _mac: &'ast Macro) {}

    fn visit_type(&mut self, ty: &'ast Type) {
        match ty {
            Type::Array(ty) => self.visit_type(&ty.elem),
            Type::BareFn(ty) => {
                for arg in &ty.inputs {
                    self.visit_type(&arg.ty);
                }
                self.visit_return_type(&ty.output);
            }
            Type::Group(ty) => self.visit_type(&ty.elem),
            Type::ImplTrait(ty) => {
                for bound in &ty.bounds {
                    self.visit_type_param_bound(bound);
                }
            }
            Type::Macro(ty) => self.visit_macro(&ty.mac),
            Type::Paren(ty) => self.visit_type(&ty.elem),
            Type::Path(ty) => {
                if let Some(qself) = &ty.qself {
                    self.visit_type(&qself.ty);
                }
                self.visit_path(&ty.path);
            }
            Type::Ptr(ty) => self.visit_type(&ty.elem),
            Type::Reference(ty) => self.visit_type(&ty.elem),
            Type::Slice(ty) => self.visit_type(&ty.elem),
            Type::TraitObject(ty) => {
                for bound in &ty.bounds {
                    self.visit_type_param_bound(bound);
                }
            }
            Type::Tuple(ty) => {
                for elem in &ty.elems {
                    self.visit_type(elem);
                }
            }

            Type::Infer(_) | Type::Never(_) | Type::Verbatim(_) => {}

            #[cfg_attr(all(test, exhaustive), deny(non_exhaustive_omitted_patterns))]
            _ => {}
        }
    }
}
