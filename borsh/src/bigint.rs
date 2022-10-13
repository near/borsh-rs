use crate::de::ERROR_UNEXPECTED_LENGTH_OF_INPUT;
use crate::maybestd::{
    format,
    io::{Error, ErrorKind, Result, Write},
    vec,
    vec::Vec,
};
use crate::{BorshDeserialize, BorshSerialize};

#[cfg(feature = "num-bigint")]
const ERROR_NON_CANONICAL_VALUE: &str = "Padded zero bytes found";

#[cfg(feature = "bigdecimal")]
impl BorshSerialize for bigdecimal_dep::BigDecimal {
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let (bigint, exponent) = self.as_bigint_and_exponent();
        bigint.serialize(writer)?;
        exponent.serialize(writer)
    }
}

#[cfg(feature = "bigdecimal")]
impl BorshDeserialize for bigdecimal_dep::BigDecimal {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        let digits = num_bigint_dep::BigInt::deserialize(buf)?;
        let scale = i64::deserialize(buf)?;
        Ok(bigdecimal_dep::BigDecimal::new(digits, scale))
    }
}

impl BorshSerialize for num_bigint_dep::BigUint {
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let data = self.to_bytes_le();
        match data.iter().rev().position(|&v| v != 0) {
            Some(index) => {
                // Remove padding bytes to serialize canonically.
                let (bytes, _): (&[u8], _) = data.split_at(data.len() - index);
                (bytes).serialize(writer)
            }
            None => (&[] as &[u8]).serialize(writer),
        }
    }
}

impl BorshDeserialize for num_bigint_dep::BigUint {
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> Result<Self> {
        // TODO should be able to avoid this allocation with zero-copy deserialization.
        let digits = <Vec<u8>>::deserialize(buf)?;
        if digits.len() > 0 && digits.last().unwrap() == &0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                ERROR_NON_CANONICAL_VALUE,
            ));
        }

        Ok(num_bigint_dep::BigUint::from_bytes_le(&digits))
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
