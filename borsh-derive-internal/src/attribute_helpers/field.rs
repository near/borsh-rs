#![allow(unused)]
// TODO: remove unused when unsplit is done
use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use syn::{meta::ParseNestedMeta, Attribute, ExprPath, WherePredicate};

use self::{bounds::BOUNDS_FIELD_PARSE_MAP, schema::SCHEMA_FIELD_PARSE_MAP};

use super::{
    parsing::{attr_get_by_symbol_keys, meta_get_by_symbol_keys, parse_lit_into},
    BoundType, Symbol, BORSH, BOUND, DESERIALIZE_WITH, PARAMS, SCHEMA, SERIALIZE_WITH, SKIP,
    WITH_FUNCS,
};

pub mod bounds;
pub mod schema;

enum Variants {
    Schema(schema::Attributes),
    Bounds(bounds::Bounds),
    SerializeWith(syn::ExprPath),
    DeserializeWith(syn::ExprPath),
}

type ParseFn = dyn Fn(Symbol, Symbol, &ParseNestedMeta) -> syn::Result<Variants> + Send + Sync;

static BORSH_FIELD_PARSE_MAP: Lazy<BTreeMap<Symbol, Box<ParseFn>>> = Lazy::new(|| {
    let mut m = BTreeMap::new();
    // has to be inlined; assigning closure separately doesn't work
    let f1: Box<ParseFn> = Box::new(|_attr_name, _meta_item_name, meta| {
        let map_result = meta_get_by_symbol_keys(BOUND, meta, &BOUNDS_FIELD_PARSE_MAP)?;
        let bounds_attributes: bounds::Bounds = map_result.into();
        Ok(Variants::Bounds(bounds_attributes))
    });

    let f2: Box<ParseFn> = Box::new(|_attr_name, _meta_item_name, meta| {
        let map_result = meta_get_by_symbol_keys(SCHEMA, meta, &SCHEMA_FIELD_PARSE_MAP)?;
        let schema_attributes: schema::Attributes = map_result.into();
        Ok(Variants::Schema(schema_attributes))
    });

    let f3: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
            .map(Variants::SerializeWith)
    });

    let f4: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
            .map(Variants::DeserializeWith)
    });

    m.insert(BOUND, f1);
    m.insert(SCHEMA, f2);
    m.insert(SERIALIZE_WITH, f3);
    m.insert(DESERIALIZE_WITH, f4);
    m
});

#[derive(Default)]
pub(crate) struct Attributes {
    pub bounds: Option<bounds::Bounds>,
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
        let bounds = bounds.map(|variant| match variant {
            Variants::Bounds(bounds) => bounds,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });
        let schema = schema.map(|variant| match variant {
            Variants::Schema(schema) => schema,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });

        let serialize_with = serialize_with.map(|variant| match variant {
            Variants::SerializeWith(serialize_with) => serialize_with,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });

        let deserialize_with = deserialize_with.map(|variant| match variant {
            Variants::DeserializeWith(deserialize_with) => deserialize_with,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });
        Self {
            bounds,
            schema,
            serialize_with,
            deserialize_with,
        }
    }
}
impl Attributes {
    pub(crate) fn parse(attrs: &[Attribute], skipped: bool) -> Result<Self, syn::Error> {
        let mut ref_attr: Option<&Attribute> = None;
        let mut map_result = BTreeMap::new();
        for attr in attrs {
            if attr.path() != BORSH {
                continue;
            }
            ref_attr = Some(attr);

            map_result = attr_get_by_symbol_keys(BORSH, attr, &BORSH_FIELD_PARSE_MAP)?;
        }

        let result: Self = map_result.into();
        if skipped && (result.serialize_with.is_some() || result.deserialize_with.is_some()) {
            return Err(syn::Error::new_spanned(
                ref_attr.unwrap(),
                format!(
                    "`{}` cannot be used at the same time as `{}` or `{}`",
                    SKIP.0, SERIALIZE_WITH.0, DESERIALIZE_WITH.0
                ),
            ));
        }
        if let Some(ref schema) = result.schema {
            if skipped && schema.params.is_some() {
                return Err(syn::Error::new_spanned(
                    ref_attr.unwrap(),
                    format!(
                        "`{}` cannot be used at the same time as `{}({})`",
                        SKIP.0, SCHEMA.0, PARAMS.1
                    ),
                ));
            }

            if skipped && schema.with_funcs.is_some() {
                return Err(syn::Error::new_spanned(
                    ref_attr.unwrap(),
                    format!(
                        "`{}` cannot be used at the same time as `{}({})`",
                        SKIP.0, SCHEMA.0, WITH_FUNCS.1
                    ),
                ));
            }
        }
        Ok(result)
    }
    pub(crate) fn needs_bounds_derive(&self, ty: BoundType) -> bool {
        let predicates = self.get_bounds(ty);
        if predicates.is_some() {
            return false;
        }
        true
    }

