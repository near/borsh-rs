use std::collections::BTreeMap;

use cfg_if::cfg_if;
use once_cell::sync::Lazy;
use syn::{meta::ParseNestedMeta, Attribute, WherePredicate};
#[cfg(feature = "schema")]
use {
    super::schema_keys::{PARAMS, SCHEMA, WITH_FUNCS},
    schema::SCHEMA_FIELD_PARSE_MAP,
};

use self::bounds::BOUNDS_FIELD_PARSE_MAP;
use super::{
    get_one_attribute,
    parsing::{attr_get_by_symbol_keys, meta_get_by_symbol_keys, parse_lit_into},
    BoundType, Symbol, BORSH, BOUND, DESERIALIZE_WITH, SERIALIZE_WITH, SKIP,
};
#[cfg(feature = "async")]
use super::{ASYNC_BOUND, DESERIALIZE_WITH_ASYNC, SERIALIZE_WITH_ASYNC};

pub mod bounds;
#[cfg(feature = "schema")]
pub mod schema;

enum Variants {
    Bounds(bounds::Bounds),
    SerializeWith(syn::ExprPath),
    DeserializeWith(syn::ExprPath),
    SerializeWithAsync(syn::ExprPath),
    DeserializeWithAsync(syn::ExprPath),
    Skip(()),
    #[cfg(feature = "schema")]
    Schema(schema::Attributes),
}

type ParseFn = dyn Fn(Symbol, Symbol, &ParseNestedMeta) -> syn::Result<Variants> + Send + Sync;

static BORSH_FIELD_PARSE_MAP: Lazy<BTreeMap<Symbol, Box<ParseFn>>> = Lazy::new(|| {
    let mut m = BTreeMap::new();
    // assigning closure `let f = |args| {...};` and boxing closure `let f: Box<ParseFn> = Box::new(f);`
    // on 2 separate lines doesn't work
    let f_bounds: Box<ParseFn> = Box::new(|_attr_name, _meta_item_name, meta| {
        let map_result = meta_get_by_symbol_keys(BOUND, meta, &BOUNDS_FIELD_PARSE_MAP)?;
        let bounds_attributes: bounds::Bounds = map_result.into();
        Ok(Variants::Bounds(bounds_attributes))
    });

    #[cfg(feature = "async")]
    let f_async_bounds: Box<ParseFn> = Box::new(|_attr_name, _meta_item_name, meta| {
        let map_result = meta_get_by_symbol_keys(ASYNC_BOUND, meta, &BOUNDS_FIELD_PARSE_MAP)?;
        let bounds_attributes: bounds::Bounds = map_result.into();
        Ok(Variants::Bounds(bounds_attributes))
    });

    let f_serialize_with: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
            .map(Variants::SerializeWith)
    });

    let f_deserialize_with: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
            .map(Variants::DeserializeWith)
    });

    #[cfg(feature = "async")]
    let f_serialize_with_async: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
            .map(Variants::SerializeWithAsync)
    });

    #[cfg(feature = "async")]
    let f_deserialize_with_async: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
            .map(Variants::DeserializeWithAsync)
    });

    #[cfg(feature = "schema")]
    let f_schema: Box<ParseFn> = Box::new(|_attr_name, _meta_item_name, meta| {
        let map_result = meta_get_by_symbol_keys(SCHEMA, meta, &SCHEMA_FIELD_PARSE_MAP)?;
        let schema_attributes: schema::Attributes = map_result.into();
        Ok(Variants::Schema(schema_attributes))
    });

    let f_skip: Box<ParseFn> =
        Box::new(|_attr_name, _meta_item_name, _meta| Ok(Variants::Skip(())));
    m.insert(BOUND, f_bounds);
    #[cfg(feature = "async")]
    m.insert(ASYNC_BOUND, f_async_bounds);
    m.insert(SERIALIZE_WITH, f_serialize_with);
    m.insert(DESERIALIZE_WITH, f_deserialize_with);
    #[cfg(feature = "async")]
    m.insert(SERIALIZE_WITH_ASYNC, f_serialize_with_async);
    #[cfg(feature = "async")]
    m.insert(DESERIALIZE_WITH_ASYNC, f_deserialize_with_async);
    m.insert(SKIP, f_skip);
    #[cfg(feature = "schema")]
    m.insert(SCHEMA, f_schema);
    m
});

