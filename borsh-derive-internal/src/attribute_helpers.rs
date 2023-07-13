use syn::{Attribute, Field, Path, WherePredicate};
pub mod parsing_helpers;
use parsing_helpers::get_where_predicates;

#[derive(Copy, Clone)]
pub struct Symbol(pub &'static str);

pub const BORSH: Symbol = Symbol("borsh");
pub const BOUND: Symbol = Symbol("bound");
pub const SERIALIZE: Symbol = Symbol("serialize");
pub const DESERIALIZE: Symbol = Symbol("deserialize");

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

pub fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("borsh_skip"))
}

pub fn contains_initialize_with(attrs: &[Attribute]) -> Option<Path> {
    for attr in attrs.iter() {
        if attr.path().is_ident("borsh_init") {
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

type Bounds = Option<Vec<WherePredicate>>;

pub fn parse_bounds(attrs: &[Attribute]) -> Result<(Bounds, Bounds), syn::Error> {
    let (mut ser, mut de): (Bounds, Bounds) = (None, None);
    for attr in attrs {
        if attr.path() != BORSH {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path == BOUND {
                // #[borsh(bound(serialize = "...", deserialize = "...", schema = "..."))]

                let (ser_parsed, de_parsed) = get_where_predicates(&meta)?;
                ser = ser_parsed;
                de = de_parsed;
            }
            Ok(())
        })?;
    }

    Ok((ser, de))
}

pub enum BoundType {
    Serialize,
    Deserialize,
}

pub fn get_bounds(field: &Field, ty: BoundType) -> Result<Bounds, syn::Error> {
    let (ser, de) = parse_bounds(&field.attrs)?;
    match ty {
        BoundType::Serialize => Ok(ser),
        BoundType::Deserialize => Ok(de),
    }
}

pub fn collect_override_bounds(
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
    use syn::ItemStruct;

    use super::{parse_bounds, Bounds};
    fn debug_print_bounds(bounds: Bounds) -> String {
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
        insta::assert_snapshot!(debug_print_bounds(ser));
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
        insta::assert_snapshot!(debug_print_bounds(ser));
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
}
