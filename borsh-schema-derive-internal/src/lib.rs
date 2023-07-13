#![recursion_limit = "128"]
#![cfg_attr(
    feature = "force_exhaustive_checks",
    feature(non_exhaustive_omitted_patterns_lint)
)]

mod generics;
mod helpers;

mod enum_schema;
mod struct_schema;
pub use enum_schema::process_enum;
pub use struct_schema::process_struct;

#[cfg(test)]
pub mod test_helpers;
