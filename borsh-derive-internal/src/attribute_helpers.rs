// TODO: remove this unused attribute, when the unsplit is done
#![allow(unused)]
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{spanned::Spanned, Attribute, DeriveInput, Expr, Field, ItemEnum, Path, WherePredicate};
pub mod parsing_helpers;
use parsing_helpers::get_where_predicates;

use self::parsing_helpers::{get_schema_attrs, SchemaParamsOverride};

#[derive(Copy, Clone)]
pub struct Symbol(pub &'static str);

/// borsh - top level prefix in nested meta attribute
pub const BORSH: Symbol = Symbol("borsh");
/// bound - sub-borsh nested meta, field-level only, `BorshSerialize` and `BorshDeserialize` contexts
pub const BOUND: Symbol = Symbol("bound");
// item level attribute for enums
pub const USE_DISCRIMINANT: &str = "use_discriminant";
/// serialize - sub-bound nested meta attribute
pub const SERIALIZE: Symbol = Symbol("serialize");
/// deserialize - sub-bound nested meta attribute
pub const DESERIALIZE: Symbol = Symbol("deserialize");
/// borsh_skip - field-level only attribute, `BorshSerialize`, `BorshDeserialize`, `BorshSchema` contexts
pub const SKIP: Symbol = Symbol("borsh_skip");
/// borsh_init - item-level only attribute  `BorshDeserialize` context
pub const INIT: Symbol = Symbol("borsh_init");
/// schema - sub-borsh nested meta, `BorshSchema` context
pub const SCHEMA: Symbol = Symbol("schema");
/// params - sub-schema nested meta, field-level only attribute
pub const PARAMS: Symbol = Symbol("params");

impl PartialEq<Symbol> for Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

impl<'a> PartialEq<Symbol> for &'a Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

pub(crate) fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path() == SKIP)
}

pub fn check_item_attributes(derive_input: &DeriveInput) -> Result<(), TokenStream> {
    for attr in &derive_input.attrs {
        if attr.path().is_ident("borsh") {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident(USE_DISCRIMINANT) {
                    return Err(syn::Error::new(
                        derive_input.ident.span(),
                        "`use_discriminant` is the only supported attribute for `borsh`",
                    ));
                }
                if meta.path.is_ident(USE_DISCRIMINANT) {
                    let _expr: Expr = meta.value()?.parse()?;
                    if let syn::Data::Struct(ref _data) = derive_input.data {
                        return Err(syn::Error::new(
                            derive_input.ident.span(),
                            "borsh(use_discriminant=<bool>) does not support structs",
                        ));
                    }
                }

                Ok(())
            })
            .map_err(|err| err.to_compile_error())?;
        }
    }
    Ok(())
}

pub(crate) fn contains_use_discriminant(input: &ItemEnum) -> Result<bool, syn::Error> {
    if input.variants.len() >= 256 {
        return Err(syn::Error::new(
            input.span(),
            "up to 256 enum variants are supported",
        ));
    }

    let attrs = &input.attrs;
    let mut use_discriminant = None;
    for attr in attrs {
        if attr.path().is_ident("borsh") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident(USE_DISCRIMINANT) {
                    let value_expr: Expr = meta.value()?.parse()?;
                    let value = value_expr.to_token_stream().to_string();
                    // this goes to contains_use_discriminant
                    match value.as_str() {
                        "true" => {
                            use_discriminant = Some(true);
                        }
                        "false" => use_discriminant = Some(false),
                        _ => {
                            return Err(syn::Error::new(
                                value_expr.span(),
                                "`use_discriminant` accept only `true` or `false`",
                            ));
                        }
                    };
                }

                Ok(())
            })?;
        }
    }
    let has_explicit_discriminants = input
        .variants
        .iter()
        .any(|variant| variant.discriminant.is_some());
    if has_explicit_discriminants && use_discriminant.is_none() {
        return Err(syn::Error::new(
                input.ident.span(),
                "You have to specify `#[borsh(use_discriminant=true)]` or `#[borsh(use_discriminant=false)]` for all structs that have enum with explicit discriminant",
            ));
    }
    Ok(use_discriminant.unwrap_or(false))
}

pub(crate) fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
    for attr in attrs.iter() {
        if attr.path() == INIT {
            let mut res = None;
            let _ = attr.parse_nested_meta(|meta| {
                res = Some(meta.path);
                Ok(())
            });
            return res;
        }
    }

    None
}

pub(crate) type Bounds = Option<Vec<WherePredicate>>;
pub(crate) type SchemaParams = Option<Vec<SchemaParamsOverride>>;

fn parse_bounds(attrs: &[Attribute]) -> Result<(Bounds, Bounds), syn::Error> {
    let (mut ser, mut de): (Bounds, Bounds) = (None, None);
    for attr in attrs {
        if attr.path() != BORSH {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path == BOUND {
                // #[borsh(bound(serialize = "...", deserialize = "..."))]

                let (ser_parsed, de_parsed) = get_where_predicates(&meta)?;
                ser = ser_parsed;
                de = de_parsed;
            }
            Ok(())
        })?;
    }

    Ok((ser, de))
}

pub(crate) fn parse_schema_attrs(attrs: &[Attribute]) -> Result<SchemaParams, syn::Error> {
    let mut params: SchemaParams = None;
    for attr in attrs {
        if attr.path() != BORSH {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path == SCHEMA {
                // #[borsh(schema(params = "..."))]

                let params_parsed = get_schema_attrs(&meta)?;
                params = params_parsed;
            }
            Ok(())
        })?;
    }

    Ok(params)
}
pub(crate) enum BoundType {
    Serialize,
    Deserialize,
}

