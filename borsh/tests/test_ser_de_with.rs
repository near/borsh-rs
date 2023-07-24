#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "derive")]

#[cfg(feature = "std")]
use std::collections::BTreeMap;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
};
use borsh::{from_slice, BorshDeserialize, BorshSerialize};

#[derive(Debug, PartialEq, Eq)]
struct ThirdParty<K, V>(pub BTreeMap<K, V>);

mod third_party_impl {
    use super::ThirdParty;

    #[cfg(feature = "std")]
    use std::io;

    #[cfg(not(feature = "std"))]
    use borsh::nostd_io as io;
    pub(super) fn serialize_third_party<
        K: borsh::ser::BorshSerialize,
        V: borsh::ser::BorshSerialize,
        W: io::Write,
    >(
        obj: &ThirdParty<K, V>,
        writer: &mut W,
    ) -> ::core::result::Result<(), io::Error> {
        borsh::BorshSerialize::serialize(&obj.0, writer)?;
        Ok(())
    }

    pub(super) fn deserialize_third_party<
        R: io::Read,
        K: borsh::de::BorshDeserialize + Ord,
        V: borsh::de::BorshDeserialize,
    >(
        reader: &mut R,
    ) -> ::core::result::Result<ThirdParty<K, V>, io::Error> {
        Ok(ThirdParty(borsh::BorshDeserialize::deserialize_reader(
            reader,
        )?))
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Debug)]
struct A<K, V> {
    #[borsh(
        deserialize_with = "third_party_impl::deserialize_third_party",
        serialize_with = "third_party_impl::serialize_third_party",
        bound(
            serialize = "K: borsh::ser::BorshSerialize, V: borsh::ser::BorshSerialize",
            deserialize = "K: borsh::de::BorshDeserialize + Ord, V: borsh::de::BorshDeserialize",
        )
    )]
    x: ThirdParty<K, V>,
    y: u64,
}

#[allow(unused)]
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Debug)]
enum C<K, V> {
    C3(u64, u64),
    C4(
        u64,
        #[borsh(
            deserialize_with = "third_party_impl::deserialize_third_party",
            serialize_with = "third_party_impl::serialize_third_party",
            bound(
                serialize = "K: borsh::ser::BorshSerialize, V: borsh::ser::BorshSerialize",
                deserialize = "K: borsh::de::BorshDeserialize + Ord, V: borsh::de::BorshDeserialize",
            )
        )]
        ThirdParty<K, V>,
    ),
}

#[test]
fn test_overriden_struct() {
    let mut m = BTreeMap::<u64, String>::new();
    m.insert(0, "0th element".to_string());
    m.insert(1, "1st element".to_string());
    let th_p = ThirdParty(m);
    let a = A { x: th_p, y: 42 };

    let data = a.try_to_vec().unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_a = from_slice::<A<u64, String>>(&data).unwrap();
    assert_eq!(a, actual_a);
}

#[test]
fn test_overriden_enum() {
    let mut m = BTreeMap::<u64, String>::new();
    m.insert(0, "0th element".to_string());
    m.insert(1, "1st element".to_string());
    let th_p = ThirdParty(m);
    let c = C::C4(42, th_p);

    let data = c.try_to_vec().unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(data);
    let actual_c = from_slice::<C<u64, String>>(&data).unwrap();
    assert_eq!(c, actual_c);
}
