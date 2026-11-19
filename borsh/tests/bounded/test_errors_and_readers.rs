//! Parity with plain decoding in errors and reader calls.

use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::any::type_name;
use core::cell::{Cell, RefCell};
use core::fmt::Debug;
use core::marker::PhantomData;
use core::mem::size_of;
use core::ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};

#[cfg(feature = "hashbrown")]
use hashbrown::{HashMap, HashSet};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};

use borsh::error::ERROR_ZST_FORBIDDEN;
use borsh::io::{Error, ErrorKind, Read, Result};
use borsh::{
    from_reader, from_reader_bounded, from_slice, from_slice_bounded, to_vec, BorshDeserialize,
    BorshDeserializeBounded, BorshSerialize, Budget,
};

use super::cases::MIB;
use super::common::{assert_same_outcome, error_of, ERROR_UNEXPECTED_LENGTH_OF_INPUT};
use crate::common_macro::{limit_exceeded, PARITY_LIMIT};

/// Checks parity and returns the plain result plus both readers after decoding.
#[track_caller]
fn decode_both<T, R>(reader: &R, context: &str) -> (Result<T>, R, R)
where
    T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
    R: Read + Clone,
{
    let mut plain_reader = reader.clone();
    let plain = from_reader::<_, T>(&mut plain_reader);
    let mut bounded_reader = reader.clone();
    let bounded =
        from_reader_bounded::<_, T>(&mut bounded_reader, &mut Budget::from_bytes(PARITY_LIMIT));
    assert_same_outcome(&plain, &bounded, context);
    (plain, plain_reader, bounded_reader)
}

fn take(input: &mut &[u8], buf: &mut [u8], max: usize) -> usize {
    let n = buf.len().min(input.len()).min(max);
    buf[..n].copy_from_slice(&input[..n]);
    *input = &input[n..];
    n
}

/// Records requested buffer lengths, including calls from the default `read_exact`.
#[derive(Clone)]
struct TraceReader<'a> {
    input: &'a [u8],
    max_read: usize,
    trace: Vec<usize>,
}

impl<'a> TraceReader<'a> {
    fn new(input: &'a [u8], max_read: usize) -> Self {
        TraceReader {
            input,
            max_read,
            trace: Vec::new(),
        }
    }
}

impl Read for TraceReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.trace.push(buf.len());
        Ok(take(&mut self.input, buf, self.max_read))
    }
}

/// Fails with `kind` after `until` bytes; `Interrupted` fails once, as decoding retries it.
#[derive(Clone)]
struct FailAfter<'a> {
    input: &'a [u8],
    until: Option<usize>,
    kind: ErrorKind,
}

impl Read for FailAfter<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self.until {
            Some(0) => {
                if self.kind == ErrorKind::Interrupted {
                    self.until = None;
                }
                Err(Error::new(self.kind, "custom failure"))
            }
            Some(until) => {
                let read = take(&mut self.input, buf, until);
                self.until = Some(until - read);
                Ok(read)
            }
            None => Ok(take(&mut self.input, buf, usize::MAX)),
        }
    }
}

/// The ZST error comes before the length is read and before anything is charged.
#[track_caller]
fn assert_zst_rejected_first<T>()
where
    T: BorshDeserialize + BorshDeserializeBounded + Debug,
{
    let expected = (ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN.to_string());
    for input in [&[][..], &[0xff, 0xff, 0xff, 0xff][..]] {
        assert_eq!(
            error_of(from_slice::<T>(input)),
            expected,
            "{}",
            type_name::<T>()
        );
        let mut reader = super::common::CountingReader::new(input);
        let bounded = T::deserialize_bounded_alloc(&mut reader, &mut Budget::from_bytes(0));
        assert_eq!(error_of(bounded), expected, "{}", type_name::<T>());
        assert_eq!(reader.consumed, 0, "{}", type_name::<T>());
    }
}

