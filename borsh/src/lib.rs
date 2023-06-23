#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub use borsh_derive::{BorshDeserialize, BorshSchema, BorshSerialize};

pub mod de;

// See `hash_collections` alias definition in build.rs
#[cfg(hash_collections)]
pub mod schema;
#[cfg(hash_collections)]
pub mod schema_helpers;
pub mod ser;

pub use de::BorshDeserialize;
pub use de::{from_reader, from_slice};
#[cfg(hash_collections)]
pub use schema::BorshSchema;
#[cfg(hash_collections)]
pub use schema_helpers::{try_from_slice_with_schema, try_to_vec_with_schema};
pub use ser::helpers::{to_vec, to_writer};
pub use ser::BorshSerialize;

#[cfg(all(feature = "std", feature = "hashbrown"))]
compile_error!("feature \"std\" and feature \"hashbrown\" don't make sense at the same time");

/// A facade around all the types we need from the `std`, `core`, and `alloc`
/// crates. This avoids elaborate import wrangling having to happen in every
/// module.
#[doc(hidden)]
#[cfg(feature = "std")]
pub mod __maybestd {
    pub use std::{borrow, boxed, collections, format, io, string, vec};

    #[cfg(feature = "rc")]
    pub use std::{rc, sync};
}

#[cfg(not(feature = "std"))]
pub mod nostd_io;

#[doc(hidden)]
#[cfg(not(feature = "std"))]
pub mod __maybestd {
    pub use alloc::{borrow, boxed, format, string, vec};

    #[cfg(feature = "rc")]
    pub use alloc::{rc, sync};

    pub mod collections {
        pub use alloc::collections::{BTreeMap, BTreeSet, LinkedList, VecDeque};
        #[cfg(feature = "hashbrown")]
        pub use hashbrown::*;
    }

    pub mod io {
        pub use super::super::nostd_io::*;
    }
}
