//! Types which are not zero-sized in memory, but always encode to zero bytes,
//! are as dangerous in a collection as zero-sized types: a 4-byte length prefix
//! drives up to `u32::MAX` iterations and allocations without consuming input.
use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{
    cell::RefCell,
    fmt::Debug,
    marker::PhantomData,
    ops::{Range, RangeFrom, RangeInclusive, RangeTo, RangeToInclusive},
};

#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};

#[cfg(feature = "hashbrown")]
use hashbrown::{HashMap, HashSet};

use borsh::{error::ERROR_ZST_FORBIDDEN, from_slice, io::ErrorKind, to_vec};
use borsh::{BorshDeserialize, BorshSerialize};

/// a length prefix of 2^27 elements, the 4 bytes `00 00 00 08`
const LARGE_LENGTH_PREFIX: [u8; 4] = [0, 0, 0, 8];

#[track_caller]
fn assert_zst_error<T: Debug>(result: borsh::io::Result<T>) {
    let err = result.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert_eq!(err.to_string(), ERROR_ZST_FORBIDDEN);
}

/// `T` is rejected as the element of every sequence, on both decode and encode
#[track_caller]
fn assert_sequences_rejected<T>(make: impl Fn() -> T)
where
    T: BorshSerialize + BorshDeserialize + Debug,
{
    assert_zst_error(from_slice::<Vec<T>>(&LARGE_LENGTH_PREFIX));
    assert_zst_error(from_slice::<VecDeque<T>>(&LARGE_LENGTH_PREFIX));
    assert_zst_error(from_slice::<LinkedList<T>>(&LARGE_LENGTH_PREFIX));

    assert_zst_error(to_vec(&vec![make()]));
    assert_zst_error(to_vec(&VecDeque::from(vec![make()])));
    assert_zst_error(to_vec(&vec![make()].into_iter().collect::<LinkedList<T>>()));
}

/// `T` is rejected as the element of every set and the key of every map,
/// on both decode and encode
#[track_caller]
fn assert_keys_rejected<T>(make: impl Fn() -> T)
where
    T: BorshSerialize + BorshDeserialize + Debug + Ord + core::hash::Hash,
{
    assert_zst_error(from_slice::<BTreeSet<T>>(&LARGE_LENGTH_PREFIX));
    assert_zst_error(from_slice::<BTreeMap<T, u8>>(&LARGE_LENGTH_PREFIX));
    assert_zst_error(to_vec(&vec![make()].into_iter().collect::<BTreeSet<T>>()));
    assert_zst_error(to_vec(
        &vec![(make(), 0u8)].into_iter().collect::<BTreeMap<T, u8>>(),
    ));

    #[cfg(hash_collections)]
    {
        assert_zst_error(from_slice::<HashSet<T>>(&LARGE_LENGTH_PREFIX));
        assert_zst_error(from_slice::<HashMap<T, u8>>(&LARGE_LENGTH_PREFIX));
        assert_zst_error(to_vec(&vec![make()].into_iter().collect::<HashSet<T>>()));
        assert_zst_error(to_vec(
            &vec![(make(), 0u8)].into_iter().collect::<HashMap<T, u8>>(),
        ));
    }

    #[cfg(feature = "indexmap")]
    {
        use indexmap::{IndexMap, IndexSet};

        assert_zst_error(from_slice::<IndexSet<T>>(&LARGE_LENGTH_PREFIX));
        assert_zst_error(from_slice::<IndexMap<T, u8>>(&LARGE_LENGTH_PREFIX));
        assert_zst_error(to_vec(&vec![make()].into_iter().collect::<IndexSet<T>>()));
        assert_zst_error(to_vec(
            &vec![(make(), 0u8)].into_iter().collect::<IndexMap<T, u8>>(),
        ));
    }
}

