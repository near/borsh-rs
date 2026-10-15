use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque};
use std::io::{self, ErrorKind, Read};
use std::mem::size_of;
use std::panic::catch_unwind;
use std::thread;

use borsh::error::{LimitExceeded, ERROR_ZST_FORBIDDEN};
use borsh::{
    from_reader, from_reader_with_limit, from_slice, from_slice_with_limit, to_vec,
    BorshDeserialize,
};

use super::cases::{cases, decoded_capacity, LYING_LENGTH_PREFIX, MIB};

const ERROR_MEMORY_LIMIT_EXCEEDED: &str = "Memory limit exceeded";
const ERROR_UNEXPECTED_LENGTH_OF_INPUT: &str = "Unexpected length of input";
const ERROR_NOT_ALL_BYTES_READ: &str = "Not all bytes read";

fn assert_error<T>(result: io::Result<T>, kind: ErrorKind, message: &str) -> io::Error {
    let err = result.err().expect("decoding should fail");
    assert_eq!(err.kind(), kind);
    assert_eq!(err.to_string(), message);
    err
}

fn assert_limit_exceeded<T>(result: io::Result<T>) {
    let err = assert_error(result, ErrorKind::OutOfMemory, ERROR_MEMORY_LIMIT_EXCEEDED);
    assert!(err.get_ref().is_some_and(|e| e.is::<LimitExceeded>()));
}

fn u64_vec(len: usize) -> Vec<u8> {
    to_vec(&vec![7u64; len]).unwrap()
}

fn byte_vec(len: usize) -> Vec<u8> {
    to_vec(&vec![7u8; len]).unwrap()
}

#[test]
fn test_exact_limit() {
    for case in cases() {
        (case.decode)(&case.input, Some(case.limit))
            .unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        assert_limit_exceeded((case.decode)(&case.input, Some(case.limit - 1)));
    }
}

#[test]
fn test_zero_limit_decodes_values_that_allocate_nothing() {
    assert_eq!(
        from_slice_with_limit::<u64>(&to_vec(&7u64).unwrap(), 0).unwrap(),
        7
    );
    assert!(from_slice_with_limit::<Vec<u64>>(&u64_vec(0), 0)
        .unwrap()
        .is_empty());
    assert!(from_slice_with_limit::<Vec<u8>>(&byte_vec(0), 0)
        .unwrap()
        .is_empty());
    assert!(from_slice_with_limit::<String>(&byte_vec(0), 0)
        .unwrap()
        .is_empty());
    assert_limit_exceeded(from_slice_with_limit::<Vec<u64>>(&u64_vec(1), 0));
    assert_limit_exceeded(from_slice_with_limit::<Vec<u8>>(&byte_vec(1), 0));
}

#[test]
fn test_zero_sized_elements_keep_their_error() {
    assert_error(
        from_slice_with_limit::<Vec<()>>(&[1, 0, 0, 0], 0),
        ErrorKind::InvalidData,
        ERROR_ZST_FORBIDDEN,
    );
}

#[test]
fn test_lying_length_prefix_is_rejected_before_allocating() {
    assert_limit_exceeded(from_slice_with_limit::<Vec<u8>>(
        &LYING_LENGTH_PREFIX,
        64 * 1024,
    ));
    assert_limit_exceeded(from_slice_with_limit::<Vec<u64>>(
        &LYING_LENGTH_PREFIX,
        1024,
    ));
}

/// The first buffer each type decodes through exceeds 1024 bytes. An uncharged
/// type would fail on the short input instead.
fn assert_charged<T: BorshDeserialize>() {
    assert_limit_exceeded(from_slice_with_limit::<T>(&LYING_LENGTH_PREFIX, 1024));
}

#[test]
fn test_types_decoded_through_vectors_are_charged() {
    assert_charged::<String>();
    assert_charged::<Box<[u64]>>();
    assert_charged::<Box<str>>();
    assert_charged::<VecDeque<u64>>();
    assert_charged::<Cow<'static, [u64]>>();
    assert_charged::<Cow<'static, str>>();
    assert_charged::<LinkedList<u64>>();
    assert_charged::<HashMap<u64, u64>>();
    assert_charged::<HashSet<u64>>();
    assert_charged::<BTreeMap<u64, u64>>();
    assert_charged::<BTreeSet<u64>>();
    #[cfg(feature = "bytes")]
    assert_charged::<bytes::Bytes>();
    #[cfg(feature = "ascii")]
    assert_charged::<ascii::AsciiString>();
    #[cfg(feature = "indexmap")]
    assert_charged::<indexmap::IndexMap<u64, u64>>();
    #[cfg(feature = "indexmap")]
    assert_charged::<indexmap::IndexSet<u64>>();
}

#[test]
fn test_growth_beyond_limit_fails() {
    let input = u64_vec(513);
    // Element 513 forces the initial 4096-byte buffer to double.
    assert_limit_exceeded(from_slice_with_limit::<Vec<u64>>(&input, 4096));
    from_slice_with_limit::<Vec<u64>>(&input, 8192).unwrap();
}

