#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "derive")]

#[cfg(feature = "std")]
use std::collections::BTreeMap;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    borrow,
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use borsh::{from_slice, BorshDeserialize, BorshSerialize};

struct ThirdParty<K: Ord, V>(
    BTreeMap<K, V>,
);

mod third_party_impl {
    use super::ThirdParty;

    #[cfg(feature = "std")]
    use std::io;

    #[cfg(not(feature = "std"))]
    use borsh::nostd_io as io;
    pub(super) fn serialize_third_party<
        K: borsh::ser::BorshSerialize + Ord,
        V: borsh::ser::BorshSerialize,
        W: io::Write,
    >(
        obj: &ThirdParty<K, V>,
        writer: &mut W,
    ) -> ::core::result::Result<(), io::Error> {
        borsh::BorshSerialize::serialize(&obj.0, writer)?;
        Ok(())
    }

    pub(super) fn deserialize_third_party<R: io::Read,
        K: borsh::de::BorshDeserialize + Ord,
        V: borsh::de::BorshDeserialize,
    >(
        reader: &mut R,
    ) -> ::core::result::Result<ThirdParty<K, V>, io::Error> {
        Ok(ThirdParty(borsh::BorshDeserialize::deserialize_reader(reader)?))
    }
}

#[derive(BorshSerialize)]
struct A<K: Ord, V> {
    #[borsh(serialize_with = "third_party_impl::serialize_third_party")]
    x: ThirdParty<K, V>,
    y: u64,
}

#[derive(BorshDeserialize)]
struct B<K: Ord, V> {
    #[borsh(deserialize_with = "third_party_impl::deserialize_third_party")]
    x: ThirdParty<K, V>,
    y: u64,
}