#[test]
fn test_zero_sized_elements_are_rejected_first() {
    assert_zst_rejected_first::<Vec<()>>();
    assert_zst_rejected_first::<VecDeque<()>>();
    assert_zst_rejected_first::<LinkedList<()>>();
    assert_zst_rejected_first::<Box<[()]>>();
    assert_zst_rejected_first::<BTreeSet<()>>();
    assert_zst_rejected_first::<BTreeMap<(), u8>>();
    #[cfg(hash_collections)]
    assert_zst_rejected_first::<HashMap<(), u8>>();
    #[cfg(feature = "rc")]
    assert_zst_rejected_first::<alloc::rc::Rc<[()]>>();

    // Not zero-sized, but encode to zero bytes: only `ENCODES_TO_ZERO_BYTES` rejects them.
    assert_zst_rejected_first::<Vec<(Box<()>,)>>();
    assert_zst_rejected_first::<Vec<Range<Box<()>>>>();
    assert_zst_rejected_first::<Vec<Cow<'static, ()>>>();
    assert_zst_rejected_first::<Vec<RefCell<Box<()>>>>();
    assert_zst_rejected_first::<Vec<[Box<()>; 2]>>();
    assert_zst_rejected_first::<Vec<Box<PhantomData<u8>>>>();
    assert_zst_rejected_first::<BTreeMap<Box<()>, u8>>();
    #[cfg(hash_collections)]
    assert_zst_rejected_first::<HashMap<Box<()>, u8>>();
    #[cfg(feature = "rc")]
    assert_zst_rejected_first::<Vec<alloc::rc::Rc<()>>>();
}

