use alloc::string::{String, ToString};
use core::fmt::Debug;
use core::mem::size_of;

use borsh::io::{Error, ErrorKind, Read, Result};
use borsh::{
    from_reader, from_reader_bounded, from_slice, from_slice_bounded, BorshDeserialize,
    BorshDeserializeBounded, Budget,
};

pub use crate::common_macro::assert_same_outcome;
use crate::common_macro::{limit_exceeded, OneByteReader, PARITY_LIMIT};

pub const ERROR_UNEXPECTED_LENGTH_OF_INPUT: &str = "Unexpected length of input";

fn kind_and_message(err: &Error) -> (ErrorKind, String) {
    (err.kind(), err.to_string())
}

#[track_caller]
pub fn error_of<T: Debug>(result: Result<T>) -> (ErrorKind, String) {
    kind_and_message(&result.unwrap_err())
}

#[track_caller]
pub fn assert_fails_alike<T>(input: &[u8])
where
    T: BorshDeserialize + BorshDeserializeBounded + Debug,
{
    assert_eq!(
        error_of(from_slice::<T>(input)),
        error_of(from_slice_bounded::<T>(
            input,
            &mut Budget::from_bytes(PARITY_LIMIT)
        )),
        "{:?}",
        input
    );
    assert_eq!(
        error_of(from_reader::<_, T>(&mut OneByteReader(input))),
        error_of(from_reader_bounded::<_, T>(
            &mut OneByteReader(input),
            &mut Budget::from_bytes(PARITY_LIMIT)
        )),
        "{:?}",
        input
    );
}

#[track_caller]
pub fn assert_truncations_fail_alike<T>(input: &[u8])
where
    T: BorshDeserialize + BorshDeserializeBounded + Debug,
{
    for len in 0..input.len() {
        assert_fails_alike::<T>(&input[..len]);
    }
}

#[track_caller]
pub fn charged<T: BorshDeserializeBounded>(input: &[u8]) -> usize {
    let mut budget = Budget::from_bytes(PARITY_LIMIT);
    T::deserialize_bounded_alloc(&mut &input[..], &mut budget).unwrap();
    PARITY_LIMIT - budget.remaining_bytes()
}

/// `charge` excludes `size_of::<T>()`; `(requested, remaining)` is the limit error one byte
/// below their sum.
#[track_caller]
pub fn assert_charge<T>(
    input: &[u8],
    expected: &T,
    charge: usize,
    requested: usize,
    remaining: usize,
) where
    T: BorshDeserializeBounded + PartialEq + Debug,
{
    let mut reader = input;
    let mut budget = Budget::from_bytes(charge);
    let decoded = T::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    assert_eq!(&decoded, expected);
    assert!(reader.is_empty());
    assert_eq!(budget.remaining_bytes(), 0);
    let entry = size_of::<T>() + charge;
    let decoded =
        from_reader_bounded::<_, T>(&mut OneByteReader(input), &mut Budget::from_bytes(entry))
            .unwrap();
    assert_eq!(&decoded, expected);

    let limit = entry - 1;
    let below = Some((limit, requested, remaining));
    let err = from_slice_bounded::<T>(input, &mut Budget::from_bytes(limit)).unwrap_err();
    assert_eq!(limit_exceeded(&err), below, "{}", err);
    let err =
        from_reader_bounded::<_, T>(&mut OneByteReader(input), &mut Budget::from_bytes(limit))
            .unwrap_err();
    assert_eq!(limit_exceeded(&err), below, "{}", err);
}

pub struct CountingReader<'a> {
    input: &'a [u8],
    pub consumed: usize,
}

impl<'a> CountingReader<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        CountingReader { input, consumed: 0 }
    }
}

impl Read for CountingReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let read = self.input.read(buf)?;
        self.consumed += read;
        Ok(read)
    }
}
