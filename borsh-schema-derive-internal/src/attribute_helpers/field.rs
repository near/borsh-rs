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
    Bounds(bounds::Attributes),
    SerializeWith(syn::ExprPath),
    DeserializeWith(syn::ExprPath),
}

type ParseFn = dyn Fn(Symbol, Symbol, &ParseNestedMeta) -> syn::Result<Variants> + Send + Sync;

static BORSH_FIELD_PARSE_MAP: Lazy<BTreeMap<Symbol, Box<ParseFn>>> = Lazy::new(|| {
    let mut m = BTreeMap::new();
    // has to be inlined; assigning closure separately doesn't work
    let f1: Box<ParseFn> = Box::new(|_attr_name, _meta_item_name, meta| {
        let map_result = meta_get_by_symbol_keys(BOUND, meta, &BOUNDS_FIELD_PARSE_MAP)?;
        let bounds_attributes: bounds::Attributes = map_result.into();
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
    pub bounds: Option<bounds::Attributes>,
    #[allow(unused)]
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

    fn get_bounds(&self, ty: BoundType) -> Bounds {
        let attributes = self.bounds.as_ref();
        attributes.and_then(|attributes| match ty {
            BoundType::Serialize => attributes.serialize.clone(),
            BoundType::Deserialize => attributes.deserialize.clone(),
        })
    }
    pub(crate) fn collect_bounds(&self, ty: BoundType) -> Vec<WherePredicate> {
        let predicates = self.get_bounds(ty);
        predicates.unwrap_or(vec![])
    }
}

pub(crate) type Bounds = Option<Vec<WherePredicate>>;
