use borsh::{from_reader, to_vec, BorshDeserialize, BorshSerialize};

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

const ERROR_NOT_ALL_BYTES_READ: &str = "Not all bytes read";
const ERROR_UNEXPECTED_LENGTH_OF_INPUT: &str = "Unexpected length of input";

#[derive(BorshSerialize, BorshDeserialize, Debug)]
struct Serializable {
    item1: i32,
    item2: String,
    item3: f64,
}

#[test]
fn test_custom_reader() {
    let s = Serializable {
        item1: 100,
        item2: "foo".into(),
        item3: 1.2345,
    };
    let bytes = to_vec(&s).unwrap();
    let mut reader = CustomReader {
        data: bytes,
        read_index: 0,
    };
    let de: Serializable = BorshDeserialize::deserialize_reader(&mut reader).unwrap();
    assert_eq!(de.item1, s.item1);
    assert_eq!(de.item2, s.item2);
    assert_eq!(de.item3, s.item3);
}

#[test]
fn test_custom_reader_with_insufficient_data() {
    let s = Serializable {
        item1: 100,
        item2: "foo".into(),
        item3: 1.2345,
    };
    let mut bytes = to_vec(&s).unwrap();
    bytes.pop().unwrap();
    let mut reader = CustomReader {
        data: bytes,
        read_index: 0,
    };
    assert_eq!(
        <Serializable as BorshDeserialize>::deserialize_reader(&mut reader)
            .unwrap_err()
            .to_string(),
        ERROR_UNEXPECTED_LENGTH_OF_INPUT
    );
}

#[test]
fn test_custom_reader_with_too_much_data() {
    let s = Serializable {
        item1: 100,
        item2: "foo".into(),
        item3: 1.2345,
    };
    let mut bytes = to_vec(&s).unwrap();
    bytes.push(1);
    let mut reader = CustomReader {
        data: bytes,
        read_index: 0,
    };
    assert_eq!(
        from_reader::<CustomReader, Serializable>(&mut reader)
            .unwrap_err()
            .to_string(),
        ERROR_NOT_ALL_BYTES_READ
    );
}

struct CustomReader {
    data: Vec<u8>,
    read_index: usize,
}

impl borsh::io::Read for CustomReader {
    fn read(&mut self, buf: &mut [u8]) -> borsh::io::Result<usize> {
        let len = buf.len().min(self.data.len() - self.read_index);
        buf[0..len].copy_from_slice(&self.data[self.read_index..self.read_index + len]);
        self.read_index += len;
        Ok(len)
    }
}

#[test]
fn test_custom_reader_that_doesnt_fill_slices() {
    let s = Serializable {
        item1: 100,
        item2: "foo".into(),
        item3: 1.2345,
    };
    let bytes = to_vec(&s).unwrap();
    let mut reader = CustomReaderThatDoesntFillSlices {
        data: bytes,
        read_index: 0,
    };
    let de: Serializable = BorshDeserialize::deserialize_reader(&mut reader).unwrap();
    assert_eq!(de.item1, s.item1);
    assert_eq!(de.item2, s.item2);
    assert_eq!(de.item3, s.item3);
}

struct CustomReaderThatDoesntFillSlices {
    data: Vec<u8>,
    read_index: usize,
}

impl borsh::io::Read for CustomReaderThatDoesntFillSlices {
    fn read(&mut self, buf: &mut [u8]) -> borsh::io::Result<usize> {
        let len = buf.len().min(self.data.len() - self.read_index);
        let len = if len <= 1 { len } else { len / 2 };
        buf[0..len].copy_from_slice(&self.data[self.read_index..self.read_index + len]);
        self.read_index += len;
        Ok(len)
    }
}

#[test]
fn test_custom_reader_that_is_interrupted() {
    let s = Serializable {
        item1: 100,
        item2: "foo".into(),
        item3: 1.2345,
    };
    let bytes = to_vec(&s).unwrap();
    // One byte per read, so interruptions also land inside the string's payload.
    let mut reader = CustomReaderThatIsInterrupted {
        data: bytes,
        read_index: 0,
        interrupted: false,
        max_read: 1,
    };
    let de: Serializable = from_reader(&mut reader).unwrap();
    assert_eq!(de.item1, s.item1);
    assert_eq!(de.item2, s.item2);
    assert_eq!(de.item3, s.item3);
}

#[test]
fn test_custom_reader_that_is_interrupted_large_byte_vec() {
    // Over 1 MiB, so decoding grows the buffer between interrupted partial reads.
    let v: Vec<u8> = (0..(1usize << 20) + 3).map(|i| i as u8).collect();
    let mut reader = CustomReaderThatIsInterrupted {
        data: to_vec(&v).unwrap(),
        read_index: 0,
        interrupted: false,
        max_read: 4093,
    };
    let de: Vec<u8> = from_reader(&mut reader).unwrap();
    assert_eq!(de, v);
}

