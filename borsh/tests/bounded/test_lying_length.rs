//! Over-limit length prefixes fail before any element is read; others decode like plain.

use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    format,
    string::ToString,
    vec,
    vec::Vec,
};
use core::fmt::Debug;
use core::mem::size_of;

#[cfg(feature = "hashbrown")]
use hashbrown::{HashMap, HashSet};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};

use borsh::io::ErrorKind;
use borsh::{
    from_reader, from_reader_bounded, from_slice, from_slice_bounded, to_vec, BorshDeserialize,
    BorshDeserializeBounded, BorshSerialize, Budget,
};

#[cfg(hash_collections)]
use super::cases::table;
use super::cases::{btree_nodes, sort_scratch};
use super::common::{
    assert_same_outcome, error_of, CountingReader, ERROR_UNEXPECTED_LENGTH_OF_INPUT,
};
use crate::common_macro::{limit_exceeded, OneByteReader};

/// Far above what the honest values below charge, far below a claim of `u32::MAX` elements.
const LIMIT: usize = 1024 * 1024;

/// A claim that fits [`LIMIT`] but not the input.
const FALSE_LEN: u32 = 1000;

fn with_prefix(bytes: &[u8], offset: usize, len: u32) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    bytes[offset..offset + 4].copy_from_slice(&len.to_le_bytes());
    bytes
}

/// Charge for a `u32::MAX` claim. Only below 64 bits is it an invalid size: on 64-bit
/// targets that would need elements too large for the decoder's stack.
#[cfg(target_pointer_width = "64")]
fn huge(array: fn(usize) -> usize) -> usize {
    array(u32::MAX as usize)
}

#[cfg(not(target_pointer_width = "64"))]
fn huge(_array: fn(usize) -> usize) -> usize {
    usize::MAX
}

/// Expects `(limit, requested, remaining)` and exactly `read` consumed bytes.
#[track_caller]
fn assert_limit_error<T>(bytes: &[u8], read: usize, expected: (usize, usize, usize))
where
    T: BorshDeserializeBounded + Debug,
{
    let (limit, _, remaining) = expected;
    let mut reader = CountingReader::new(bytes);
    let mut budget = Budget::from_bytes(limit);
    let err = T::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap_err();
    assert_eq!(limit_exceeded(&err), Some(expected), "{:?}: {}", bytes, err);
    assert_eq!(reader.consumed, read, "{:?}", bytes);
    assert_eq!(budget.remaining_bytes(), remaining, "{:?}", bytes);
}

#[track_caller]
fn assert_truncated<T: BorshDeserialize + Debug>(bytes: &[u8]) {
    assert_eq!(
        error_of(from_slice::<T>(bytes)),
        (
            ErrorKind::InvalidData,
            ERROR_UNEXPECTED_LENGTH_OF_INPUT.to_string()
        ),
        "{:?}",
        bytes
    );
}

#[track_caller]
fn assert_like_plain<T>(bytes: &[u8], limit: usize)
where
    T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let context = format!("{:?}", bytes);
    let mut plain_rest = bytes;
    let plain = T::deserialize_reader(&mut plain_rest);
    let mut bounded_rest = bytes;
    let bounded = T::deserialize_bounded_alloc(&mut bounded_rest, &mut Budget::from_bytes(limit));
    assert_same_outcome(&plain, &bounded, &context);
    assert_eq!(plain_rest.len(), bounded_rest.len(), "{}", context);
    assert_same_outcome(
        &from_reader::<_, T>(&mut OneByteReader(bytes)),
        &from_reader_bounded::<_, T>(&mut OneByteReader(bytes), &mut Budget::from_bytes(limit)),
        &context,
    );
}

struct Prefix {
    offset: usize,
    /// The length it holds in the honest encoding.
    len: u32,
    /// Charge before the prefix.
    before: usize,
    /// Charge for the backing vector of `n` elements.
    array: fn(usize) -> usize,
    /// Charge after that vector.
    extra: fn(usize) -> usize,
}

fn outer(len: u32, array: fn(usize) -> usize) -> Prefix {
    Prefix {
        offset: 0,
        len,
        before: 0,
        array,
        extra: |_| 0,
    }
}

fn outer_with_extra(len: u32, array: fn(usize) -> usize, extra: fn(usize) -> usize) -> Prefix {
    Prefix {
        extra,
        ..outer(len, array)
    }
}

