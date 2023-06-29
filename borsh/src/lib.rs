#![cfg_attr(not(feature = "std"), no_std)]

/*!

# Crate features

### Ecosystem features

* **std** -
  When enabled, this will cause `borsh` to use the standard library. Currently,
  disabling this feature will result in building the crate in `no_std` environment.

### Default features

* **std** - enabled by default.

### Other features

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


### Config aliases

* **hash_collections** -
  This is a feature alias, set up in `build.rs` to be equivalent to (**std** OR **hashbrown**).
  This alias gates [schema](crate::schema) and [schema_helpers](crate::schema_helpers) modules, as
  [BorshSchema](crate::schema::BorshSchema) relies on [HashMap](std::collections::HashMap) existing.
  Gates implementation of [BorshSerialize](crate::ser::BorshSerialize) and [BorshDeserialize](crate::de::BorshDeserialize)
  for [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet).


*/

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "schema")]
pub use borsh_derive::BorshSchema;
pub use borsh_derive::{BorshDeserialize, BorshSerialize};

pub mod de;

// See `hash_collections` alias definition in build.rs
#[cfg(feature = "schema")]
pub mod schema;
#[cfg(feature = "schema")]
pub mod schema_helpers;
pub mod ser;

pub use de::BorshDeserialize;
pub use de::{from_reader, from_slice};
#[cfg(feature = "schema")]
pub use schema::BorshSchema;
#[cfg(feature = "schema")]
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
        pub use alloc::collections::{btree_map, BTreeMap, BTreeSet, LinkedList, VecDeque};
        #[cfg(feature = "hashbrown")]
        pub use hashbrown::*;
    }

    pub mod io {
        pub use super::super::nostd_io::*;
    }
}
