//! Parity with plain decoding in errors and reader calls.

use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::any::type_name;
use core::cmp::Ordering;
use core::fmt::Debug;
use core::mem::size_of;
use core::ops::Range;

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;

use borsh::error::ERROR_ZST_FORBIDDEN;
use borsh::io::{Error, ErrorKind, Read, Result};
use borsh::{
    from_reader, from_reader_bounded, from_slice, from_slice_bounded, to_vec, BorshDeserialize,
    BorshDeserializeBounded, BorshSerialize, Budget,
};

use super::cases::MIB;
use super::common::{assert_same_outcome, charged, error_of, ERROR_UNEXPECTED_LENGTH_OF_INPUT};
use crate::common_macro::{is_limit_exceeded, limit_exceeded, PARITY_LIMIT};

const ERROR_NOT_ALL_BYTES_READ: &str = "Not all bytes read";

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

/// Fails with `kind` after `until` bytes; `Interrupted` fails once, as `read_exact` retries it.
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

/// Trailing-byte validation follows decoding, so a limit error takes precedence.
#[track_caller]
fn assert_trailing_bytes_rejected<T>(value: &T)
where
    T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + Debug,
{
    let encoding = to_vec(value).unwrap();
    let used = size_of::<T>() + charged::<T>(&encoding);
    let expected = (ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ.to_string());
    let input = [&encoding[..], &[0][..]].concat();
    assert_eq!(error_of(from_slice::<T>(&input)), expected, "{:?}", value);
    assert_eq!(
        error_of(from_reader::<_, T>(&mut &input[..])),
        expected,
        "{:?}",
        value
    );
    assert_eq!(
        error_of(from_slice_bounded::<T>(
            &input,
            &mut Budget::from_bytes(used)
        )),
        expected,
        "{:?}",
        value
    );
    assert_eq!(
        error_of(from_reader_bounded::<_, T>(
            &mut &input[..],
            &mut Budget::from_bytes(used)
        )),
        expected,
        "{:?}",
        value
    );
    if used > 0 {
        let err = from_slice_bounded::<T>(&input, &mut Budget::from_bytes(used - 1)).unwrap_err();
        assert!(is_limit_exceeded(&err), "{:?}: {}", value, err);
        let err = from_reader_bounded::<_, T>(&mut &input[..], &mut Budget::from_bytes(used - 1))
            .unwrap_err();
        assert!(is_limit_exceeded(&err), "{:?}: {}", value, err);
    }
}

#[test]
fn test_trailing_bytes() {
    assert_trailing_bytes_rejected(&());
    assert_trailing_bytes_rejected(&vec![1u64, 2, 3]);
    assert_trailing_bytes_rejected(&String::from("hello"));
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
    use alloc::boxed::Box;
    use alloc::collections::{BTreeSet, LinkedList, VecDeque};

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
}

#[test]
fn test_zero_sized_map_values_are_allowed() {
    let input = to_vec(&vec![(1u8, ()), (2, ())]).unwrap();
    let context = "map with zero-sized values";
    let plain = from_slice::<BTreeMap<u8, ()>>(&input);
    assert_eq!(plain.as_ref().unwrap().len(), 2);
    assert_same_outcome(
        &plain,
        &from_slice_bounded::<BTreeMap<u8, ()>>(&input, &mut Budget::from_bytes(PARITY_LIMIT)),
        context,
    );
    #[cfg(hash_collections)]
    {
        let plain = from_slice::<HashMap<u8, ()>>(&input);
        assert_eq!(plain.as_ref().unwrap().len(), 2);
        assert_same_outcome(
            &plain,
            &from_slice_bounded::<HashMap<u8, ()>>(&input, &mut Budget::from_bytes(PARITY_LIMIT)),
            context,
        );
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
fn assert_interruptions<T>(value: &T, fails_at: Range<usize>)
where
    T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let input = to_vec(value).unwrap();
    for at in 0..=input.len() + 1 {
        let result = failing::<T>(&input, at, ErrorKind::Interrupted);
        if fails_at.contains(&at) {
            assert_eq!(
                error_of(result),
                (ErrorKind::Interrupted, "custom failure".to_string()),
                "{:?} at {}",
                value,
                at
            );
        } else {
            assert_eq!(&result.unwrap(), value, "at {}", at);
        }
    }
}

/// `read_exact` retries `Interrupted`; the `u8` byte loop's `read` passes it on.
#[test]
fn test_interrupted_reads() {
    // A length prefix of 4 bytes, then the byte loop.
    assert_interruptions(&vec![1u8, 2, 3], 4..7);
    // Only `read_exact`.
    assert_interruptions(&[1u8, 2, 3], 0..0);

    // Between the first and the second window.
    let input = to_vec(&vec![7u8; MIB + 1]).unwrap();
    assert_eq!(
        error_of(failing::<Vec<u8>>(&input, 4 + MIB, ErrorKind::Interrupted)),
        (ErrorKind::Interrupted, "custom failure".to_string())
    );
}

/// After the value, any reader error but EOF is reported as trailing bytes.
#[track_caller]
fn assert_reader_failures<T>(value: &T)
where
    T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let input = to_vec(value).unwrap();
    for at in 0..=input.len() + 1 {
        let result = failing::<T>(&input, at, ErrorKind::Other);
        let context = format!("{:?} failing after {} bytes", value, at);
        match at.cmp(&input.len()) {
            Ordering::Less => assert_eq!(
                error_of(result),
                (ErrorKind::Other, "custom failure".to_string()),
                "{}",
                context
            ),
            Ordering::Equal => assert_eq!(
                error_of(result),
                (ErrorKind::InvalidData, ERROR_NOT_ALL_BYTES_READ.to_string()),
                "{}",
                context
            ),
            Ordering::Greater => assert_eq!(&result.unwrap(), value, "{}", context),
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
    for at in 0..=input.len() {
        let result = failing::<Vec<u8>>(&input, at, ErrorKind::UnexpectedEof);
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
        } else if at < input.len() {
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
