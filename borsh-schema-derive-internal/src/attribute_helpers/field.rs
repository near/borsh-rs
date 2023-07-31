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
    // assigning closure `let f = |args| {...};` and boxing closure `let f: Box<ParseFn> = Box::new(f);`
    // on 2 separate lines doesn't work
    let f_bounds: Box<ParseFn> = Box::new(|_attr_name, _meta_item_name, meta| {
        let map_result = meta_get_by_symbol_keys(BOUND, meta, &BOUNDS_FIELD_PARSE_MAP)?;
        let bounds_attributes: bounds::Bounds = map_result.into();
        Ok(Variants::Bounds(bounds_attributes))
    });

    let f_schema: Box<ParseFn> = Box::new(|_attr_name, _meta_item_name, meta| {
        let map_result = meta_get_by_symbol_keys(SCHEMA, meta, &SCHEMA_FIELD_PARSE_MAP)?;
        let schema_attributes: schema::Attributes = map_result.into();
        Ok(Variants::Schema(schema_attributes))
    });

    let f_serialize_with: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
            .map(Variants::SerializeWith)
    });

    let f_deserialize_with: Box<ParseFn> = Box::new(|attr_name, meta_item_name, meta| {
        parse_lit_into::<syn::ExprPath>(attr_name, meta_item_name, meta)
            .map(Variants::DeserializeWith)
    });

    m.insert(BOUND, f_bounds);
    m.insert(SCHEMA, f_schema);
    m.insert(SERIALIZE_WITH, f_serialize_with);
    m.insert(DESERIALIZE_WITH, f_deserialize_with);
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
    fn check(&self, skipped: bool, attr: &Attribute) -> Result<(), syn::Error> {
        if skipped && (self.serialize_with.is_some() || self.deserialize_with.is_some()) {
            return Err(syn::Error::new_spanned(
                attr,
                format!(
                    "`{}` cannot be used at the same time as `{}` or `{}`",
                    SKIP.0, SERIALIZE_WITH.0, DESERIALIZE_WITH.0
                ),
            ));
        }
        if let Some(ref schema) = self.schema {
            if skipped && schema.params.is_some() {
                return Err(syn::Error::new_spanned(
                    attr,
                    format!(
                        "`{}` cannot be used at the same time as `{}({})`",
                        SKIP.0, SCHEMA.0, PARAMS.1
                    ),
                ));
            }

            if skipped && schema.with_funcs.is_some() {
                return Err(syn::Error::new_spanned(
                    attr,
                    format!(
                        "`{}` cannot be used at the same time as `{}({})`",
                        SKIP.0, SCHEMA.0, WITH_FUNCS.1
                    ),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn parse(attrs: &[Attribute], skipped: bool) -> Result<Self, syn::Error> {
        let attr = attrs.iter().find(|attr| attr.path() == BORSH);

        let result: Self = if let Some(attr) = attr {
            let result: Self = attr_get_by_symbol_keys(BORSH, attr, &BORSH_FIELD_PARSE_MAP)?.into();
            result.check(skipped, attr)?;
            result
        } else {
            BTreeMap::new().into()
        };

        Ok(result)
    }
    pub(crate) fn needs_bounds_derive(&self, ty: BoundType) -> bool {
        let predicates = self.get_bounds(ty);
        predicates.is_none()
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
