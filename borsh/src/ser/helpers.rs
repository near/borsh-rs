use crate::BorshSerialize;
use crate::__private::maybestd::vec::Vec;
use crate::io::{Result, Write};

pub(super) const DEFAULT_SERIALIZER_CAPACITY: usize = 1024;

/// Serialize an object into a vector of bytes.
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: BorshSerialize + ?Sized,
{
    let mut result = Vec::with_capacity(DEFAULT_SERIALIZER_CAPACITY);
    value.serialize(&mut result)?;
    Ok(result)
}

/// Serializes an object directly into a `Writer`.
pub fn to_writer<T, W: Write>(mut writer: W, value: &T) -> Result<()>
where
    T: BorshSerialize + ?Sized,
{
    value.serialize(&mut writer)
}

/// Serializes an object without allocation to compute and return its length
pub fn object_length<T>(value: &T) -> Result<usize>
where
    T: BorshSerialize + ?Sized,
{
    // copy-paste of solution provided by @matklad
    // in https://github.com/near/borsh-rs/issues/23#issuecomment-816633365
    struct LengthWriter {
        len: usize,
    }
    impl Write for LengthWriter {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> Result<usize> {
            self.len += buf.len();
            Ok(buf.len())
        }
        #[inline]
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }
    let mut w = LengthWriter { len: 0 };
    value.serialize(&mut w)?;
    Ok(w.len)
}
