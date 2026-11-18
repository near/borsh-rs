#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../docs/rustdoc_include/borsh_crate_top_level.md")]

#[cfg(not(feature = "std"))]
extern crate alloc;

/// Calls `$m!` on every prefix of the list after the brackets: `$m!(0 T0)`,
/// `$m!(0 T0 1 T1)`, and so on, to implement the traits for tuples.
macro_rules! tuples {
    ($m:ident [$($done:tt)*]) => {};
    ($m:ident [$($done:tt)*] $idx:tt $name:ident $($rest:tt)*) => {
        $m!($($done)* $idx $name);
        tuples!($m [$($done)* $idx $name] $($rest)*);
    };
}

#[doc = include_str!("../docs/rustdoc_include/borsh_schema.md")]
#[cfg(feature = "unstable__schema")]
pub use borsh_derive::BorshSchema;

#[doc = include_str!("../docs/rustdoc_include/borsh_deserialize.md")]
#[cfg(feature = "derive")]
pub use borsh_derive::BorshDeserialize;

#[doc = include_str!("../docs/rustdoc_include/borsh_serialize.md")]
#[cfg(feature = "derive")]
pub use borsh_derive::BorshSerialize;

pub mod de;
pub mod error;
#[cfg(feature = "unstable__schema")]
pub mod schema;
pub mod ser;

pub use de::{from_reader, from_slice, BorshDeserialize};
#[cfg(feature = "unstable__schema")]
pub use schema::{
    helpers::{
        max_serialized_size, schema_container_of, try_from_slice_with_schema,
        try_to_vec_with_schema,
    },
    BorshSchema,
};
pub use ser::{
    helpers::{object_length, to_vec, to_writer},
    BorshSerialize,
};

#[cfg(all(feature = "std", feature = "hashbrown"))]
compile_error!("feature \"std\" and feature \"hashbrown\" don't make sense at the same time");

#[cfg(not(feature = "std"))]
mod nostd_io;

/// `std::io` with `std`, a small copy of its relevant parts without.
pub mod io {
    #[cfg(not(feature = "std"))]
    pub use super::nostd_io::{Error, ErrorKind, Read, Result, Write};
    #[cfg(feature = "std")]
    pub use std::io::{Error, ErrorKind, Read, Result, Write};
}

#[doc(hidden)]
pub mod __private {
    /// Whether every value of `T` encodes to zero bytes. Collections of such types are
    /// rejected, since a 4-byte length could otherwise demand unbounded work.
    pub const fn encodes_to_zero_bytes_de<T: crate::BorshDeserialize>() -> bool {
        T::ENCODES_TO_ZERO_BYTES
    }

    pub const fn encodes_to_zero_bytes_ser<T: crate::BorshSerialize + ?Sized>() -> bool {
        T::ENCODES_TO_ZERO_BYTES
    }

    /// The derive names a field's type through a pointer projection, so it never has to
    /// spell the type out.
    pub const fn field_encodes_to_zero_bytes_de<S: ?Sized, T: crate::BorshDeserialize>(
        _field: fn(&S) -> *const T,
    ) -> bool {
        T::ENCODES_TO_ZERO_BYTES
    }

    pub const fn field_encodes_to_zero_bytes_ser<S: ?Sized, T: crate::BorshSerialize + ?Sized>(
        _field: fn(&S) -> *const T,
    ) -> bool {
        T::ENCODES_TO_ZERO_BYTES
    }

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