#[derive(Default, Clone)]
pub(crate) struct Attributes {
    pub bounds: Option<bounds::Bounds>,
    #[cfg(feature = "async")]
    pub async_bounds: Option<bounds::Bounds>,
    pub serialize_with: Option<syn::ExprPath>,
    pub deserialize_with: Option<syn::ExprPath>,
    #[cfg(feature = "async")]
    pub serialize_with_async: Option<syn::ExprPath>,
    #[cfg(feature = "async")]
    pub deserialize_with_async: Option<syn::ExprPath>,
    pub skip: bool,
    #[cfg(feature = "schema")]
    pub schema: Option<schema::Attributes>,
}

impl From<BTreeMap<Symbol, Variants>> for Attributes {
    fn from(mut map: BTreeMap<Symbol, Variants>) -> Self {
        let bounds = map.remove(&BOUND);
        #[cfg(feature = "async")]
        let async_bounds = map.remove(&ASYNC_BOUND);
        let serialize_with = map.remove(&SERIALIZE_WITH);
        let deserialize_with = map.remove(&DESERIALIZE_WITH);
        #[cfg(feature = "async")]
        let serialize_with_async = map.remove(&SERIALIZE_WITH_ASYNC);
        #[cfg(feature = "async")]
        let deserialize_with_async = map.remove(&DESERIALIZE_WITH_ASYNC);
        let skip = map.remove(&SKIP);

        let bounds = bounds.map(|variant| match variant {
            Variants::Bounds(bounds) => bounds,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });

        #[cfg(feature = "async")]
        let async_bounds = async_bounds.map(|variant| match variant {
            Variants::Bounds(bounds) => bounds,
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

        #[cfg(feature = "async")]
        let serialize_with_async = serialize_with_async.map(|variant| match variant {
            Variants::SerializeWithAsync(serialize_with_async) => serialize_with_async,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });

        #[cfg(feature = "async")]
        let deserialize_with_async = deserialize_with_async.map(|variant| match variant {
            Variants::DeserializeWithAsync(deserialize_with_async) => deserialize_with_async,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });

        let skip = skip.map(|variant| match variant {
            Variants::Skip(skip) => skip,
            _ => unreachable!("only one enum variant is expected to correspond to given map key"),
        });

        #[cfg(feature = "schema")]
        let schema = {
            let schema = map.remove(&SCHEMA);
            schema.map(|variant| match variant {
                Variants::Schema(schema) => schema,
                _ => {
                    unreachable!("only one enum variant is expected to correspond to given map key")
                }
            })
        };

        Self {
            bounds,
            #[cfg(feature = "async")]
            async_bounds,
            serialize_with,
            deserialize_with,
            #[cfg(feature = "async")]
            serialize_with_async,
            #[cfg(feature = "async")]
            deserialize_with_async,
            skip: skip.is_some(),
            #[cfg(feature = "schema")]
            schema,
        }
    }
}

#[cfg(feature = "schema")]
pub(crate) fn filter_attrs(
    attrs: impl Iterator<Item = Attribute>,
) -> impl Iterator<Item = Attribute> {
    attrs.filter(|attr| attr.path() == BORSH)
}

impl Attributes {
    fn check(&self, attr: &Attribute) -> Result<(), syn::Error> {
        cfg_if! {
            if #[cfg(feature = "async")] {
                let test_serde_with = ||
                    self.serialize_with.is_some() ||
                    self.deserialize_with.is_some() ||
                    self.serialize_with_async.is_some() ||
                    self.deserialize_with_async.is_some();
            } else {
                let test_serde_with = ||
                    self.serialize_with.is_some() ||
                    self.deserialize_with.is_some();
            }
        }

        if self.skip && test_serde_with() {
            cfg_if! {
                if #[cfg(feature = "async")] {
                    let msg = format!(
                        "`{}` cannot be used at the same time as `{}`, `{}`, `{}` or `{}`",
                        SKIP.name,
                        SERIALIZE_WITH.name,
                        DESERIALIZE_WITH.name,
                        SERIALIZE_WITH_ASYNC.name,
                        DESERIALIZE_WITH_ASYNC.name,
                    );
                } else {
                    let msg = format!(
                        "`{}` cannot be used at the same time as `{}` or `{}`",
                        SKIP.name,
                        SERIALIZE_WITH.name,
                        DESERIALIZE_WITH.name,
                    );
                }
            }

            return Err(syn::Error::new_spanned(attr, msg));
        }

        #[cfg(feature = "schema")]
        self.check_schema(attr)?;

        Ok(())
    }

