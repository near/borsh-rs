#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../docs/rustdoc_include/borsh_crate_top_level.md")]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[doc = include_str!("../docs/rustdoc_include/borsh_deserialize.md")]
#[cfg(feature = "derive")]
pub use borsh_derive::BorshDeserialize;
// TODO: add docs
#[cfg(all(feature = "derive", feature = "unstable__async"))]
pub use borsh_derive::BorshDeserializeAsync;
#[doc = include_str!("../docs/rustdoc_include/borsh_schema.md")]
#[cfg(feature = "unstable__schema")]
pub use borsh_derive::BorshSchema;
#[doc = include_str!("../docs/rustdoc_include/borsh_serialize.md")]
#[cfg(feature = "derive")]
pub use borsh_derive::BorshSerialize;
// TODO: add docs
#[cfg(all(feature = "derive", feature = "unstable__async"))]
pub use borsh_derive::BorshSerializeAsync;

pub mod de;

// See `hash_collections` alias definition in build.rs
/// Module is available if borsh is built with `features = ["unstable__schema"]`.
#[cfg(feature = "unstable__schema")]
pub mod schema;
#[cfg(feature = "unstable__schema")]
pub(crate) mod schema_helpers;
pub mod ser;

pub use de::{from_reader, from_slice, BorshDeserialize};
#[cfg(feature = "unstable__async")]
pub use de::{from_reader_async, BorshDeserializeAsync};
#[cfg(feature = "unstable__schema")]
pub use schema::BorshSchema;
#[cfg(feature = "unstable__schema")]
pub use schema_helpers::{
    max_serialized_size, schema_container_of, try_from_slice_with_schema, try_to_vec_with_schema,
};
#[cfg(feature = "unstable__async")]
pub use ser::{helpers::to_writer_async, BorshSerializeAsync};
pub use ser::{
    helpers::{object_length, to_vec, to_writer},
    BorshSerialize,
};
pub mod error;

#[cfg(all(feature = "std", feature = "hashbrown"))]
compile_error!("feature \"std\" and feature \"hashbrown\" don't make sense at the same time");

#[cfg(all(feature = "unstable__tokio", feature = "unstable__async-std"))]
compile_error!(
    "Cannot enable both `unstable__tokio` and `unstable__async-std` features at the same time"
);

#[cfg(feature = "std")]
use std::io as io_impl;

/// Provides traits for async I/O operations.
///
/// Module is available if `borsh` is built with `features = ["unstable__async"]`.
#[cfg(feature = "unstable__async")]
pub mod async_io;

#[cfg(not(feature = "std"))]
mod nostd_io;

#[cfg(not(feature = "std"))]
use nostd_io as io_impl;

/// Subset of `std::io` which is used as part of `borsh` public API.
///
/// When crate is built with `std` feature disabled (it’s enabled by default),
/// the exported types are custom `borsh` types that try to mimic the behavior of
/// corresponding standard types, usually offering a subset of features.
pub mod io {
    pub use super::io_impl::{Error, ErrorKind, Read, Result, Write};
}

#[doc(hidden)]
pub mod __private {

    /// A facade around all the types we need from the `std`, and `alloc`
    /// crates. This avoids elaborate import wrangling having to happen in every
    /// module.
    #[cfg(feature = "std")]
    pub mod maybestd {
        pub use std::{borrow, boxed, collections, format, string, vec};
        #[cfg(feature = "rc")]
        pub use std::{rc, sync};
    }
    #[cfg(not(feature = "std"))]
    pub mod maybestd {
        pub use alloc::{borrow, boxed, format, string, vec};
        #[cfg(feature = "rc")]
        pub use alloc::{rc, sync};

        pub mod collections {
            pub use alloc::collections::{btree_map, BTreeMap, BTreeSet, LinkedList, VecDeque};

            #[cfg(feature = "hashbrown")]
            pub use hashbrown::*;
        }
    }
}