/// `T` is accepted in a `Vec` and round-trips
#[track_caller]
fn assert_roundtrip<T>(value: Vec<T>)
where
    T: BorshSerialize + BorshDeserialize + Debug + PartialEq,
{
    let encoded = to_vec(&value).unwrap();
    assert_eq!(from_slice::<Vec<T>>(&encoded).unwrap(), value);
}

#[test]
fn test_box() {
    assert_sequences_rejected(|| Box::new(()));
    assert_keys_rejected(|| Box::new(()));
    assert_sequences_rejected(|| Box::new([0u8; 0]));
    assert_sequences_rejected(|| Box::new(Box::new(())));
    assert_sequences_rejected(|| Box::new(PhantomData::<u64>));
}

#[test]
fn test_vec_of_vec_of_box() {
    // the inner collection is rejected, even when the outer one is not empty
    assert_zst_error(from_slice::<Vec<Vec<Box<()>>>>(&[1, 0, 0, 0, 0, 0, 0, 8]));
    assert_zst_error(to_vec(&vec![vec![Box::new(())]]));
}

#[test]
fn test_cow() {
    assert_sequences_rejected(|| Cow::<'static, ()>::Owned(()));
    assert_sequences_rejected(|| Cow::<'static, Box<()>>::Owned(Box::new(())));
}

#[test]
fn test_ref_cell() {
    // `RefCell<()>` carries a borrow flag, so it is not zero-sized in memory
    assert_sequences_rejected(|| RefCell::new(()));
    assert_sequences_rejected(|| RefCell::new(Box::new(())));
}

#[test]
fn test_ranges() {
    // `RangeInclusive<()>` carries an `exhausted` flag, so it is not zero-sized in memory
    assert_sequences_rejected(|| RangeInclusive::new((), ()));
    assert_sequences_rejected(|| Range {
        start: Box::new(()),
        end: Box::new(()),
    });
    assert_sequences_rejected(|| RangeFrom {
        start: Box::new(()),
    });
    assert_sequences_rejected(|| RangeTo { end: Box::new(()) });
    assert_sequences_rejected(|| RangeToInclusive { end: Box::new(()) });
}

#[test]
fn test_tuples_and_arrays() {
    assert_sequences_rejected(|| (Box::new(()),));
    assert_sequences_rejected(|| (Box::new(()), (), Box::new([0u64; 0])));
    assert_keys_rejected(|| (Box::new(()), Box::new(())));
    assert_sequences_rejected(|| [Box::new(()), Box::new(())]);
    assert_sequences_rejected(|| Box::new([] as [Box<()>; 0]));
}

#[test]
fn test_serialize_references() {
    let unit = Box::new(());
    assert_zst_error(to_vec(&vec![&unit]));
    assert_zst_error(to_vec(&vec![&&unit]));
}

/// A manual impl for a zero-sized type, which encodes a tag byte. Manual impls don't
/// override the hidden constant, so nothing is inferred about their encoding.
#[derive(Debug, Clone, PartialEq)]
struct Tag;

impl BorshSerialize for Tag {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        7u8.serialize(writer)
    }
}

impl BorshDeserialize for Tag {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        match u8::deserialize_reader(reader)? {
            7 => Ok(Tag),
            _ => Err(borsh::io::Error::new(ErrorKind::InvalidData, "not a tag")),
        }
    }
}

#[test]
fn test_manual_impl_of_zero_sized_type() {
    // unchanged: zero-sized in memory, so it is rejected directly
    assert_zst_error(from_slice::<Vec<Tag>>(&LARGE_LENGTH_PREFIX));
    assert_zst_error(to_vec(&vec![Tag]));
    // unchanged: behind a wrapper it is accepted, as the encoding of a manual impl is unknown
    assert_roundtrip(vec![Box::new(Tag), Box::new(Tag)]);
    assert_roundtrip(vec![RefCell::new(Tag)]);
    assert_roundtrip(vec![(Tag, Box::new(()))]);
}