    pub(crate) fn parse(attrs: &[Attribute]) -> Result<Self, syn::Error> {
        let borsh = get_one_attribute(attrs)?;

        let result: Self = if let Some(attr) = borsh {
            let result: Self = attr_get_by_symbol_keys(BORSH, attr, &BORSH_FIELD_PARSE_MAP)?.into();
            result.check(attr)?;
            result
        } else {
            BTreeMap::new().into()
        };

        Ok(result)
    }

    pub(crate) fn needs_bounds_derive<const IS_ASYNC: bool>(&self, ty: BoundType) -> bool {
        let predicates = self.get_bounds::<IS_ASYNC>(ty);
        predicates.is_none()
    }

    fn get_bounds<const IS_ASYNC: bool>(&self, ty: BoundType) -> Option<Vec<WherePredicate>> {
        let bounds = if IS_ASYNC {
            cfg_if! {
                if #[cfg(feature = "async")] {
                    self.async_bounds.as_ref()
                } else {
                    None
                }
            }
        } else {
            self.bounds.as_ref()
        };
        bounds.and_then(|bounds| match ty {
            BoundType::Serialize => bounds.serialize.clone(),
            BoundType::Deserialize => bounds.deserialize.clone(),
        })
    }

    pub(crate) fn collect_bounds<const IS_ASYNC: bool>(
        &self,
        ty: BoundType,
    ) -> Vec<WherePredicate> {
        let predicates = self.get_bounds::<IS_ASYNC>(ty);
        predicates.unwrap_or_default()
    }
}