#[track_caller]
fn check<T>(value: &T, prefix: Prefix)
where
    T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let Prefix {
        offset,
        len,
        before,
        array,
        extra,
    } = prefix;
    let input = to_vec(value).unwrap();
    assert_eq!(
        &input[offset..offset + 4],
        &len.to_le_bytes()[..],
        "{:?}",
        value
    );
    assert_eq!(
        &from_slice_bounded::<T>(&input, &mut Budget::from_bytes(LIMIT)).unwrap(),
        value
    );
    let read = offset + 4;

    let lie = with_prefix(&input, offset, u32::MAX);
    let requested = huge(array);
    assert_limit_error::<T>(&lie, read, (LIMIT, requested, LIMIT - before));
    if requested == usize::MAX {
        // An invalid allocation size must fail even at the largest limit, without allocating.
        assert_limit_error::<T>(&lie, read, (usize::MAX, usize::MAX, usize::MAX - before));
    }

    let lie = with_prefix(&input, offset, FALSE_LEN);
    assert_truncated::<T>(&lie);
    assert_like_plain::<T>(&lie, LIMIT);
    let n = FALSE_LEN as usize;
    let limit = before + array(n) + extra(n) - 1;
    let last = if extra(n) == 0 { array(n) } else { extra(n) };
    assert_limit_error::<T>(&lie, read, (limit, last, last - 1));

    for len in [0, 1] {
        assert_like_plain::<T>(&with_prefix(&input, offset, len), LIMIT);
    }
}

#[test]
fn test_sequences() {
    check(&vec![1u8, 2, 3], outer(3, |n| n));
    check(&vec![1u64, 2, 3], outer(3, |n| n * size_of::<u64>()));
    check(&"abc".to_string(), outer(3, |n| n));
    check(
        &(1..=3u32).collect::<VecDeque<_>>(),
        outer(3, |n| n * size_of::<u32>()),
    );
    check(
        &(1..=3u16).collect::<LinkedList<_>>(),
        outer_with_extra(
            3,
            |n| n * size_of::<u16>(),
            |n| n * size_of::<(usize, usize, u16)>(),
        ),
    );
}

#[test]
fn test_boxes_and_cows() {
    check(
        &vec![1u64, 2, 3].into_boxed_slice(),
        outer(3, |n| n * size_of::<u64>()),
    );
    check(&Cow::<'static, [u8]>::Borrowed(&[1, 2, 3]), outer(3, |n| n));
    check(
        &Box::new(vec![1u64, 2, 3]),
        Prefix {
            before: size_of::<Vec<u64>>(),
            ..outer(3, |n| n * size_of::<u64>())
        },
    );
}

#[test]
fn test_btree_collections() {
    check(
        &vec![(1u32, 10u64), (2, 20)]
            .into_iter()
            .collect::<BTreeMap<_, _>>(),
        outer_with_extra(
            2,
            |n| n * size_of::<(u32, u64)>(),
            |n| sort_scratch(size_of::<(u32, u64)>(), n) + btree_nodes::<u32, u64>(n),
        ),
    );
    check(
        &(1..=3u32).collect::<BTreeSet<_>>(),
        outer_with_extra(
            3,
            |n| n * size_of::<u32>(),
            |n| sort_scratch(size_of::<u32>(), n) + btree_nodes::<u32, ()>(n),
        ),
    );
}

#[cfg(hash_collections)]
#[test]
fn test_hash_collections() {
    check(
        &vec![(1u32, 10u64), (2, 20)]
            .into_iter()
            .collect::<HashMap<_, _>>(),
        outer_with_extra(
            2,
            |n| n * size_of::<(u32, u64)>(),
            |n| table(size_of::<(u32, u64)>(), n),
        ),
    );
    check(
        &(1..=3u32).collect::<HashSet<_>>(),
        outer_with_extra(3, |n| n * size_of::<u32>(), |n| table(size_of::<u32>(), n)),
    );
}

#[cfg(feature = "rc")]
#[test]
fn test_rc() {
    use alloc::{rc::Rc, sync::Arc};

    // The reference-count block is charged only after the value decodes successfully.
    check(
        &Rc::<[u32]>::from(vec![1u32, 2, 3]),
        outer(3, |n| n * size_of::<u32>()),
    );
    check(&Arc::<str>::from("abc"), outer(3, |n| n));
}

#[test]
fn test_inner_prefixes() {
    // [3] [2] 1 2 [3] 3 4 5 [1] 6: the second inner vector.
    check(
        &vec![vec![1u64, 2], vec![3, 4, 5], vec![6]],
        Prefix {
            offset: 4 + 4 + 2 * 8,
            len: 3,
            before: 3 * size_of::<Vec<u64>>() + 2 * size_of::<u64>(),
            array: |n| n * size_of::<u64>(),
            extra: |_| 0,
        },
    );

    check(
        &Some(vec![1u32, 2]),
        Prefix {
            offset: 1,
            len: 2,
            before: 0,
            array: |n| n * size_of::<u32>(),
            extra: |_| 0,
        },
    );

    // [2] 1 [1] 10 2 [2] 20 21: the second value.
    let map = vec![(1u8, vec![10u64]), (2, vec![20, 21])]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let entry = size_of::<(u8, Vec<u64>)>();
    check(
        &map,
        Prefix {
            offset: 4 + 1 + 4 + 8 + 1,
            len: 2,
            before: 2 * entry
                + sort_scratch(entry, 2)
                + btree_nodes::<u8, Vec<u64>>(2)
                + size_of::<u64>(),
            array: |n| n * size_of::<u64>(),
            extra: |_| 0,
        },
    );
}
