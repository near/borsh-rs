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
  Gates derive macros of [BorshSerialize] and
  [BorshDeserialize] traits.
* **unstable__schema** -
  Gates [BorshSchema] trait and its derive macro.
  Gates [schema] module.
  This feature requires **derive** to be enabled too.
* **rc** -
  Gates implementation of [BorshSerialize] and [BorshDeserialize]
  for [`Rc<T>`](std::rc::Rc)/[`Arc<T>`](std::sync::Arc) respectively.
  In `no_std` setting `Rc`/`Arc` are pulled from `alloc` crate.
  Serializing and deserializing these types
  does not preserve identity and may result in multiple copies of the same data.
  Be sure that this is what you want before enabling this feature.
* **hashbrown** -
  Pulls in [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet) when no `std` is available.
  This feature is set to be mutually exclusive with **std** feature.
* **bytes** -
  Gates implementation of [BorshSerialize] and [BorshDeserialize]
  for [Bytes](https://docs.rs/bytes/1.5.0/bytes/struct.Bytes.html) and [BytesMut](https://docs.rs/bytes/1.5.0/bytes/struct.BytesMut.html).
* **bson** -
  Gates implementation of [BorshSerialize] and [BorshDeserialize]
  for [ObjectId](https://docs.rs/bson/2.9.0/bson/oid/struct.ObjectId.html).
* **ascii** -
  Gates implementation of [BorshSerialize], [BorshDeserialize], [BorshSchema] for
  types from [ascii](https://docs.rs/ascii/1.1.0/ascii/) crate.
* **de_strict_order** -
  Enables check that keys, parsed during deserialization of
  [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet) and
  [BTreeSet](std::collections::BTreeSet)/[BTreeMap](std::collections::BTreeMap)
  are encountered in ascending order with respect to [PartialOrd] for hash collections,
  and [Ord] for btree ones. Deserialization emits error otherwise.

  If this feature is not enabled, it is possible that two different byte slices could deserialize into the same `HashMap`/`HashSet` object.

### Config aliases

* **hash_collections** -
  This is a feature alias, set up in `build.rs` to be equivalent to (**std** OR **hashbrown**).
  Gates implementation of [BorshSerialize], [BorshDeserialize]
  and [BorshSchema]
  for [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet).


*/

#[cfg(not(feature = "std"))]
extern crate alloc;

/// Derive macro available if borsh is built with `features = ["unstable__schema"]`.
#[cfg(feature = "unstable__schema")]
pub use borsh_derive::BorshSchema;

/// Derive macro available if borsh is built with `features = ["derive"]`.
#[cfg(feature = "derive")]
pub use borsh_derive::{BorshDeserialize, BorshSerialize};

pub mod de;

// See `hash_collections` alias definition in build.rs
/// Module is available if borsh is built with `features = ["unstable__schema"]`.
#[cfg(feature = "unstable__schema")]
pub mod schema;
#[cfg(feature = "unstable__schema")]
pub(crate) mod schema_helpers;
pub mod ser;

pub use de::BorshDeserialize;
pub use de::{from_reader, from_slice};
#[cfg(feature = "unstable__schema")]
pub use schema::BorshSchema;
#[cfg(feature = "unstable__schema")]
pub use schema_helpers::{
    max_serialized_size, schema_container_of, try_from_slice_with_schema, try_to_vec_with_schema,
};
pub use ser::helpers::{object_length, to_vec, to_writer};
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