/// Fails every other `read` with `ErrorKind::Interrupted`, which callers should retry, and
/// returns at most `max_read` bytes from the others.
struct CustomReaderThatIsInterrupted {
    data: Vec<u8>,
    read_index: usize,
    interrupted: bool,
    max_read: usize,
}

impl borsh::io::Read for CustomReaderThatIsInterrupted {
    fn read(&mut self, buf: &mut [u8]) -> borsh::io::Result<usize> {
        self.interrupted = !self.interrupted;
        if self.interrupted {
            return Err(borsh::io::ErrorKind::Interrupted.into());
        }
        let len = buf
            .len()
            .min(self.data.len() - self.read_index)
            .min(self.max_read);
        buf[0..len].copy_from_slice(&self.data[self.read_index..self.read_index + len]);
        self.read_index += len;
        Ok(len)
    }
}

#[test]
fn test_custom_reader_that_fails_preserves_error_information() {
    let mut reader = CustomReaderThatFails;

    let err = from_reader::<CustomReaderThatFails, Serializable>(&mut reader).unwrap_err();
    assert_eq!(err.to_string(), "I don't like to run");
    assert_eq!(err.kind(), borsh::io::ErrorKind::ConnectionAborted);
}

struct CustomReaderThatFails;

impl borsh::io::Read for CustomReaderThatFails {
    fn read(&mut self, _buf: &mut [u8]) -> borsh::io::Result<usize> {
        Err(borsh::io::Error::new(
            borsh::io::ErrorKind::ConnectionAborted,
            "I don't like to run",
        ))
    }
}

/// Both entry points must return the error the reader gives after the value.
fn assert_fails_after_value(kind: borsh::io::ErrorKind) {
    let mut reader = CustomReaderThatFailsAfterData {
        data: &[1, 0, 0, 0],
        kind,
    };
    let err = from_reader::<_, u32>(&mut reader).unwrap_err();
    assert_eq!(err.kind(), kind);

    let mut reader = CustomReaderThatFailsAfterData {
        data: &[1, 0, 0, 0],
        kind,
    };
    let err = u32::try_from_reader(&mut reader).unwrap_err();
    assert_eq!(err.kind(), kind);
}

#[test]
fn test_custom_reader_that_fails_after_value() {
    assert_fails_after_value(borsh::io::ErrorKind::ConnectionReset);
}

#[test]
fn test_custom_reader_that_hits_eof_error_after_value() {
    // Truncated input that ends in an error rather than `Ok(0)` was accepted as `Ok`.
    assert_fails_after_value(borsh::io::ErrorKind::UnexpectedEof);
}

/// Yields `data`, then fails with `kind`, like a decompressor on truncated input.
struct CustomReaderThatFailsAfterData {
    data: &'static [u8],
    kind: borsh::io::ErrorKind,
}

impl borsh::io::Read for CustomReaderThatFailsAfterData {
    fn read(&mut self, buf: &mut [u8]) -> borsh::io::Result<usize> {
        if self.data.is_empty() {
            return Err(self.kind.into());
        }
        borsh::io::Read::read(&mut self.data, buf)
    }
}

#[test]
fn test_custom_reader_interrupted_after_value() {
    let mut reader = CustomReaderInterruptedAfterData {
        data: &[1, 0, 0, 0],
        interrupted: false,
        tail: &[],
    };
    assert_eq!(from_reader::<_, u32>(&mut reader).unwrap(), 1);

    let mut reader = CustomReaderInterruptedAfterData {
        data: &[1, 0, 0, 0],
        interrupted: false,
        tail: &[0],
    };
    let err = from_reader::<_, u32>(&mut reader).unwrap_err();
    assert_eq!(err.kind(), borsh::io::ErrorKind::InvalidData);
}

/// Yields `data`, fails once with `Interrupted`, then yields `tail`.
struct CustomReaderInterruptedAfterData {
    data: &'static [u8],
    interrupted: bool,
    tail: &'static [u8],
}

impl borsh::io::Read for CustomReaderInterruptedAfterData {
    fn read(&mut self, buf: &mut [u8]) -> borsh::io::Result<usize> {
        if !self.data.is_empty() {
            return borsh::io::Read::read(&mut self.data, buf);
        }
        if !self.interrupted {
            self.interrupted = true;
            return Err(borsh::io::ErrorKind::Interrupted.into());
        }
        borsh::io::Read::read(&mut self.tail, buf)
    }
}
