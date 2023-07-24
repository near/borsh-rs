use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use syn::{meta::ParseNestedMeta, Attribute, Field, WherePredicate};

use self::{bounds::BOUNDS_FIELD_PARSE_MAP, schema::SCHEMA_FIELD_PARSE_MAP};

use super::{
    parsing::{attr_get_by_symbol_keys, meta_get_by_symbol_keys, parse_lit_into},
    BoundType, Symbol, BORSH, BOUND, SCHEMA, SERIALIZE_WITH, DESERIALIZE_WITH,
};

pub mod bounds;
pub mod schema;

enum Variants {
    Schema(schema::Attributes),
    Bounds(bounds::Attributes),
    SerializeWith(syn::ExprPath),
    DeserializeWith(syn::ExprPath),
}

type ParseFn = dyn Fn(Symbol, Symbol, &ParseNestedMeta) -> syn::Result<Variants> + Send + Sync;

static BORSH_FIELD_PARSE_MAP: Lazy<BTreeMap<Symbol, Box<ParseFn>>> = Lazy::new(|| {
    let mut m = BTreeMap::new();
    // has to be inlined; assigning closure separately doesn't work
    let f1: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        let mut map_result = meta_get_by_symbol_keys(BOUND, meta, &BOUNDS_FIELD_PARSE_MAP)?;
        let bounds_attributes: bounds::Attributes = map_result.into();
        Ok(Variants::Bounds(bounds_attributes))
    });

    let f2: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        let mut map_result = meta_get_by_symbol_keys(SCHEMA, meta, &SCHEMA_FIELD_PARSE_MAP)?;
        let schema_attributes: schema::Attributes = map_result.into();
        Ok(Variants::Schema(schema_attributes))
    });

    let f3: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta).map(Variants::SerializeWith)
    });

    let f4: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta).map(Variants::DeserializeWith)
    });

    m.insert(BOUND, f1);
    m.insert(SCHEMA, f2);
    m.insert(SERIALIZE_WITH, f3);
    m.insert(DESERIALIZE_WITH, f4);
    m
});

#[derive(Default)]
pub(crate) struct Attributes {
    pub bounds: Option<bounds::Attributes>,
    pub schema: Option<schema::Attributes>,
    pub serialize_with: Option<syn::ExprPath>,
    pub deserialize_with: Option<syn::ExprPath>,
}

impl From<BTreeMap<Symbol, Variants>> for Attributes {
    fn from(mut map: BTreeMap<Symbol, Variants>) -> Self {
        let bounds = map.remove(&BOUND);
        let schema = map.remove(&SCHEMA);
        let serialize_with = map.remove(&SERIALIZE_WITH);
        let deserialize_with = map.remove(&DESERIALIZE_WITH);
        let bounds = bounds.and_then(|variant| match variant {
            Variants::Bounds(bounds) => Some(bounds),
            _ => None,
        });
        let schema = schema.and_then(|variant| match variant {
            Variants::Schema(schema) => Some(schema),
            _ => None,
        });

        let serialize_with = serialize_with.and_then(|variant| match variant {
            Variants::SerializeWith(serialize_with) => Some(serialize_with),
            _ => None,
        });

        let deserialize_with = deserialize_with.and_then(|variant| match variant {
            Variants::DeserializeWith(deserialize_with) => Some(deserialize_with),
            _ => None,
        });
        Self { bounds, schema, serialize_with, deserialize_with }
    }
}
impl Attributes {
    pub(crate) fn parse(attrs: &[Attribute]) -> Result<Self, syn::Error> {
        let mut map_result = BTreeMap::new();
        for attr in attrs {
            if attr.path() != BORSH {
                continue;
            }

            map_result = attr_get_by_symbol_keys(BORSH, attr, &BORSH_FIELD_PARSE_MAP)?;
        }

        Ok(map_result.into())
    }

    fn get_bounds(&self, ty: BoundType) -> Result<Bounds, syn::Error> {
        let attributes = self.bounds.as_ref();
        let r = attributes.and_then(|attributes| match ty {
            BoundType::Serialize => attributes.serialize.clone(),
            BoundType::Deserialize => attributes.deserialize.clone(),
        });
        Ok(r)
    }

