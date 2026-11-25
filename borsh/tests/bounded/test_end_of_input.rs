use alloc::{string::String, string::ToString, vec::Vec};

use borsh::de::Budget;
use borsh::io::{Error, ErrorKind, Read, Result};
use borsh::{from_reader, from_reader_bounded, BorshDeserialize, BorshDeserializeBounded};

/// Reads `input`, then returns `end` once instead of the end of input.
struct EndsWith<'a> {
    input: &'a [u8],
    end: Option<ErrorKind>,
}

impl Read for EndsWith<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.input.is_empty() {
            if let Some(kind) = self.end.take() {
                return Err(Error::new(kind, "end"));
            }
        }
        self.input.read(buf)
    }
}

type Outcome<T> = core::result::Result<T, (ErrorKind, String)>;

/// Plain `from_reader`, then `from_reader_bounded` and `try_from_reader_bounded`.
fn decode<T>(input: &[u8], end: ErrorKind) -> [Outcome<T>; 3]
where
    T: BorshDeserialize + BorshDeserializeBounded,
{
    let reader = || EndsWith {
        input,
        end: Some(end),
    };
    let plain = from_reader::<_, T>(&mut reader());
    let bounded = from_reader_bounded::<_, T>(&mut reader(), &mut Budget::from_bytes(1024));
    let method = T::try_from_reader_bounded(&mut reader(), &mut Budget::from_bytes(1024));
    [plain, bounded, method].map(|r| r.map_err(|e| (e.kind(), e.to_string())))
}

#[test]
fn test_end_of_input_check_matches_plain_decoding() {
    let input = [2, 0, 0, 0, 7, 8];
    // `Interrupted` is retried, and the input then ends.
    let [plain, bounded, method] = decode::<Vec<u8>>(&input, ErrorKind::Interrupted);
    assert_eq!(plain, Ok(Vec::from([7, 8])));
    assert_eq!([&bounded, &method], [&plain, &plain]);
    // Any other error is returned as is, `UnexpectedEof` from the reader too.
    for kind in [ErrorKind::ConnectionReset, ErrorKind::UnexpectedEof] {
        let [plain, bounded, method] = decode::<Vec<u8>>(&input, kind);
        assert_eq!(plain, Err((kind, "end".to_string())));
        assert_eq!([&bounded, &method], [&plain, &plain]);
    }
}