#[cfg(feature = "schema")]
impl Attributes {
    fn check_schema(&self, attr: &Attribute) -> Result<(), syn::Error> {
        if let Some(ref schema) = self.schema {
            if self.skip && schema.params.is_some() {
                return Err(syn::Error::new_spanned(
                    attr,
                    format!(
                        "`{}` cannot be used at the same time as `{}({})`",
                        SKIP.name, SCHEMA.name, PARAMS.expected
                    ),
                ));
            }

            if self.skip && schema.with_funcs.is_some() {
                return Err(syn::Error::new_spanned(
                    attr,
                    format!(
                        "`{}` cannot be used at the same time as `{}({})`",
                        SKIP.name, SCHEMA.name, WITH_FUNCS.expected
                    ),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn needs_schema_params_derive(&self) -> bool {
        if let Some(ref schema) = self.schema {
            if schema.params.is_some() {
                return false;
            }
        }
        true
    }

    pub(crate) fn schema_declaration(&self) -> Option<syn::ExprPath> {
        self.schema.as_ref().and_then(|schema| {
            schema
                .with_funcs
                .as_ref()
                .and_then(|with_funcs| with_funcs.declaration.clone())
        })
    }

    pub(crate) fn schema_definitions(&self) -> Option<syn::ExprPath> {
        self.schema.as_ref().and_then(|schema| {
            schema
                .with_funcs
                .as_ref()
                .and_then(|with_funcs| with_funcs.definitions.clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use syn::{parse_quote, Attribute, ItemStruct};

    use super::{bounds, Attributes};
    use crate::internals::test_helpers::{
        debug_print_tokenizable, debug_print_vec_of_tokenizable, local_insta_assert_debug_snapshot,
        local_insta_assert_snapshot,
    };

    struct ParsedBounds {
        sync: Option<bounds::Bounds>,
        #[cfg(feature = "async")]
        r#async: Option<bounds::Bounds>,
    }

    fn parse_bounds(attrs: &[Attribute]) -> Result<ParsedBounds, syn::Error> {
        // #[borsh(bound(serialize = "...", deserialize = "..."), async_bound(serialize = "...", deserialize = "..."))]
        let borsh_attrs = Attributes::parse(attrs)?;
        Ok(ParsedBounds {
            sync: borsh_attrs.bounds,
            #[cfg(feature = "async")]
            r#async: borsh_attrs.async_bounds,
        })
    }

    #[test]
    fn test_reject_multiple_borsh_attrs() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(skip)]
                #[borsh(bound(deserialize = "K: Hash + Ord,
                     V: Eq + Ord",
                    serialize = "K: Hash + Eq + Ord,
                     V: Ord"
                ))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match Attributes::parse(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    fn test_bounds_parsing1() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash + Ord,
                     V: Eq + Ord",
                    serialize = "K: Hash + Eq + Ord,
                     V: Ord"
                ))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().sync.unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.serialize));
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    fn test_bounds_parsing2() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash + Eq + borsh::de::BorshDeserialize,
                     V: borsh::de::BorshDeserialize",
                    serialize = "K: Hash + Eq + borsh::ser::BorshSerialize,
                     V: borsh::ser::BorshSerialize"
                ))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().sync.unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.serialize));
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    fn test_bounds_parsing3() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash + Eq + borsh::de::BorshDeserialize,
                     V: borsh::de::BorshDeserialize",
                    serialize = ""
                ))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().sync.unwrap();
        assert_eq!(attrs.serialize.as_ref().unwrap().len(), 0);
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    fn test_bounds_parsing4() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash"))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().sync.unwrap();
        assert!(attrs.serialize.is_none());
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    fn test_bounds_parsing_error() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(bound(deser = "K: Hash"))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    fn test_bounds_parsing_error2() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(bound(deserialize = "K Hash"))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    fn test_bounds_parsing_error3() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(bound(deserialize = 42))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_async_bounds_parsing1() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(async_bound(
                    deserialize =
                    "K: Hash + Ord,
                     V: Eq + Ord",
                    serialize =
                    "K: Hash + Eq + Ord,
                     V: Ord"
                ))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().r#async.unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.serialize));
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_async_bounds_parsing2() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(async_bound(deserialize =
                    "K: Hash + Eq + borsh::de::BorshDeserializeAsync,
                     V: borsh::de::BorshDeserializeAsync",
                    serialize =
                    "K: Hash + Eq + borsh::ser::BorshSerializeAsync,
                     V: borsh::ser::BorshSerializeAsync"
                ))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().r#async.unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.serialize));
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_async_bounds_parsing3() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(async_bound(deserialize =
                    "K: Hash + Eq + borsh::de::BorshDeserializeAsync,
                     V: borsh::de::BorshDeserializeAsync",
                    serialize = ""
                ))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().r#async.unwrap();
        assert_eq!(attrs.serialize.as_ref().unwrap().len(), 0);
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_async_bounds_parsing4() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(async_bound(deserialize = "K: Hash"))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().r#async.unwrap();
        assert!(attrs.serialize.is_none());
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_async_bounds_parsing_error() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(async_bound(deser = "K: Hash"))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_async_bounds_parsing_error2() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(async_bound(deserialize = "K Hash"))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_async_bounds_parsing_error3() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(async_bound(deserialize = 42))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_bounds(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_both_bounds_parsing() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(
                    bound(
                        deserialize =
                        "K: Hash + Ord,
                         V: Eq + Ord",
                        serialize =
                        "K: Hash + Eq + Ord,
                         V: Ord"
                    ),
                    async_bound(
                        deserialize =
                        "K: Hash + Ord + A,
                        V: Eq + Ord + AA",
                        serialize =
                        "K: Hash + Eq + Ord + A,
                        V: Ord + AA"
                    )
                )]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = parse_bounds(&first_field.attrs).unwrap().sync.unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.serialize));
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));

        let attrs = parse_bounds(&first_field.attrs).unwrap().r#async.unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.serialize));
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(attrs.deserialize));
    }

    #[test]
    fn test_ser_de_with_parsing1() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(
                    serialize_with = "third_party_impl::serialize_third_party",
                    deserialize_with = "third_party_impl::deserialize_third_party",
                )]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = Attributes::parse(&first_field.attrs).unwrap();
        local_insta_assert_snapshot!(debug_print_tokenizable(attrs.serialize_with.as_ref()));
        local_insta_assert_snapshot!(debug_print_tokenizable(attrs.deserialize_with));
    }

    #[test]
    #[cfg(feature = "async")]
    fn test_async_ser_de_with_parsing1() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(
                    serialize_with_async = "third_party_impl::serialize_third_party",
                    deserialize_with_async = "third_party_impl::deserialize_third_party",
                )]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = Attributes::parse(&first_field.attrs).unwrap();
        local_insta_assert_snapshot!(debug_print_tokenizable(attrs.serialize_with_async.as_ref()));
        local_insta_assert_snapshot!(debug_print_tokenizable(attrs.deserialize_with_async));
    }

    #[test]
    fn test_borsh_skip() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(skip)]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];

        let result = Attributes::parse(&first_field.attrs).unwrap();
        assert!(result.skip);
    }

    #[test]
    fn test_borsh_no_skip() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];

        let result = Attributes::parse(&first_field.attrs).unwrap();
        assert!(!result.skip);
    }
}

#[cfg(feature = "schema")]
#[cfg(test)]
mod tests_schema {
    use syn::{parse_quote, Attribute, ItemStruct};