    pub(crate) fn collect_override_bounds(
        &self,
        ty: BoundType,
        output: &mut Vec<WherePredicate>,
    ) -> Result<bool, syn::Error> {
        let predicates = self.get_bounds(ty)?;
        match predicates {
            Some(predicates) => {
                output.extend(predicates);
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

pub(crate) type Bounds = Option<Vec<WherePredicate>>;

#[cfg(test)]
mod tests {
    use quote::{quote, ToTokens};
    use std::fmt::Write;
    use syn::{Attribute, ItemStruct};

    fn parse_bounds(attrs: &[Attribute]) -> Result<Option<bounds::Attributes>, syn::Error> {
        // #[borsh(bound(serialize = "...", deserialize = "..."))]
        let borsh_attrs = Attributes::parse(attrs)?;
        Ok(borsh_attrs.bounds)
    }

    fn parse_schema_attrs(attrs: &[Attribute]) -> Result<Option<schema::Attributes>, syn::Error> {
        // #[borsh(schema(params = "..."))]
        let borsh_attrs = Attributes::parse(attrs)?;
        Ok(borsh_attrs.schema)
    }

    use super::{bounds, schema, Attributes, Bounds};
    fn debug_print_vec_of_tokenizable<T: ToTokens>(optional: Option<Vec<T>>) -> String {
        let mut s = String::new();
        if let Some(vec) = optional {
            for bound in vec {
                writeln!(&mut s, "{}", bound.to_token_stream()).unwrap();
            }
        } else {
            write!(&mut s, "None").unwrap();
        }
        s
    }

    fn debug_print_tokenizable<T: ToTokens>(optional: Option<T>) -> String {
        let mut s = String::new();
        if let Some(type_) = optional {
            writeln!(&mut s, "{}", type_.to_token_stream()).unwrap();
        } else {
            write!(&mut s, "None").unwrap();
        }
        s
    }

    #[test]
    fn test_root_error() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(boons)]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match Attributes::parse(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
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
        let attrs = parse_bounds(&first_field.attrs).unwrap().unwrap();
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(attrs.serialize));
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
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
        let attrs = parse_bounds(&first_field.attrs).unwrap().unwrap();
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(attrs.serialize));
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
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
        let attrs = parse_bounds(&first_field.attrs).unwrap().unwrap();
        assert_eq!(attrs.serialize.unwrap().len(), 0);
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
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
        let attrs = parse_bounds(&first_field.attrs).unwrap().unwrap();
        assert!(attrs.serialize.is_none());
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
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
        let schema_attrs = parse_schema_attrs(&first_field.attrs).unwrap();
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(schema_attrs.unwrap().params));
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
    fn test_schema_params_parsing_error2() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh(schema(paramsbum =
                    "T => <T as TraitName>::Associated"
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
        let schema_attrs = parse_schema_attrs(&first_field.attrs).unwrap();
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(schema_attrs.unwrap().params));
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
        let schema_attrs = parse_schema_attrs(&first_field.attrs).unwrap();
        assert_eq!(schema_attrs.unwrap().params.unwrap().len(), 0);
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
        let schema_attrs = parse_schema_attrs(&first_field.attrs).unwrap();
        assert!(schema_attrs.is_none());
    }

    #[test]
    fn test_ser_de_with_parsing1() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(
                    serialize_with = "third_party_impl::serialize_third_party",
                    deserialize_with = "third_party_impl::deserialize_third_party",
                )]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = Attributes::parse(&first_field.attrs).unwrap();
        insta::assert_snapshot!(debug_print_tokenizable(attrs.serialize_with));
        insta::assert_snapshot!(debug_print_tokenizable(attrs.deserialize_with));
    }

    #[test]
    fn test_root_bounds_and_params_combined() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(
                    serialize_with = "third_party_impl::serialize_third_party",
                    bound(deserialize = "K: Hash"),
                    schema(params = "T => <T as TraitName>::Associated, V => Vec<V>")
                )]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];

        let attrs = Attributes::parse(&first_field.attrs).unwrap();
        let bounds = attrs.bounds.unwrap();
        assert!(bounds.serialize.is_none());
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(bounds.deserialize));
        let schema = attrs.schema.unwrap();
        insta::assert_snapshot!(debug_print_vec_of_tokenizable(schema.params));
        insta::assert_snapshot!(debug_print_tokenizable(attrs.serialize_with));
        assert!(attrs.deserialize_with.is_none());
    }

    #[test]
    fn test_root_bounds_and_wrong_key_combined() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash"),
                        schhema(params = "T => <T as TraitName>::Associated, V => Vec<V>")
                )]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];

        let err = match Attributes::parse(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
    }
}