#[test]
fn test_legitimate_types_are_accepted() {
    assert_roundtrip(vec![Box::new(1u8), Box::new(2u8)]);
    assert_roundtrip(vec![Box::new(String::from("a")), Box::new(String::new())]);
    assert_roundtrip::<Box<[u8]>>(vec![Box::new([1u8, 2]), Box::new([])]);
    assert_roundtrip::<Box<str>>(vec!["a".into(), "".into()]);
    assert_roundtrip(vec![Cow::<'static, str>::Owned("a".into())]);
    assert_roundtrip(vec![RefCell::new(1u8)]);
    assert_roundtrip(vec![1u8..2, 3..4]);
    assert_roundtrip(vec![[Box::new(1u8), Box::new(2u8)]]);
    // a tuple is fine as long as one of its elements encodes some bytes
    assert_roundtrip(vec![(Box::new(()), 1u8), (Box::new(()), 2u8)]);
    // `Option` and `Result` always encode a tag byte
    assert_roundtrip(vec![Some(Box::new(())), None]);
    assert_roundtrip::<Result<Box<()>, ()>>(vec![Ok(Box::new(())), Err(())]);
    // only keys are checked in maps, values may encode to zero bytes
    let map: BTreeMap<u8, Box<()>> = vec![(1u8, Box::new(()))].into_iter().collect();
    assert_eq!(
        from_slice::<BTreeMap<u8, Box<()>>>(&to_vec(&map).unwrap()).unwrap(),
        map
    );
    #[cfg(hash_collections)]
    {
        let map: HashMap<u8, Box<()>> = vec![(1u8, Box::new(()))].into_iter().collect();
        assert_eq!(
            from_slice::<HashMap<u8, Box<()>>>(&to_vec(&map).unwrap()).unwrap(),
            map
        );
    }
    // a `Box` of an empty collection is not zero-encoded: the length prefix is encoded
    assert_roundtrip(vec![Box::new(Vec::<u8>::new())]);
}

#[cfg(feature = "rc")]
mod rc {
    use super::*;
    use alloc::{rc::Rc, sync::Arc};

    #[test]
    fn test_rc_and_arc() {
        assert_sequences_rejected(|| Rc::new(()));
        assert_keys_rejected(|| Rc::new(()));
        assert_sequences_rejected(|| Arc::new(()));
        assert_keys_rejected(|| Arc::new(()));
        assert_sequences_rejected(|| Rc::new(Box::new(())));
        assert_sequences_rejected(|| Arc::new(RefCell::new(())));
    }

    #[test]
    fn test_rc_and_arc_legitimate() {
        assert_roundtrip(vec![Rc::new(String::from("a"))]);
        assert_roundtrip(vec![Arc::new(1u64)]);
        assert_roundtrip::<Rc<[u8]>>(vec![Rc::from(&[1u8, 2][..])]);
        assert_roundtrip::<Arc<str>>(vec![Arc::from("a")]);
        assert_roundtrip(vec![Rc::new(Tag)]);
        assert_roundtrip(vec![Arc::new(Tag)]);
    }

    #[cfg(feature = "derive")]
    mod derived {
        use super::*;

        mod inner {
            pub type Packet = alloc::rc::Rc<()>;
        }

        /// a field type that mentions the name of the struct, without any recursion
        #[derive(BorshSerialize, BorshDeserialize, Debug)]
        struct Packet(inner::Packet);

        #[derive(BorshSerialize, BorshDeserialize, Debug)]
        struct MarkerPacket(Rc<()>, PhantomData<Self>);

        #[test]
        fn test_field_types_mentioning_the_struct() {
            assert_sequences_rejected(|| Packet(Rc::new(())));
            assert_sequences_rejected(|| MarkerPacket(Rc::new(()), PhantomData));
        }
    }
}

#[cfg(feature = "derive")]
mod derived {
    use super::*;

    #[derive(
        BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
    )]
    struct Wrapper(Box<()>);

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct Named {
        a: Box<()>,
        b: (),
        c: [Box<[u8; 0]>; 2],
    }

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct Nested(Wrapper, Named, Box<Wrapper>);

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct Generic<T>(T);

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct GenericWhere<T>
    where
        T: Clone,
    {
        value: Box<T>,
    }

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct Skipped {
        #[borsh(skip)]
        x: u64,
    }

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct PartlySkipped {
        a: Box<()>,
        #[borsh(skip)]
        b: u64,
    }

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct Unit;

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct WithBytes(Box<()>, u8);

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    enum SingleVariant {
        A,
    }

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    enum OnlyZeroEncodedFields {
        A(Box<()>),
        B { x: () },
    }

    #[test]
    fn test_derived_structs() {
        assert_sequences_rejected(|| Wrapper(Box::new(())));
        assert_keys_rejected(|| Wrapper(Box::new(())));
        assert_sequences_rejected(|| Named {
            a: Box::new(()),
            b: (),
            c: [Box::new([]), Box::new([])],
        });
        assert_sequences_rejected(|| {
            Nested(
                Wrapper(Box::new(())),
                Named {
                    a: Box::new(()),
                    b: (),
                    c: [Box::new([]), Box::new([])],
                },
                Box::new(Wrapper(Box::new(()))),
            )
        });
        assert_sequences_rejected(|| Box::new(Unit));
        assert_sequences_rejected(|| Box::new(Wrapper(Box::new(()))));
    }

    #[test]
    fn test_derived_generic_structs() {
        assert_sequences_rejected(|| Generic(Box::new(())));
        assert_sequences_rejected(|| Generic(Generic(Wrapper(Box::new(())))));
        assert_sequences_rejected(|| GenericWhere {
            value: Box::new(()),
        });
        assert_roundtrip(vec![Generic(Box::new(1u8))]);
        assert_roundtrip(vec![GenericWhere {
            value: Box::new(String::from("a")),
        }]);
    }

    #[test]
    fn test_derived_skipped_fields() {
        // all fields skipped: 8 bytes in memory, nothing on the wire
        assert_sequences_rejected(|| Skipped { x: 1 });
        assert_sequences_rejected(|| PartlySkipped {
            a: Box::new(()),
            b: 1,
        });
    }

    #[test]
    fn test_derived_legitimate() {
        assert_roundtrip(vec![WithBytes(Box::new(()), 1)]);
        assert_roundtrip(vec![
            OnlyZeroEncodedFields::A(Box::new(())),
            OnlyZeroEncodedFields::B { x: () },
        ]);
        // an enum encodes its tag, so a `Box` of a single-variant fieldless enum is fine
        assert_roundtrip(vec![Box::new(SingleVariant::A)]);
    }

    /// recursive, inhabited and zero-encoded: `ZeroArray(Box::new(()), [])`
    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct ZeroArray(Box<()>, [Box<ZeroArray>; 0]);

    /// mutually recursive through an empty array, inhabited and not zero-encoded
    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct MutualA(Box<MutualB>);

    #[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
    struct MutualB([Box<MutualA>; 0], u8);

    #[test]
    fn test_derived_recursive_through_empty_array() {
        assert_sequences_rejected(|| ZeroArray(Box::new(()), []));
        assert_roundtrip(vec![
            MutualA(Box::new(MutualB([], 7))),
            MutualA(Box::new(MutualB([], 8))),
        ]);
        assert_roundtrip(vec![MutualB([], 7)]);
    }

    #[test]
    fn test_derived_single_variant_enum_unchanged() {
        // unchanged and deliberately conservative: such an enum is zero-sized in memory,
        // although it encodes a tag byte
        assert_zst_error(from_slice::<Vec<SingleVariant>>(&LARGE_LENGTH_PREFIX));
        assert_zst_error(to_vec(&vec![SingleVariant::A]));
    }
}
