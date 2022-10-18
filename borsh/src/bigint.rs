use crate::de::ERROR_UNEXPECTED_LENGTH_OF_INPUT;
use crate::maybestd::{
    format,
    io::{Error, ErrorKind, Result, Write},
    vec,
};
use crate::{BorshDeserialize, BorshSerialize};
use unsigned_varint as uvarint;

const ERROR_NON_CANONICAL_VALUE: &str = "Padded zero bytes found";

impl BorshSerialize for num_bigint_dep::BigUint {
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let data = self.to_bytes_le();
        match data.iter().rev().position(|&v| v != 0) {
            Some(index) => {
                // Remove padding bytes to serialize canonically.
                let (bytes, _): (&[u8], _) = data.split_at(data.len() - index);
                let mut buffer = uvarint::encode::u32_buffer();
                let encoded_len = uvarint::encode::u32(bytes.len() as u32, &mut buffer);
                writer.write_all(&encoded_len)?;
                writer.write_all(&bytes)
            }
            None => {
                // Writing 0 varint length for 0 value integer.
                writer.write_all(&[0])
            }
        }
    }
}

impl BorshDeserialize for num_bigint_dep::BigUint {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let (val, rem) = uvarint::decode::u32(&buf).map_err(|e| {
            Error::new(
                ErrorKind::InvalidInput,
                format!("varint decoding error: {}", e),
            )
        })?;
        let (digits, new_buf) = rem.split_at(val as usize);
        *buf = new_buf;
        if digits.last() == Some(&0) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_NON_CANONICAL_VALUE,
            ));
        }

        Ok(num_bigint_dep::BigUint::from_bytes_le(digits))
    }
}

impl BorshSerialize for num_bigint_dep::BigInt {
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let sign = self.sign();
        if matches!(sign, num_bigint_dep::Sign::NoSign) {
            sign.serialize(writer)
        } else {
            sign.serialize(writer)?;
            self.magnitude().serialize(writer)
        }
    }
}

impl BorshDeserialize for num_bigint_dep::BigInt {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let sign = num_bigint_dep::Sign::deserialize(buf)?;
        let value = if matches!(sign, num_bigint_dep::Sign::NoSign) {
            num_bigint_dep::BigUint::new(vec![])
        } else {
            let uint = num_bigint_dep::BigUint::deserialize(buf)?;
            if uint == num_bigint_dep::BigUint::default() {
                // If the abs value is 0 when sign is positive or negative, reject for being
                // not canonical.
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    ERROR_NON_CANONICAL_VALUE,
                ));
            }
            uint
        };
        Ok(num_bigint_dep::BigInt::from_biguint(sign, value))
    }
}

impl BorshSerialize for num_bigint_dep::Sign {
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            num_bigint_dep::Sign::Minus => 0u8.serialize(writer),
            num_bigint_dep::Sign::NoSign => 1u8.serialize(writer),
            num_bigint_dep::Sign::Plus => 2u8.serialize(writer),
        }
    }
}

impl BorshDeserialize for num_bigint_dep::Sign {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        if buf.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_UNEXPECTED_LENGTH_OF_INPUT,
            ));
        }
        let sign_flag = buf[0];
        *buf = &buf[1..];
        match sign_flag {
            0 => Ok(num_bigint_dep::Sign::Minus),
            1 => Ok(num_bigint_dep::Sign::NoSign),
            2 => Ok(num_bigint_dep::Sign::Plus),
            _ => {
                let msg = format!(
                    "Invalid Result representation: {}. The first byte must be 0, 1 or 2",
                    sign_flag
                );
                Err(Error::new(ErrorKind::InvalidInput, msg))
            }
        }
    }
}