pub(crate) fn get_bounds(field: &Field, ty: BoundType) -> Result<Bounds, syn::Error> {
    let (ser, de) = parse_bounds(&field.attrs)?;
    match ty {
        BoundType::Serialize => Ok(ser),
        BoundType::Deserialize => Ok(de),
    }
}

pub(crate) fn collect_override_bounds(
    field: &Field,
    ty: BoundType,
    output: &mut Vec<WherePredicate>,
) -> Result<bool, syn::Error> {
    let predicates = get_bounds(field, ty)?;
    match predicates {
        Some(predicates) => {
            output.extend(predicates);
            Ok(true)
        }
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use quote::{quote, ToTokens};
    use std::fmt::Write;
    use syn::{Item, ItemStruct};

    use crate::attribute_helpers::parse_schema_attrs;

    use super::{parse_bounds, Bounds};
    fn debug_print_bounds<T: ToTokens>(bounds: Option<Vec<T>>) -> String {
        let mut s = String::new();
        if let Some(bounds) = bounds {
            for bound in bounds {
                writeln!(&mut s, "{}", bound.to_token_stream()).unwrap();
            }
        } else {
            write!(&mut s, "None").unwrap();
        }
        s
    }

    #[test]
    fn test_bounds_parsing1() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash + Ord,
                     V: Eq + Ord",
                    serialize = "K: Hash + Eq + Ord,
                     V: Ord"
                ))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let (ser, de) = parse_bounds(&first_field.attrs).unwrap();
        insta::assert_snapshot!(debug_print_bounds(ser));
        insta::assert_snapshot!(debug_print_bounds(de));
    }

    #[test]
    fn test_bounds_parsing2() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash + Eq + borsh::de::BorshDeserialize,
                     V: borsh::de::BorshDeserialize",
                    serialize = "K: Hash + Eq + borsh::ser::BorshSerialize,
                     V: borsh::ser::BorshSerialize"
                ))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let (ser, de) = parse_bounds(&first_field.attrs).unwrap();
        insta::assert_snapshot!(debug_print_bounds(ser));
        insta::assert_snapshot!(debug_print_bounds(de));
    }

    #[test]
    fn test_bounds_parsing3() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash + Eq + borsh::de::BorshDeserialize,
                     V: borsh::de::BorshDeserialize",
                    serialize = ""
                ))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let (ser, de) = parse_bounds(&first_field.attrs).unwrap();
        assert_eq!(ser.unwrap().len(), 0);
        insta::assert_snapshot!(debug_print_bounds(de));
    }

    #[test]
    fn test_bounds_parsing4() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash"))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let (ser, de) = parse_bounds(&first_field.attrs).unwrap();
        assert!(ser.is_none());
        insta::assert_snapshot!(debug_print_bounds(de));
    }

    #[test]
    fn test_bounds_parsing_error() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(bound(deser = "K: Hash"))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
    }

    #[test]
    fn test_bounds_parsing_error2() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(bound(deserialize = "K Hash"))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
    }

    #[test]
    fn test_bounds_parsing_error3() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(bound(deserialize = 42))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
    }

    #[test]
    fn test_schema_params_parsing1() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh(schema(params =
                    "T => <T as TraitName>::Associated"
               ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let schema_params = parse_schema_attrs(&first_field.attrs).unwrap();
        insta::assert_snapshot!(debug_print_bounds(schema_params));
    }
    #[test]
    fn test_schema_params_parsing_error() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh(schema(params =
                    "T => <T as TraitName, W>::Associated"
               ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_schema_attrs(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
    }

    #[test]
    fn test_schema_params_parsing2() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh(schema(params =
                    "T => <T as TraitName>::Associated, V => Vec<V>"
               ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let schema_params = parse_schema_attrs(&first_field.attrs).unwrap();
        insta::assert_snapshot!(debug_print_bounds(schema_params));
    }
    #[test]
    fn test_schema_params_parsing3() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh(schema(params = "" ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let schema_params = parse_schema_attrs(&first_field.attrs).unwrap();
        assert_eq!(schema_params.unwrap().len(), 0);
    }

    #[test]
    fn test_schema_params_parsing4() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                field: <T as TraitName>::Associated,
                another: V,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let schema_params = parse_schema_attrs(&first_field.attrs).unwrap();
        assert!(schema_params.is_none());
    }

    use super::*;
    #[test]
    fn test_check_use_discriminant() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = false)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum);
        assert!(!actual.unwrap());
    }

    #[test]
    fn test_check_use_discriminant_true() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = true)]
            enum AWithUseDiscriminantTrue {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum);
        assert!(actual.unwrap());
    }

    #[test]
    fn test_check_use_discriminant_wrong_value() {
        let item_enum: ItemEnum = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = 111)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = contains_use_discriminant(&item_enum);
        let err = match actual {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
    }
    #[test]
    fn test_check_use_discriminant_on_struct() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = false)]
            struct AWithUseDiscriminantFalse {
                x: X,
                y: Y,
            }
        })
        .unwrap();
        let actual = check_item_attributes(&item_enum);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    fn test_check_use_borsh_skip_on_whole_struct() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(use_discriminant = false)]
            #[borsh_skip]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = check_item_attributes(&item_enum);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
    #[test]
    fn test_check_use_borsh_invalid_on_whole_struct() {
        let item_enum: DeriveInput = syn::parse2(quote! {
            #[derive(BorshDeserialize, Debug)]
            #[borsh(invalid)]
            enum AWithUseDiscriminantFalse {
                X,
                Y,
            }
        })
        .unwrap();
        let actual = check_item_attributes(&item_enum);
        insta::assert_snapshot!(actual.unwrap_err().to_token_stream().to_string());
    }
}
