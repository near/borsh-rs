//! Checks bounded derives and `#[borsh(...)]` attributes for parity with plain decoding,
//! charge thresholds, and exact charges where known. Parity includes enum tags and truncations.

use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt::Debug;
use core::mem::size_of;

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;

use borsh::de::{EnumExt, EnumExtBounded};
use borsh::io::{ErrorKind, Read, Result};
use borsh::{
    from_reader_bounded, from_slice, from_slice_bounded, to_vec, BorshDeserialize,
    BorshDeserializeBounded, BorshSerialize, Budget,
};

use super::cases::btree_nodes;
use super::cases::derived::Record;
#[cfg(hash_collections)]
use super::cases::table;
use super::common::{self, assert_truncations_fail_alike, kind_and_message};
use crate::common_macro::{assert_bounded_parity, limit_exceeded, OneByteReader, PARITY_LIMIT};

fn decode_bounded<T: BorshDeserializeBounded>(input: &[u8], limit: usize) -> [Result<T>; 2] {
    [
        from_slice_bounded::<T>(input, &mut Budget::from_bytes(limit)),
        from_reader_bounded::<_, T>(&mut OneByteReader(input), &mut Budget::from_bytes(limit)),
    ]
}

/// Checks parity and the entry points' threshold; returns the `deserialize_bounded_alloc` charge.
#[track_caller]
fn assert_decodes<T>(input: &[u8], expected: &T) -> usize
where
    T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    assert_eq!(&from_slice::<T>(input).unwrap(), expected);
    assert_bounded_parity::<T>(input);

    let mut reader = input;
    let mut budget = Budget::from_bytes(PARITY_LIMIT);
    let decoded = T::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    assert_eq!(&decoded, expected);
    assert!(reader.is_empty());
    let used = PARITY_LIMIT - budget.remaining_bytes();

    let entry = size_of::<T>() + used;
    for limit in [entry, entry + 1] {
        for decoded in decode_bounded::<T>(input, limit) {
            assert_eq!(&decoded.unwrap(), expected);
        }
    }
    if entry > 0 {
        for limit in [entry - 1, entry / 2, 0] {
            for decoded in decode_bounded::<T>(input, limit) {
                let err = decoded.unwrap_err();
                let reported = limit_exceeded(&err).map(|(reported, _, _)| reported);
                assert_eq!(reported, Some(limit), "{:?}: {}", expected, err);
            }
        }
    }
    used
}

#[track_caller]
fn assert_roundtrip<T>(value: &T) -> usize
where
    T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    assert_decodes(&to_vec(value).unwrap(), value)
}

#[track_caller]
fn assert_charge<T>(input: &[u8], expected: &T, charge: usize, requested: usize, remaining: usize)
where
    T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    assert_eq!(assert_decodes(input, expected), charge, "{:?}", expected);
    common::assert_charge(input, expected, charge, requested, remaining);
}

fn unexpected_tag(tag: u8) -> (ErrorKind, String) {
    (
        ErrorKind::InvalidData,
        format!("Unexpected variant tag: {:?}", tag),
    )
}

