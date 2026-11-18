use alloc::{string::ToString, vec::Vec};

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

fn both<T>(input: &[u8], end: ErrorKind) -> [core::result::Result<T, (ErrorKind, alloc::string::String)>; 2]
where
    T: BorshDeserialize + BorshDeserializeBounded,
{
    let plain = from_reader::<_, T>(&mut EndsWith { input, end: Some(end) });
    let bounded = from_reader_bounded::<_, T>(
        &mut EndsWith { input, end: Some(end) },
        &mut Budget::from_bytes(1024),
    );
    [plain, bounded].map(|r| r.map_err(|e| (e.kind(), e.to_string())))
}

#[test]
fn test_end_of_input_check_matches_plain_decoding() {
    let input = [2, 0, 0, 0, 7, 8];
    // `Interrupted` is retried, and the input then ends.
    let [plain, bounded] = both::<Vec<u8>>(&input, ErrorKind::Interrupted);
    assert_eq!(plain, Ok(Vec::from([7, 8])));
    assert_eq!(bounded, plain);
    // Any other error is returned as is.
    let [plain, bounded] = both::<Vec<u8>>(&input, ErrorKind::ConnectionReset);
    assert_eq!(plain, Err((ErrorKind::ConnectionReset, "end".to_string())));
    assert_eq!(bounded, plain);
}