/// Each bounded impl's `ENCODES_TO_ZERO_BYTES` is its plain twin's.
#[test]
fn test_zero_bytes_flags_match_plain() {
    macro_rules! assert_flags_match {
        ($($t:ty),* $(,)?) => {$(
            assert_eq!(
                <$t as BorshDeserialize>::ENCODES_TO_ZERO_BYTES,
                <$t as BorshDeserializeBounded>::ENCODES_TO_ZERO_BYTES,
                "{}",
                stringify!($t)
            );
        )*};
    }
    type Res<T, E> = core::result::Result<T, E>;

    assert_flags_match!(u8, u64, i128, f64, bool, usize, core::num::NonZeroU32);
    assert_flags_match!(core::net::IpAddr, core::net::SocketAddrV6);
    assert_flags_match!((), RangeFull, PhantomData<u8>, String);
    assert_flags_match!(Vec<u8>, Vec<()>, VecDeque<()>, LinkedList<()>);
    assert_flags_match!(Box<u8>, Box<()>, Box<[u8]>, Box<str>, Box<PhantomData<u8>>);
    assert_flags_match!(Option<()>, Res<(), ()>);
    assert_flags_match!(Cow<'static, u8>, Cow<'static, ()>, Cow<'static, [u8]>);
    assert_flags_match!(BTreeSet<u8>, BTreeMap<u8, ()>);
    #[cfg(hash_collections)]
    assert_flags_match!(HashSet<u8>, HashMap<u8, ()>);
    assert_flags_match!([u8; 0], [u8; 2], [(); 2], [Box<()>; 2]);
    assert_flags_match!((u8,), ((),), ((), u8), (Box<()>, PhantomData<u8>, ()));
    assert_flags_match!(Range<u8>, Range<()>, RangeInclusive<()>, RangeFrom<()>);
    assert_flags_match!(RangeTo<()>, RangeToInclusive<()>);
    assert_flags_match!(Cell<u8>, Cell<()>, RefCell<u8>, RefCell<Box<()>>);
    #[cfg(feature = "rc")]
    assert_flags_match!(
        alloc::rc::Rc<u8>,
        alloc::rc::Rc<()>,
        alloc::rc::Rc<[u8]>,
        alloc::sync::Arc<()>,
        alloc::sync::Arc<str>,
    );
}

#[test]
fn test_zero_sized_map_values_are_allowed() {
    #[track_caller]
    fn check<M>(len: fn(&M) -> usize)
    where
        M: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
    {
        let input = to_vec(&vec![(1u8, ()), (2, ())]).unwrap();
        let plain = from_slice::<M>(&input);
        assert_eq!(len(plain.as_ref().unwrap()), 2, "{}", type_name::<M>());
        let bounded = from_slice_bounded::<M>(&input, &mut Budget::from_bytes(PARITY_LIMIT));
        assert_same_outcome(&plain, &bounded, type_name::<M>());
    }
    check::<BTreeMap<u8, ()>>(BTreeMap::len);
    check::<BTreeMap<u8, Box<()>>>(BTreeMap::len);
    #[cfg(hash_collections)]
    {
        check::<HashMap<u8, ()>>(HashMap::len);
        check::<HashMap<u8, Box<()>>>(HashMap::len);
    }
}

#[test]
fn test_nested_zero_sized_elements_are_rejected_after_the_outer_charge() {
    let input = [1, 0, 0, 0];
    let element = size_of::<Vec<()>>();
    let charge = size_of::<Vec<Vec<()>>>() + element;
    let expected = (ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN.to_string());
    assert_eq!(error_of(from_slice::<Vec<Vec<()>>>(&input)), expected);
    assert_eq!(
        error_of(from_slice_bounded::<Vec<Vec<()>>>(
            &input,
            &mut Budget::from_bytes(charge)
        )),
        expected
    );
    let err = from_slice_bounded::<Vec<Vec<()>>>(&input, &mut Budget::from_bytes(charge - 1))
        .unwrap_err();
    assert_eq!(
        limit_exceeded(&err),
        Some((charge - 1, element, element - 1)),
        "{}",
        err
    );
}

/// Bounded decoding counts the same nesting levels as plain decoding.
#[cfg(feature = "std")]
#[test]
fn test_recursion_limit_parity() {
    use crate::common_macro::assert_bounded_parity;

    #[track_caller]
    fn check<T>(value: &T)
    where
        T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
    {
        let input = to_vec(value).unwrap();
        for limit in 0..=2 {
            borsh::de::with_recursion_limit(limit, || {
                assert_bounded_parity::<T>(&input);
                Ok(())
            })
            .unwrap();
        }
    }
    // One or two levels each.
    let bytes = || vec![1u8, 2];
    check(&vec![bytes(), vec![]]);
    check(&VecDeque::from(vec![bytes()]));
    check(&vec![bytes()].into_iter().collect::<LinkedList<_>>());
    check(&vec![bytes()].into_iter().collect::<BTreeSet<_>>());
    check(&vec![(1u8, bytes())].into_iter().collect::<BTreeMap<_, _>>());
    check(&vec![bytes()].into_iter().collect::<HashSet<_>>());
    check(&vec![(1u8, bytes())].into_iter().collect::<HashMap<_, _>>());
    check(&Some(Box::new(7u8)));
    check(&[Box::new(7u8), Box::new(8)]);
    check(&(Box::new(7u8),));
    #[cfg(feature = "rc")]
    {
        check(&alloc::rc::Rc::new(bytes()));
        check(&alloc::sync::Arc::<[u8]>::from(bytes()));
    }
}

/// At the limit, a collection fails before it charges for its elements.
#[cfg(feature = "std")]
#[test]
fn test_recursion_limit_comes_before_the_charge() {
    let input = to_vec(&vec![7u64]).unwrap();
    let mut budget = Budget::from_bytes(size_of::<Vec<u64>>());
    let result =
        borsh::de::with_recursion_limit(0, || from_slice_bounded::<Vec<u64>>(&input, &mut budget));
    assert_eq!(result.unwrap_err().to_string(), "Recursion limit exceeded");
}

/// Only for types that decode without allocating. Tags 0, 1, 2 and 255 reach every branch.
#[track_caller]
fn assert_tag_parity<T>()
where
    T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    for tag in [0, 1, 2, 0xff] {
        for payload in [None, Some(0), Some(1), Some(2)] {
            let input: Vec<u8> = core::iter::once(tag).chain(payload).collect();
            let context = format!("{} from {:?}", type_name::<T>(), input);
            assert_same_outcome(
                &from_slice::<T>(&input),
                &from_slice_bounded::<T>(&input, &mut Budget::from_bytes(size_of::<T>())),
                &context,
            );
            assert_same_outcome(
                &from_reader::<_, T>(&mut &input[..]),
                &from_reader_bounded::<_, T>(
                    &mut &input[..],
                    &mut Budget::from_bytes(size_of::<T>()),
                ),
                &context,
            );
        }
    }
}