/// Invalid tags must fail before any charge after the root's.
#[track_caller]
fn assert_every_tag<T>(tags: &[u8], payload: &[u8])
where
    T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    for tag in 0..=255u8 {
        let mut input = vec![tag];
        input.extend_from_slice(payload);
        assert_bounded_parity::<T>(&input);

        let plain = from_slice::<T>(&input).map_err(|err| kind_and_message(&err));
        if tags.contains(&tag) {
            assert_ne!(plain.err(), Some(unexpected_tag(tag)));
        } else {
            assert_eq!(plain.err(), Some(unexpected_tag(tag)));
            let bounded = from_slice_bounded::<T>(&input, &mut Budget::from_bytes(size_of::<T>()))
                .unwrap_err();
            assert_eq!(kind_and_message(&bounded), unexpected_tag(tag));
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct Unit;

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
enum Shape {
    Unit,
    Tuple(u8, Vec<u64>),
}

#[derive(Debug, PartialEq)]
struct Preallocated(Vec<u8>);

impl Default for Preallocated {
    fn default() -> Self {
        Preallocated(vec![0; 100])
    }
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct SkipsPreallocated {
    value: u32,
    #[borsh(skip)]
    buffer: Preallocated,
}

fn record(cache: Vec<u8>) -> Record {
    Record {
        id: 7,
        numbers: vec![1, 2, 3],
        digest: Box::new([7; 16]),
        cache,
        name: "abc".to_string(),
    }
}

#[test]
fn test_derived_structs_charge_their_fields() {
    let input = to_vec(&record(vec![9; 100])).unwrap();
    assert_charge(&input, &record(Vec::new()), 12 + 16 + 3, 3, 2);
    assert_truncations_fail_alike::<Record>(&input);

    assert_eq!(assert_roundtrip(&Unit), 0);
}

#[test]
fn test_derived_skip_default_is_not_charged() {
    let value = SkipsPreallocated {
        value: 7,
        buffer: Preallocated(Vec::new()),
    };
    let input = to_vec(&value).unwrap();
    assert_eq!(input, to_vec(&7u32).unwrap());
    let expected = SkipsPreallocated {
        value: 7,
        buffer: Preallocated::default(),
    };
    assert_eq!(assert_decodes(&input, &expected), 0);
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, Clone, PartialEq, Debug)]
struct Inner {
    name: String,
    numbers: Vec<u64>,
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct Middle {
    inner: Inner,
    extra: u32,
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct Outer {
    middle: Middle,
    list: Vec<Inner>,
    boxed: Box<Inner>,
}

/// A struct nested by value is part of its parent's size, so only its fields charge.
#[test]
fn test_derived_nested_structs_charge_once() {
    let make = |name: &str, n: usize| Inner {
        name: name.to_string(),
        numbers: vec![7; n],
    };
    let value = Outer {
        middle: Middle {
            inner: make("abc", 2),
            extra: 1,
        },
        list: vec![make("de", 1), make("f", 0)],
        boxed: Box::new(make("gh", 3)),
    };
    let input = to_vec(&value).unwrap();
    let inner = size_of::<Inner>();
    // `middle.inner`'s fields, `list`'s buffer and its elements' fields, then the box and its
    // fields; the boxed numbers come last.
    let charge = (3 + 2 * 8) + (2 * inner + 2 + 8 + 1) + (inner + 2 + 3 * 8);
    assert_charge(&input, &value, charge, 24, 23);
    assert_truncations_fail_alike::<Outer>(&input);
}

#[test]
fn test_derived_enum_variants_charge_their_fields() {
    assert_eq!(assert_roundtrip(&Shape::Unit), 0);

    let value = Shape::Tuple(7, vec![1, 2, 3]);
    let input = to_vec(&value).unwrap();
    assert_charge(&input, &value, 24, 24, 23);
    assert_truncations_fail_alike::<Shape>(&input);
}

#[test]
fn test_derived_enum_ext_bounded_charges_like_the_derived_impl() {
    let value = Shape::Tuple(7, vec![1, 2, 3]);
    let input = to_vec(&value).unwrap();
    let charge = common::charged::<Shape>(&input);

    let (&tag, rest) = input.split_first().unwrap();
    let mut reader = rest;
    let mut budget = Budget::from_bytes(PARITY_LIMIT);
    let decoded = Shape::deserialize_variant_bounded_alloc(&mut reader, tag, &mut budget).unwrap();
    assert_eq!(decoded, value);
    assert!(reader.is_empty());
    assert_eq!(PARITY_LIMIT - budget.remaining_bytes(), charge);

    let input = to_vec(&vec![1u64, 2]).unwrap();
    let mut reader = &input[..];
    let mut budget = Budget::from_bytes(100);
    let err = Shape::deserialize_variant_bounded_alloc(&mut reader, 3, &mut budget).unwrap_err();
    assert_eq!(kind_and_message(&err), unexpected_tag(3));
    let plain = Shape::deserialize_variant(&mut &input[..], 3).unwrap_err();
    assert_eq!(kind_and_message(&plain), unexpected_tag(3));
    assert_eq!(reader.len(), input.len());
    assert_eq!(budget.remaining_bytes(), 100);
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
#[borsh(init = init)]
struct WithInit {
    text: String,
    #[borsh(skip)]
    words: Vec<String>,
    #[borsh(skip)]
    init_calls: u8,
}

impl WithInit {
    fn init(&mut self) {
        self.init_calls += 1;
        self.words = self.text.split(' ').map(String::from).collect();
    }
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
#[borsh(init = init)]
enum InitEnum {
    Numbers {
        numbers: Vec<u32>,
        #[borsh(skip)]
        sorted: Vec<u32>,
        #[borsh(skip)]
        init_calls: u8,
    },
}

impl InitEnum {
    fn init(&mut self) {
        match self {
            InitEnum::Numbers {
                numbers,
                sorted,
                init_calls,
            } => {
                *init_calls += 1;
                *sorted = numbers.clone();
                sorted.sort_unstable();
            }
        }
    }
}

fn init_numbers(numbers: &[u32], sorted: &[u32], init_calls: u8) -> InitEnum {
    InitEnum::Numbers {
        numbers: numbers.to_vec(),
        sorted: sorted.to_vec(),
        init_calls,
    }
}

#[test]
fn test_derived_init_runs_once_and_is_not_charged() {
    let text = "decoded with a limit";
    let input = to_vec(&WithInit {
        text: text.to_string(),
        words: Vec::new(),
        init_calls: 0,
    })
    .unwrap();
    let expected = WithInit {
        text: text.to_string(),
        words: vec![
            "decoded".to_string(),
            "with".to_string(),
            "a".to_string(),
            "limit".to_string(),
        ],
        init_calls: 1,
    };
    assert_charge(&input, &expected, text.len(), text.len(), text.len() - 1);

    let input = to_vec(&init_numbers(&[3, 1, 2], &[], 0)).unwrap();
    let expected = init_numbers(&[3, 1, 2], &[1, 2, 3], 1);
    assert_charge(&input, &expected, 12, 12, 11);
}

/// Stands in for a type from another crate, which has no borsh impls.
#[derive(Debug, PartialEq)]
struct ThirdParty<K, V>(BTreeMap<K, V>);

mod third_party_impl {
    use super::ThirdParty;
    use alloc::collections::BTreeMap;
    use borsh::io::{Read, Result, Write};
    use borsh::{BorshDeserialize, BorshDeserializeBounded, BorshSerialize, Budget};

    pub(super) fn serialize<K, V, W>(value: &ThirdParty<K, V>, writer: &mut W) -> Result<()>
    where
        K: BorshSerialize,
        V: BorshSerialize,
        W: Write,
    {
        value.0.serialize(writer)
    }

    pub(super) fn deserialize<R, K, V>(reader: &mut R) -> Result<ThirdParty<K, V>>
    where
        R: Read,
        K: BorshDeserialize + Ord,
        V: BorshDeserialize,
    {
        BTreeMap::deserialize_reader(reader).map(ThirdParty)
    }

    pub(super) fn deserialize_bounded<R, K, V>(
        reader: &mut R,
        budget: &mut Budget,
    ) -> Result<ThirdParty<K, V>>
    where
        R: Read,
        K: BorshDeserializeBounded + Ord,
        V: BorshDeserializeBounded,
    {
        BTreeMap::deserialize_bounded_alloc(reader, budget).map(ThirdParty)
    }
}

/// The automatic bounds (`K: BorshDeserializeBounded`) are not enough for the functions,
/// which need `K: Ord`, so this only compiles with both overrides.
#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct WithThirdParty<K, V> {
    #[borsh(
        serialize_with = "third_party_impl::serialize",
        deserialize_with = "third_party_impl::deserialize",
        deserialize_with_bounded = "third_party_impl::deserialize_bounded",
        bound(
            deserialize = "K: borsh::de::BorshDeserialize + Ord, V: borsh::de::BorshDeserialize",
            deserialize_bounded = "K: borsh::de::BorshDeserializeBounded + Ord,
                                   V: borsh::de::BorshDeserializeBounded",
        )
    )]
    map: ThirdParty<K, V>,
    tail: Vec<u8>,
}

/// Returns the map and its charge excluding string contents.
fn third_party() -> (ThirdParty<u64, String>, usize) {
    let map = BTreeMap::from([(0, "zero".to_string()), (1, "one".to_string())]);
    (ThirdParty(map), btree_nodes::<u64, String>(2))
}

#[test]
fn test_derived_deserialize_with_bounded_on_a_struct_field() {
    let (map, map_charge) = third_party();
    let value = WithThirdParty {
        map,
        tail: vec![1, 2, 3],
    };
    let input = to_vec(&value).unwrap();
    // The map, its two strings, then the tail.
    assert_charge(&input, &value, map_charge + 4 + 3 + 3, 3, 2);
    assert_truncations_fail_alike::<WithThirdParty<u64, String>>(&input);
}

/// Has a plain impl and no bounded one.
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
struct PlainOnly(Vec<u64>);

fn plain_only_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<PlainOnly> {
    let len = u32::deserialize_bounded_alloc(reader, budget)? as usize;
    budget.charge_array::<u64>(len)?;
    let mut numbers = Vec::with_capacity(len);
    for _ in 0..len {
        numbers.push(u64::deserialize_bounded_alloc(reader, budget)?);
    }
    Ok(PlainOnly(numbers))
}

/// `deserialize_with_bounded` alone: the plain derive decodes the field with its plain impl.
#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct UsesPlainOnly {
    #[borsh(deserialize_with_bounded = "plain_only_bounded")]
    numbers: PlainOnly,
    name: String,
}

#[test]
fn test_derived_deserialize_with_bounded_alone() {
    let value = UsesPlainOnly {
        numbers: PlainOnly(vec![1, 2, 3]),
        name: "ab".to_string(),
    };
    let input = to_vec(&value).unwrap();
    assert_charge(&input, &value, 24 + 2, 2, 1);
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
enum ByDiscriminant {
    Low = 1,
    Bytes(Vec<u8>) = 20,
    Next,
    Text { text: String } = 200,
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
#[borsh(use_discriminant = false)]
#[repr(u8)]
enum ByIndex {
    Low = 1,
    Bytes(Vec<u8>) = 20,
    Next,
    Text { text: String } = 200,
}

/// `values` are `Low`, `Bytes(vec![1, 2, 3])`, `Next` and `Text { text: "abcd" }`, in this
/// order, and are written with `tags`.
#[track_caller]
fn assert_tags<T>(values: [T; 4], tags: [u8; 4])
where
    T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    for ((value, tag), charge) in values.iter().zip(tags).zip([0, 3, 0, 4]) {
        let input = to_vec(value).unwrap();
        assert_eq!(input[0], tag);
        if charge == 0 {
            assert_eq!(assert_decodes(&input, value), 0);
        } else {
            assert_charge(&input, value, charge, charge, charge - 1);
        }
    }
    assert_every_tag::<T>(&tags, &to_vec("abc").unwrap());
}

#[test]
fn test_derived_use_discriminant_true() {
    let values = [
        ByDiscriminant::Low,
        ByDiscriminant::Bytes(vec![1, 2, 3]),
        ByDiscriminant::Next,
        ByDiscriminant::Text {
            text: "abcd".to_string(),
        },
    ];
    assert_tags(values, [1, 20, 21, 200]);
}

#[test]
fn test_derived_use_discriminant_false() {
    let values = [
        ByIndex::Low,
        ByIndex::Bytes(vec![1, 2, 3]),
        ByIndex::Next,
        ByIndex::Text {
            text: "abcd".to_string(),
        },
    ];
    assert_tags(values, [0, 1, 2, 3]);
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, Clone, PartialEq, Debug)]
enum Expr {
    Num(i64),
    Neg(Box<Self>),
    Add(Box<Self>, Box<Self>),
    Call { name: String, args: Vec<Self> },
}

#[test]
fn test_derived_recursive_boxes_and_vecs() {
    use Expr::{Add, Call, Neg, Num};

    let expr = size_of::<Expr>();
    let value = Call {
        name: "sum".to_string(),
        args: vec![
            Num(1),
            Neg(Box::new(Num(2))),
            Add(
                Box::new(Num(3)),
                Box::new(Call {
                    name: "g".to_string(),
                    args: vec![],
                }),
            ),
        ],
    };
    let input = to_vec(&value).unwrap();
    // The name, the arguments, then each box before its content: the one of `Neg`, both of
    // `Add`, and last the name `g`.
    assert_charge(&input, &value, 3 + 3 * expr + 3 * expr + 1, 1, 0);
    assert_truncations_fail_alike::<Expr>(&input);
}

#[cfg(hash_collections)]
#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct Dir {
    files: Vec<String>,
    dirs: HashMap<String, Self>,
}

#[cfg(hash_collections)]
#[test]
fn test_derived_recursive_hash_map() {
    fn dir(files: &[&str], dirs: Vec<(&str, Dir)>) -> Dir {
        Dir {
            files: files.iter().map(|file| file.to_string()).collect(),
            dirs: dirs
                .into_iter()
                .map(|(name, dir)| (name.to_string(), dir))
                .collect(),
        }
    }

    let value = dir(
        &["a.txt", "bb"],
        vec![("sub", dir(&[], vec![])), ("x", dir(&["c"], vec![]))],
    );
    let input = to_vec(&value).unwrap();
    let string = size_of::<String>();
    let entry = size_of::<(String, Dir)>();
    // The files, the map's table, then the entries in key order: "sub" and its empty
    // directory, "x" and its file "c".
    let charge = (2 * string + 5 + 2) + table(entry, 2) + 3 + (1 + string + 1);
    assert_charge(&input, &value, charge, 1, 0);
    assert_truncations_fail_alike::<Dir>(&input);
}

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct WithCow<'a> {
    name: Cow<'a, str>,
    numbers: Cow<'a, [u64]>,
    labels: Vec<Cow<'a, str>>,
}

#[test]
fn test_derived_cow_and_lifetimes() {
    let numbers = [1u64, 2];
    let value = WithCow {
        name: Cow::Borrowed("abc"),
        numbers: Cow::Borrowed(&numbers),
        labels: vec![Cow::Borrowed("x"), Cow::Owned("yz".to_string())],
    };
    let input = to_vec(&value).unwrap();
    let charge = 3 + 16 + 2 * size_of::<Cow<str>>() + 1 + 2;
    assert_charge(&input, &value, charge, 2, 1);
    let limit = size_of::<WithCow<'static>>() + charge;
    let decoded =
        from_slice_bounded::<WithCow<'static>>(&input, &mut Budget::from_bytes(limit)).unwrap();
    assert!(matches!(decoded.name, Cow::Owned(_)));
}