    use super::schema;
    use crate::internals::{
        attributes::field::Attributes,
        test_helpers::{
            debug_print_tokenizable, debug_print_vec_of_tokenizable,
            local_insta_assert_debug_snapshot, local_insta_assert_snapshot,
        },
    };

    fn parse_schema_attrs(attrs: &[Attribute]) -> Result<Option<schema::Attributes>, syn::Error> {
        // #[borsh(schema(params = "..."))]
        let borsh_attrs = Attributes::parse(attrs)?;
        Ok(borsh_attrs.schema)
    }

    #[test]
    fn test_root_bounds_and_params_combined() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(
                    serialize_with = "third_party_impl::serialize_third_party",
                    bound(deserialize = "K: Hash"),
                    schema(params = "T => <T as TraitName>::Associated, V => Vec<V>")
                )]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];

        let attrs = Attributes::parse(&first_field.attrs).unwrap();
        let bounds = attrs.bounds.clone().unwrap();
        assert!(bounds.serialize.is_none());
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(bounds.deserialize));
        assert!(attrs.deserialize_with.is_none());
        let schema = attrs.schema.clone().unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(schema.params));
        local_insta_assert_snapshot!(debug_print_tokenizable(attrs.serialize_with));
    }

    #[test]
    fn test_schema_params_parsing1() {
        let item_struct: ItemStruct = parse_quote! {
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
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let schema_attrs = parse_schema_attrs(&first_field.attrs).unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(schema_attrs.unwrap().params));
    }

    #[test]
    fn test_schema_params_parsing_error() {
        let item_struct: ItemStruct = parse_quote! {
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
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_schema_attrs(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    fn test_schema_params_parsing_error2() {
        let item_struct: ItemStruct = parse_quote! {
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
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match parse_schema_attrs(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    fn test_schema_params_parsing2() {
        let item_struct: ItemStruct = parse_quote! {
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
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let schema_attrs = parse_schema_attrs(&first_field.attrs).unwrap();
        local_insta_assert_snapshot!(debug_print_vec_of_tokenizable(schema_attrs.unwrap().params));
    }

    #[test]
    fn test_schema_params_parsing3() {
        let item_struct: ItemStruct = parse_quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                #[borsh(schema(params = "" ))]
                field: <T as TraitName>::Associated,
                another: V,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let schema_attrs = parse_schema_attrs(&first_field.attrs).unwrap();
        assert_eq!(schema_attrs.unwrap().params.unwrap().len(), 0);
    }

    #[test]
    fn test_schema_params_parsing4() {
        let item_struct: ItemStruct = parse_quote! {
            struct Parametrized<V, T>
            where
                T: TraitName,
            {
                field: <T as TraitName>::Associated,
                another: V,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let schema_attrs = parse_schema_attrs(&first_field.attrs).unwrap();
        assert!(schema_attrs.is_none());
    }

    #[test]
    fn test_schema_with_funcs_parsing() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(schema(with_funcs(
                    declaration = "third_party_impl::declaration::<K, V>",
                    definitions = "third_party_impl::add_definitions_recursively::<K, V>"
                )))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = Attributes::parse(&first_field.attrs).unwrap();
        let schema = attrs.schema.unwrap();
        let with_funcs = schema.with_funcs.unwrap();

        local_insta_assert_snapshot!(debug_print_tokenizable(with_funcs.declaration));
        local_insta_assert_snapshot!(debug_print_tokenizable(with_funcs.definitions));
    }

    // both `declaration` and `definitions` have to be specified
    #[test]
    fn test_schema_with_funcs_parsing_error() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(schema(with_funcs(
                    declaration = "third_party_impl::declaration::<K, V>"
                )))]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let attrs = Attributes::parse(&first_field.attrs);

        let err = match attrs {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    fn test_root_error() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(boons)]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];
        let err = match Attributes::parse(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }

    #[test]
    fn test_root_bounds_and_wrong_key_combined() {
        let item_struct: ItemStruct = parse_quote! {
            struct A {
                #[borsh(bound(deserialize = "K: Hash"),
                        schhema(params = "T => <T as TraitName>::Associated, V => Vec<V>")
                )]
                x: u64,
                y: String,
            }
        };

        let first_field = &item_struct.fields.into_iter().collect::<Vec<_>>()[0];

        let err = match Attributes::parse(&first_field.attrs) {
            Ok(..) => unreachable!("expecting error here"),
            Err(err) => err,
        };
        local_insta_assert_debug_snapshot!(err);
    }
}
