//! The derives: trait impls, `EnumExt`, `ENCODES_TO_ZERO_BYTES`, schema declarations and
//! definitions, and inferred bounds, from one parse (`ir`), one bound inference (`bounds`)
//! and three short emitters.
//!
//! Generated code is hygienic by construction: paths are absolute, trait calls are fully
//! qualified, primitives are `::core::primitive::*`, and every impl sits in a `const _` block.
//! The names the code introduces use the reserved `__borsh_` prefix, and each binding is also
//! declared in that block as a function, so a user `const` of that name can't turn the binding
//! into a constant pattern.
mod bounds;
mod gen;
mod ir;

use gen::{deserialize, serialize};
use ir::Item;
use proc_macro2::TokenStream;
use syn::{parse_macro_input, DeriveInput, Result};

#[proc_macro_derive(BorshSerialize, attributes(borsh))]
pub fn borsh_serialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    run(input, serialize)
}

#[proc_macro_derive(BorshDeserialize, attributes(borsh))]
pub fn borsh_deserialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    run(input, deserialize)
}

#[cfg(feature = "schema")]
#[proc_macro_derive(BorshSchema, attributes(borsh))]
pub fn borsh_schema(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    run(input, gen::schema)
}

fn run(
    input: proc_macro::TokenStream,
    f: fn(&Item) -> Result<TokenStream>,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ir::parse(&input)
        .and_then(|item| f(&item))
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