    pub(crate) fn needs_schema_params_derive(&self) -> bool {
        if let Some(ref schema) = self.schema {
            if schema.params.is_some() {
                return false;
            }
        }
        true
    }
    pub(crate) fn schema_declaration(&self) -> Option<ExprPath> {
        self.schema.as_ref().and_then(|schema| {
            schema
                .with_funcs
                .as_ref()
                .and_then(|with_funcs| with_funcs.declaration.clone())
        })
    }

    pub(crate) fn schema_definitions(&self) -> Option<ExprPath> {
        self.schema.as_ref().and_then(|schema| {
            schema
                .with_funcs
                .as_ref()
                .and_then(|with_funcs| with_funcs.definitions.clone())
        })
    }

    fn get_bounds(&self, ty: BoundType) -> Option<Vec<WherePredicate>> {
        let bounds = self.bounds.as_ref();
        bounds.and_then(|bounds| match ty {
            BoundType::Serialize => bounds.serialize.clone(),
            BoundType::Deserialize => bounds.deserialize.clone(),
        })
    }
    pub(crate) fn collect_bounds(&self, ty: BoundType) -> Vec<WherePredicate> {
        let predicates = self.get_bounds(ty);
        predicates.unwrap_or(vec![])
    }
}

#[cfg(test)]
mod tests {
    use quote::{quote, ToTokens};
    use std::fmt::Write;
    use syn::{Attribute, ItemStruct};

    fn parse_bounds(attrs: &[Attribute]) -> Result<Option<bounds::Bounds>, syn::Error> {
        // #[borsh(bound(serialize = "...", deserialize = "..."))]
        let borsh_attrs = Attributes::parse(attrs, false)?;
        Ok(borsh_attrs.bounds)
    }

    fn parse_schema_attrs(attrs: &[Attribute]) -> Result<Option<schema::Attributes>, syn::Error> {
        // #[borsh(schema(params = "..."))]
        let borsh_attrs = Attributes::parse(attrs, false)?;
        Ok(borsh_attrs.schema)
    }

    use super::{bounds, schema, Attributes};
    fn debug_print_vec_of_tokenizable<T: ToTokens>(optional: Option<Vec<T>>) -> String {
        let mut s = String::new();
        if let Some(vec) = optional {
            for element in vec {
                writeln!(&mut s, "{}", element.to_token_stream()).unwrap();
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
        let err = match Attributes::parse(&first_field.attrs, false) {
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
        let attrs = Attributes::parse(&first_field.attrs, false).unwrap();
        insta::assert_snapshot!(debug_print_tokenizable(attrs.serialize_with));
        insta::assert_snapshot!(debug_print_tokenizable(attrs.deserialize_with));
    }

    #[test]
    fn test_schema_with_funcs_parsing() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(schema(with_funcs(
                    declaration = "third_party_impl::declaration::<K, V>",
                    definitions = "third_party_impl::add_definitions_recursively::<K, V>"
                )))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = Attributes::parse(&first_field.attrs, false).unwrap();
        let schema = attrs.schema.unwrap();
        let with_funcs = schema.with_funcs.unwrap();

        insta::assert_snapshot!(debug_print_tokenizable(with_funcs.declaration));
        insta::assert_snapshot!(debug_print_tokenizable(with_funcs.definitions));
    }

    // both `declaration` and `definitions` have to be specified
    #[test]
    fn test_schema_with_funcs_parsing_error() {
        let item_struct: ItemStruct = syn::parse2(quote! {
            struct A {
                #[borsh(schema(with_funcs(
                    declaration = "third_party_impl::declaration::<K, V>"
                )))]
                x: u64,
                y: String,
            }
        })
        .unwrap();

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = Attributes::parse(&first_field.attrs, false);

        let err = match attrs {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
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

        let attrs = Attributes::parse(&first_field.attrs, false).unwrap();
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

        let err = match Attributes::parse(&first_field.attrs, false) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        insta::assert_debug_snapshot!(err);
    }
}
