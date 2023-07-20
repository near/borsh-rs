use std::collections::BTreeMap;

use once_cell::sync::Lazy;
use syn::{meta::ParseNestedMeta, Attribute, Field, WherePredicate};

use self::{bounds::BOUNDS_FIELD_PARSE_MAP, schema::SCHEMA_FIELD_PARSE_MAP};

use super::{
    parsing::{attr_get_by_symbol_keys, meta_get_by_symbol_keys},
    BoundType, Symbol, BORSH, BOUND, SCHEMA,
};

pub mod bounds;
pub mod schema;

enum Variants {
    Schema(schema::Attributes),
    Bounds(bounds::Attributes),
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
    m.insert(BOUND, f1);
    m.insert(SCHEMA, f2);
    m
});

#[derive(Default)]
pub(crate) struct Attributes {
    pub bounds: Option<bounds::Attributes>,
    pub schema: Option<schema::Attributes>,
}

impl From<BTreeMap<Symbol, Variants>> for Attributes {
    fn from(mut map: BTreeMap<Symbol, Variants>) -> Self {
        let bounds = map.remove(&BOUND);
        let schema = map.remove(&SCHEMA);
        let bounds = bounds.and_then(|variant| match variant {
            Variants::Bounds(bounds) => Some(bounds),
            _ => None,
        });
        let schema = schema.and_then(|variant| match variant {
            Variants::Schema(schema) => Some(schema),
            _ => None,
        });
        Self { bounds, schema }
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