#[test]
fn test_tag_branches() {
    assert_tag_parity::<bool>();
    assert_tag_parity::<Option<u8>>();
    assert_tag_parity::<core::result::Result<u8, u8>>();
    assert_tag_parity::<core::result::Result<bool, Option<u8>>>();

    // Spot checks, so that the parity above cannot hold vacuously.
    fn decode<T: BorshDeserializeBounded>(input: &[u8]) -> T {
        from_slice_bounded::<T>(input, &mut Budget::from_bytes(size_of::<T>())).unwrap()
    }
    assert!(!decode::<bool>(&[0]));
    assert!(decode::<bool>(&[1]));
    assert_eq!(decode::<Option<u8>>(&[0]), None);
    assert_eq!(decode::<Option<u8>>(&[1, 9]), Some(9));
    assert_eq!(decode::<core::result::Result<u8, u8>>(&[0, 9]), Err(9));
    assert_eq!(decode::<core::result::Result<u8, u8>>(&[1, 9]), Ok(9));
}

/// Checks read-buffer parity and returns the plain result and requested buffer lengths.
#[track_caller]
fn traced<T>(input: &[u8], max_read: usize) -> (Result<T>, Vec<usize>)
where
    T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let context = format!(
        "{} from {} bytes, at most {} per read",
        type_name::<T>(),
        input.len(),
        max_read
    );
    let (result, plain, bounded) =
        decode_both::<T, _>(&TraceReader::new(input, max_read), &context);
    assert_eq!(plain.trace, bounded.trace, "{}", context);
    assert_eq!(plain.input.len(), bounded.input.len(), "{}", context);
    (result, plain.trace)
}

#[test]
fn test_byte_vector_read_windows() {
    let schedules = [
        (MIB - 1, vec![MIB - 1]),
        (MIB, vec![MIB]),
        (MIB + 1, vec![MIB, 1]),
        (2 * MIB + 1, vec![MIB, MIB, 1]),
    ];
    for (len, windows) in schedules {
        let value: Vec<u8> = (0..len).map(|i| i as u8).collect();
        let input = to_vec(&value).unwrap();
        let (result, trace) = traced::<Vec<u8>>(&input, usize::MAX);
        // `assert!` rather than `assert_eq!`, so a failure does not print megabytes.
        assert!(result.unwrap() == value, "{} bytes", len);
        // The length prefix, the windows, then the check for trailing bytes.
        let expected = [&[4][..], &windows[..], &[1][..]].concat();
        assert_eq!(trace, expected, "{} bytes", len);

        let (result, _) = traced::<Vec<u8>>(&input, 300_000);
        assert!(result.unwrap() == value, "{} bytes", len);
    }
}

#[test]
fn test_byte_array_read_sequence() {
    let (result, trace) = traced::<[u8; 100]>(&[7; 100], usize::MAX);
    assert_eq!(result.unwrap(), [7; 100]);
    assert_eq!(trace, vec![100, 1]);
}

#[test]
fn test_short_reads() {
    let bytes: Vec<u8> = (0..10_000).map(|i| i as u8).collect();
    let numbers: Vec<u64> = (0..100).map(|i| i * 0x0101_0101_0101).collect();
    let bytes_input = to_vec(&bytes).unwrap();
    let numbers_input = to_vec(&numbers).unwrap();
    for max_read in [1, 3] {
        let (result, _) = traced::<Vec<u8>>(&bytes_input, max_read);
        assert_eq!(result.unwrap(), bytes, "at most {} per read", max_read);
        let (result, _) = traced::<Vec<u64>>(&numbers_input, max_read);
        assert_eq!(result.unwrap(), numbers, "at most {} per read", max_read);

        for len in [bytes_input.len() - 1, bytes_input.len() / 2, 3] {
            let (result, _) = traced::<Vec<u8>>(&bytes_input[..len], max_read);
            assert_eq!(
                error_of(result).1,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
                "{} bytes",
                len
            );
        }
        for len in [numbers_input.len() - 1, numbers_input.len() / 2] {
            let (result, _) = traced::<Vec<u64>>(&numbers_input[..len], max_read);
            assert_eq!(
                error_of(result).1,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
                "{} bytes",
                len
            );
        }
    }

    // With 3 bytes per read, `read_exact` takes two calls for the length prefix and three
    // for each `u64`; the byte loop asks for everything that is left each time.
    let (_, trace) = traced::<Vec<u64>>(&numbers_input, 3);
    assert_eq!(trace[..8], [4, 1, 8, 5, 2, 8, 5, 2]);
    assert_eq!(trace.len(), 2 + 100 * 3 + 1);
    let (_, trace) = traced::<Vec<u8>>(&bytes_input, 3);
    assert_eq!(trace[..5], [4, 1, 10_000, 9_997, 9_994]);
    assert_eq!(trace.len(), 2 + 3_334 + 1);
}

