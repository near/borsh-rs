#![cfg_attr(not(feature = "std"), no_std)]

/*!

# Crate features

### Ecosystem features

* **std** -
  When enabled, `borsh` uses the standard library. Disabling this feature will
  result in building the crate in `no_std` environment.

  To carter such builds, Borsh offers [`io`] module which includes a items which
  are used in [`BorshSerialize`] and [`BorshDeserialize`] traits.  Most notably
  `io::Read`, `io::Write` and `io::Result`.

  When **std** feature is enabled, those items are re-exports of corresponding
  `std::io` items.  Otherwise they are borsh-specific types which mimic
  behaviour of corresponding standard types.

### Default features

* **std** - enabled by default.

### Other features

* **derive** -
  Gates derive macros of [BorshSerialize](crate::ser::BorshSerialize) and
  [BorshDeserialize](crate::de::BorshDeserialize) traits.
* **schema** -
  Gates [BorshSchema](crate::schema::BorshSchema) trait and its derive macro.
  Gates [schema](crate::schema) module.
  This feature requires **derive** to be enabled too.
* **rc** -
  Gates implementation of [BorshSerialize](crate::ser::BorshSerialize) and [BorshDeserialize](crate::de::BorshDeserialize)
  for [`Rc<T>`](std::rc::Rc)/[`Arc<T>`](std::sync::Arc) respectively.
  In `no_std` setting `Rc`/`Arc` are pulled from `alloc` crate.
* **hashbrown** -
  Pulls in [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet) when no `std` is available.
  This feature is set to be mutually exclusive with **std** feature.
* **bytes** -
  Gates implementation of [BorshSerialize](crate::ser::BorshSerialize) and [BorshDeserialize](crate::de::BorshDeserialize)
  for [Bytes](bytes::Bytes) and [BytesMut](bytes::BytesMut).
* **bson** -
  Gates implementation of [BorshSerialize](crate::ser::BorshSerialize) and [BorshDeserialize](crate::de::BorshDeserialize)
  for [ObjectId](bson::oid::ObjectId).
* **de_strict_order** -
  Enables check that keys, parsed during deserialization of
  [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet) and
  [BTreeSet](std::collections::BTreeSet)/[BTreeMap](std::collections::BTreeMap)
  are encountered in ascending order with respect to [PartialOrd](core::cmp::PartialOrd) for hash collections,
  and [Ord](core::cmp::Ord) for btree ones. Deserialization emits error otherwise.

  If this feature is not enabled, it is possible that two different byte slices could deserialize into the same `HashMap`/`HashSet` object.

### Config aliases

* **hash_collections** -
  This is a feature alias, set up in `build.rs` to be equivalent to (**std** OR **hashbrown**).
  Gates implementation of [BorshSerialize](crate::ser::BorshSerialize), [BorshDeserialize](crate::de::BorshDeserialize)
  and [BorshSchema](crate::schema::BorshSchema)
  for [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet).


*/

extern crate alloc;

/// Derive macro available if borsh is built with `features = ["derive", "schema"]`.
#[cfg(feature = "schema")]
pub use borsh_derive::BorshSchema;

/// Derive macro available if borsh is built with `features = ["derive"]`.
#[cfg(feature = "derive")]
pub use borsh_derive::{BorshDeserialize, BorshSerialize};

pub mod de;

// See `hash_collections` alias definition in build.rs
/// Module is available if borsh is built with `features = ["derive", "schema"]`.
#[cfg(feature = "schema")]
pub mod schema;
/// Module is available if borsh is built with `features = ["derive", "schema"]`.
#[cfg(feature = "schema")]
pub(crate) mod schema_helpers;
pub mod ser;

pub use de::BorshDeserialize;
pub use de::{from_reader, from_slice};
#[cfg(feature = "schema")]
pub use schema::BorshSchema;
#[cfg(feature = "schema")]
pub use schema_helpers::{
    max_serialized_size, schema_container_of, try_from_slice_with_schema, try_to_vec_with_schema,
    MaxSizeError,
};
pub use ser::helpers::{to_vec, to_writer};
pub use ser::BorshSerialize;
pub mod error;

#[cfg(all(feature = "std", feature = "hashbrown"))]
compile_error!("feature \"std\" and feature \"hashbrown\" don't make sense at the same time");

#[cfg(feature = "std")]
use std::io as io_impl;
#[cfg(not(feature = "std"))]
mod nostd_io;
#[cfg(not(feature = "std"))]
use nostd_io as io_impl;

/// Subset of `std::io` which is used as part of borsh public API.
///
/// When crate is built with `std` feature disabled (it’s enabled by default),
/// the exported types are custom borsh types which try to mimic behaviour of
/// corresponding standard types usually offering subset of features.
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