#[test]
fn test_truncated_input_at_growth_reports_missing_bytes() {
    let mut input = u64_vec(513);
    input.truncate(input.len() - size_of::<u64>());
    assert_error(
        from_slice_with_limit::<Vec<u64>>(&input, 4096),
        ErrorKind::InvalidData,
        ERROR_UNEXPECTED_LENGTH_OF_INPUT,
    );
}

#[test]
fn test_plain_decoding_has_no_limit() {
    for case in cases() {
        (case.decode)(&case.input, None).unwrap_or_else(|err| panic!("{}: {}", case.name, err));
    }
    from_slice::<Vec<u64>>(&u64_vec(513)).unwrap();
    assert_error(
        from_slice::<Vec<u8>>(&LYING_LENGTH_PREFIX),
        ErrorKind::InvalidData,
        ERROR_UNEXPECTED_LENGTH_OF_INPUT,
    );
    assert_error(
        from_slice::<Vec<u64>>(&LYING_LENGTH_PREFIX),
        ErrorKind::InvalidData,
        ERROR_UNEXPECTED_LENGTH_OF_INPUT,
    );
}

#[test]
fn test_decoded_vector_capacity_matches_charge() {
    let decoded = from_slice::<Vec<u64>>(&u64_vec(1000)).unwrap();
    assert_eq!(decoded.capacity(), 1024);
    assert_eq!(decoded_capacity::<u64>(1000), 1024);

    let decoded =
        from_slice::<Vec<Vec<u8>>>(&to_vec(&vec![Vec::<u8>::new(); 1000]).unwrap()).unwrap();
    assert_eq!(decoded.capacity(), decoded_capacity::<Vec<u8>>(1000));
    #[cfg(target_pointer_width = "64")]
    assert_eq!(decoded.capacity(), 1360);
}

/// Retains inner errors so the outer decode can continue after a rejection.
struct Nested<const INNER: usize> {
    inner: io::Result<Vec<u64>>,
}

impl<const INNER: usize> BorshDeserialize for Nested<INNER> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let blob = Vec::<u8>::deserialize_reader(reader)?;
        Ok(Self {
            inner: from_slice_with_limit(&blob, INNER),
        })
    }
}

/// Returns the input and the blob's length. The outer limit pays for the blob,
/// the inner decode and the trailing `u64`s.
fn nested_input(inner_len: usize, trailing_len: usize) -> (Vec<u8>, usize) {
    let blob = u64_vec(inner_len);
    let blob_len = blob.len();
    (to_vec(&(blob, vec![7u64; trailing_len])).unwrap(), blob_len)
}

#[test]
fn test_nested_unbounded_limit_is_capped_by_outer_remainder() {
    let (input, blob_len) = nested_input(1000, 0);
    type Outer = (Nested<{ usize::MAX }>, Vec<u64>);

    let (nested, _) = from_slice_with_limit::<Outer>(&input, blob_len + 8192).unwrap();
    assert_eq!(nested.inner.unwrap().len(), 1000);

    let (nested, _) = from_slice_with_limit::<Outer>(&input, blob_len + 8191).unwrap();
    assert_limit_exceeded(nested.inner);
}

#[test]
fn test_nested_smaller_limit_applies() {
    let (input, _) = nested_input(1000, 0);

    let (nested, _) = from_slice_with_limit::<(Nested<8191>, Vec<u64>)>(&input, MIB).unwrap();
    assert_limit_exceeded(nested.inner);

    let (nested, _) = from_slice_with_limit::<(Nested<8192>, Vec<u64>)>(&input, MIB).unwrap();
    assert_eq!(nested.inner.unwrap().len(), 1000);
}

#[test]
fn test_nested_charges_count_against_outer_limit() {
    let (input, blob_len) = nested_input(1000, 1);
    type Outer = (Nested<{ usize::MAX }>, Vec<u64>);

    let (nested, trailing) = from_slice_with_limit::<Outer>(&input, blob_len + 8192 + 8).unwrap();
    assert_eq!(nested.inner.unwrap().len(), 1000);
    assert_eq!(trailing.len(), 1);

    assert_limit_exceeded(from_slice_with_limit::<Outer>(&input, blob_len + 8192 + 7));
}

#[test]
fn test_failed_nested_decode_charges_outer_only_what_it_allocated() {
    let (input, blob_len) = nested_input(1000, 100);
    // Failed inner growth leaves a 4096-byte charge; the trailing vector
    // needs another 100 * 8 bytes.
    type Outer = (Nested<{ 4096 + 100 }>, Vec<u64>);

    let (nested, trailing) = from_slice_with_limit::<Outer>(&input, blob_len + 4096 + 800).unwrap();
    assert_limit_exceeded(nested.inner);
    assert_eq!(trailing.len(), 100);

    assert_limit_exceeded(from_slice_with_limit::<Outer>(
        &input,
        blob_len + 4096 + 799,
    ));
}

