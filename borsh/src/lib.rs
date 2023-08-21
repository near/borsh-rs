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

* **derive** -
  Gates derive macros of [BorshSerialize](crate::ser::BorshSerialize) and
  [BorshDeserialize](crate::de::BorshDeserialize) traits.
* **schema** -
  Gates [BorshSchema](crate::schema::BorshSchema) trait and its derive macro.
  Gates [schema](crate::schema) and [schema_helpers](crate::schema_helpers) modules.
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


### Config aliases

* **hash_collections** -
  This is a feature alias, set up in `build.rs` to be equivalent to (**std** OR **hashbrown**).
  Gates implementation of [BorshSerialize](crate::ser::BorshSerialize), [BorshDeserialize](crate::de::BorshDeserialize)
  and [BorshSchema](crate::schema::BorshSchema)
  for [HashMap](std::collections::HashMap)/[HashSet](std::collections::HashSet).
* **derive_schema** -
  This is a feature alias, set up in `build.rs` to be equivalent to (**derive** AND **schema**).


*/

#[cfg(not(feature = "std"))]
extern crate alloc;

/// Derive macro available if borsh is built with `features = ["derive", "schema"]`.
#[cfg(derive_schema)]
pub use borsh_derive::BorshSchema;

/// Derive macro available if borsh is built with `features = ["derive"]`.
#[cfg(feature = "derive")]
pub use borsh_derive::{BorshDeserialize, BorshSerialize};

pub mod de;

// See `hash_collections` alias definition in build.rs
/// Module is available if borsh is built with `features = ["derive", "schema"]`.
#[cfg(derive_schema)]
pub mod schema;
/// Module is available if borsh is built with `features = ["derive", "schema"]`.
#[cfg(derive_schema)]
pub mod schema_helpers;
pub mod ser;

pub use de::BorshDeserialize;
pub use de::{from_reader, from_slice};
#[cfg(derive_schema)]
pub use schema::BorshSchema;
#[cfg(derive_schema)]
pub use schema_helpers::{try_from_slice_with_schema, try_to_vec_with_schema};
pub use ser::helpers::{to_vec, to_writer};
pub use ser::BorshSerialize;
pub mod error;

#[cfg(all(feature = "std", feature = "hashbrown"))]
compile_error!("feature \"std\" and feature \"hashbrown\" don't make sense at the same time");

#[cfg(all(feature = "schema", not(feature = "derive")))]
compile_error!(
    "feature \"schema\" depends on \"derive\" feature in its implementation; enable it too.."
);

#[cfg(not(feature = "std"))]
pub mod nostd_io;

#[doc(hidden)]
pub mod __private {

    /// A facade around all the types we need from the `std`, and `alloc`
    /// crates. This avoids elaborate import wrangling having to happen in every
    /// module.
    #[cfg(feature = "std")]
    pub mod maybestd {
        pub use std::{borrow, boxed, collections, format, io, string, vec};

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

        pub mod io {
            pub use crate::nostd_io::*;
        }
    }
}