#[track_caller]
fn failing<T>(input: &[u8], at: usize, kind: ErrorKind) -> Result<T>
where
    T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let context = format!(
        "{} failing with {:?} after {} bytes",
        type_name::<T>(),
        kind,
        at
    );
    let reader = FailAfter {
        input,
        until: Some(at),
        kind,
    };
    let (result, plain, bounded) = decode_both::<T, _>(&reader, &context);
    assert_eq!(plain.input.len(), bounded.input.len(), "{}", context);
    result
}

#[track_caller]
fn assert_interruptions<T>(value: &T)
where
    T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let input = to_vec(value).unwrap();
    for at in 0..=input.len() + 1 {
        let result = failing::<T>(&input, at, ErrorKind::Interrupted);
        assert_eq!(&result.unwrap(), value, "at {}", at);
    }
}

/// `read_exact`, the `u8` byte loop and the end-of-input check all retry `Interrupted`.
#[test]
fn test_interrupted_reads() {
    // A length prefix of 4 bytes, then the byte loop.
    assert_interruptions(&vec![1u8, 2, 3]);
    // Only `read_exact`.
    assert_interruptions(&[1u8, 2, 3]);

    // Between the first and the second window.
    let input = to_vec(&vec![7u8; MIB + 1]).unwrap();
    let result = failing::<Vec<u8>>(&input, 4 + MIB, ErrorKind::Interrupted);
    assert!(result.unwrap() == vec![7; MIB + 1]);
}

/// Any other reader error is returned as is, also one after the value.
#[track_caller]
fn assert_reader_failures<T>(value: &T)
where
    T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let input = to_vec(value).unwrap();
    for at in 0..=input.len() + 1 {
        let result = failing::<T>(&input, at, ErrorKind::Other);
        let context = format!("{:?} failing after {} bytes", value, at);
        if at <= input.len() {
            assert_eq!(
                error_of(result),
                (ErrorKind::Other, "custom failure".to_string()),
                "{}",
                context
            );
        } else {
            assert_eq!(&result.unwrap(), value, "{}", context);
        }
    }
}

#[test]
fn test_failing_reader() {
    // Length prefixes, the byte loop, and an empty string that skips it.
    assert_reader_failures(&vec![
        String::from("ab"),
        String::new(),
        String::from("cde"),
    ]);
}

#[test]
fn test_reader_failing_with_unexpected_eof() {
    let bytes = vec![1u8, 2, 3];
    let input = to_vec(&bytes).unwrap();
    for at in 0..=input.len() + 1 {
        let result = failing::<Vec<u8>>(&input, at, ErrorKind::UnexpectedEof);
        // The length prefix turns it into invalid data; the byte loop and the end check don't.
        if at < 4 {
            assert_eq!(
                error_of(result),
                (
                    ErrorKind::InvalidData,
                    ERROR_UNEXPECTED_LENGTH_OF_INPUT.to_string()
                ),
                "at {}",
                at
            );
        } else if at <= input.len() {
            assert_eq!(
                error_of(result),
                (ErrorKind::UnexpectedEof, "custom failure".to_string()),
                "at {}",
                at
            );
        } else {
            assert_eq!(result.unwrap(), bytes, "at {}", at);
        }
    }
}