#[test]
fn test_failed_decode_leaves_no_limit() {
    let input = byte_vec(2 * MIB);
    assert_limit_exceeded(from_slice_with_limit::<Vec<u8>>(&input, MIB));
    from_slice::<Vec<u8>>(&input).unwrap();
}

/// `from_reader` must consume the rest of the input, so this only works as the
/// top-level type.
struct PlainNested(Vec<u64>);

impl BorshDeserialize for PlainNested {
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        from_reader(reader).map(Self)
    }
}

#[test]
fn test_nested_plain_decode_is_charged() {
    let input = u64_vec(1000);
    let decoded = from_slice_with_limit::<PlainNested>(&input, 8192).unwrap();
    assert_eq!(decoded.0.len(), 1000);
    assert_limit_exceeded(from_slice_with_limit::<PlainNested>(&input, 8191));
}

struct PanicsAfterDecoding;

impl BorshDeserialize for PanicsAfterDecoding {
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        Vec::<u64>::deserialize_reader(reader)?;
        panic!("PanicsAfterDecoding");
    }
}

/// Catching the inner panic lets trailing data check the restored budget.
struct CatchesNestedPanic<const INNER: usize>;

impl<const INNER: usize> BorshDeserialize for CatchesNestedPanic<INNER> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let blob = Vec::<u8>::deserialize_reader(reader)?;
        let result = catch_unwind(|| from_slice_with_limit::<PanicsAfterDecoding>(&blob, INNER));
        assert!(result.is_err(), "the nested decode should panic");
        Ok(Self)
    }
}

#[test]
fn test_nested_panic_charges_outer_limit() {
    let (input, blob_len) = nested_input(1000, 1);
    // The inner limit is below the outer remainder, so the trailing decode
    // fits only if unwinding restored the outer budget.
    type Outer = (CatchesNestedPanic<8192>, Vec<u64>);

    let (_, trailing) = from_slice_with_limit::<Outer>(&input, blob_len + 8192 + 8).unwrap();
    assert_eq!(trailing.len(), 1);

    assert_limit_exceeded(from_slice_with_limit::<Outer>(&input, blob_len + 8192 + 7));
}

#[test]
fn test_panic_leaves_no_limit() {
    let result =
        catch_unwind(|| from_slice_with_limit::<PanicsAfterDecoding>(&u64_vec(1000), 8192));
    assert!(result.is_err(), "the decode should panic");

    from_slice::<Vec<u8>>(&byte_vec(2 * MIB)).unwrap();
    from_slice_with_limit::<Vec<u64>>(&u64_vec(1000), 8192).unwrap();
}

struct DecodesOnAnotherThread {
    decoded: io::Result<Vec<u8>>,
}

impl BorshDeserialize for DecodesOnAnotherThread {
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let blob = Vec::<u8>::deserialize_reader(reader)?;
        let decoded = thread::spawn(move || from_slice::<Vec<u8>>(&blob))
            .join()
            .unwrap();
        Ok(Self { decoded })
    }
}

#[test]
fn test_limit_does_not_apply_to_other_threads() {
    let blob = byte_vec(2 * MIB);
    let input = to_vec(&blob).unwrap();

    let result = from_slice_with_limit::<DecodesOnAnotherThread>(&input, blob.len()).unwrap();
    assert_eq!(result.decoded.unwrap().len(), 2 * MIB);

    assert_limit_exceeded(from_slice_with_limit::<DecodesOnAnotherThread>(
        &input,
        blob.len() - 1,
    ));
}

/// Short reads make byte vectors fill over many reads.
struct Chunked<'a>(&'a [u8]);

impl Read for Chunked<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = buf.len().min(self.0.len()).min(1000);
        buf[..len].copy_from_slice(&self.0[..len]);
        self.0 = &self.0[len..];
        Ok(len)
    }
}

#[test]
fn test_reader_exact_limit() {
    let input = u64_vec(1000);
    from_reader_with_limit::<_, Vec<u64>>(&mut input.as_slice(), 8192).unwrap();
    assert_limit_exceeded(from_reader_with_limit::<_, Vec<u64>>(
        &mut input.as_slice(),
        8191,
    ));

    let input = byte_vec(3 * MIB);
    let decoded = from_reader_with_limit::<_, Vec<u8>>(&mut Chunked(&input), 3 * MIB).unwrap();
    assert_eq!(decoded.len(), 3 * MIB);
    assert_limit_exceeded(from_reader_with_limit::<_, Vec<u8>>(
        &mut Chunked(&input),
        3 * MIB - 1,
    ));
}

#[test]
fn test_trailing_bytes_are_reported() {
    let mut input = u64_vec(3);
    input.push(0);
    assert_error(
        from_slice_with_limit::<Vec<u64>>(&input, 1024),
        ErrorKind::InvalidData,
        ERROR_NOT_ALL_BYTES_READ,
    );
    assert_error(
        from_reader_with_limit::<_, Vec<u64>>(&mut input.as_slice(), 1024),
        ErrorKind::InvalidData,
        ERROR_NOT_ALL_BYTES_READ,
    );
}
