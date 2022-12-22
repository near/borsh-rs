use borsh::{BorshDeserialize, BorshSerialize};

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
    let bytes = s.try_to_vec().unwrap();
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
    let mut bytes = s.try_to_vec().unwrap();
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
    let mut bytes = s.try_to_vec().unwrap();
    bytes.pop().unwrap();
    let mut reader = CustomReader {
        data: bytes,
        read_index: 0,
    };
    assert_eq!(
        <Serializable as BorshDeserialize>::try_from_reader(&mut reader)
            .unwrap_err()
            .to_string(),
        ERROR_UNEXPECTED_LENGTH_OF_INPUT
    );
}

struct CustomReader {
    data: Vec<u8>,
    read_index: usize,
}

impl borsh::maybestd::io::Read for CustomReader {
    fn read(&mut self, buf: &mut [u8]) -> borsh::maybestd::io::Result<usize> {
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
    let bytes = s.try_to_vec().unwrap();
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

impl borsh::maybestd::io::Read for CustomReaderThatDoesntFillSlices {
    fn read(&mut self, buf: &mut [u8]) -> borsh::maybestd::io::Result<usize> {
        let len = buf.len().min(self.data.len() - self.read_index);
        let len = if len <= 1 { len } else { len / 2 };
        buf[0..len].copy_from_slice(&self.data[self.read_index..self.read_index + len]);
        self.read_index += len;
        Ok(len)
    }
}

#[test]
fn test_custom_reader_that_fails_preserves_error_information() {
    let mut reader = CustomReaderThatFails;
    let err = <Serializable as BorshDeserialize>::try_from_reader(&mut reader).unwrap_err();
    assert_eq!(err.to_string(), "I don't like to run");
    assert_eq!(
        err.kind(),
        borsh::maybestd::io::ErrorKind::ConnectionAborted
    );
}

struct CustomReaderThatFails;

impl borsh::maybestd::io::Read for CustomReaderThatFails {
    fn read(&mut self, _buf: &mut [u8]) -> borsh::maybestd::io::Result<usize> {
        Err(borsh::maybestd::io::Error::new(
            borsh::maybestd::io::ErrorKind::ConnectionAborted,
            "I don't like to run",
        ))
    }
}
