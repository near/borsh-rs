//! Hand-written bounded impls: nested decodes and custom allocations share one budget.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::mem::size_of;

use borsh::de::{EnumExt, EnumExtBounded};
use borsh::io::{Error, ErrorKind, Read, Result};
use borsh::{from_slice_bounded, to_vec, BorshDeserialize, BorshDeserializeBounded, Budget};

use super::common::{
    assert_charge, assert_fails_alike, assert_truncations_fail_alike, error_of, CountingReader,
};
use crate::common_macro::{assert_bounded_parity, limit_exceeded, PARITY_LIMIT};

/// Legacy encodings are untagged, so the low byte of their `numbers` length must not be 0xFF.
const VERSION_2: u8 = 0xFF;

#[derive(Debug, PartialEq)]
enum Versioned {
    Legacy {
        numbers: Vec<u64>,
        name: String,
    },
    V2 {
        name: String,
        numbers: Vec<u64>,
        labels: Vec<String>,
    },
}

#[cfg(feature = "std")]
fn chain<'a, R: Read>(first: &'a [u8], second: &'a mut R) -> std::io::Chain<&'a [u8], &'a mut R> {
    first.chain(second)
}

/// The no_std `Read` has no `chain`.
#[cfg(not(feature = "std"))]
fn chain<'a, R: Read>(first: &'a [u8], second: &'a mut R) -> Chain<'a, R> {
    Chain { first, second }
}

#[cfg(not(feature = "std"))]
struct Chain<'a, R> {
    first: &'a [u8],
    second: &'a mut R,
}

#[cfg(not(feature = "std"))]
impl<R: Read> Read for Chain<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.first.is_empty() {
            self.second.read(buf)
        } else {
            self.first.read(buf)
        }
    }
}

impl BorshDeserialize for Versioned {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let tag = u8::deserialize_reader(reader)?;
        if tag == VERSION_2 {
            Ok(Versioned::V2 {
                name: String::deserialize_reader(reader)?,
                numbers: Vec::deserialize_reader(reader)?,
                labels: Vec::deserialize_reader(reader)?,
            })
        } else {
            // The tag is the first byte of a legacy encoding: read it again.
            let prefix = [tag];
            let mut chained = chain(&prefix, reader);
            Ok(Versioned::Legacy {
                numbers: Vec::deserialize_reader(&mut chained)?,
                name: String::deserialize_reader(&mut chained)?,
            })
        }
    }
}

impl BorshDeserializeBounded for Versioned {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let tag = u8::deserialize_bounded_alloc(reader, budget)?;
        if tag == VERSION_2 {
            Ok(Versioned::V2 {
                name: String::deserialize_bounded_alloc(reader, budget)?,
                numbers: Vec::deserialize_bounded_alloc(reader, budget)?,
                labels: Vec::deserialize_bounded_alloc(reader, budget)?,
            })
        } else {
            let prefix = [tag];
            let mut chained = chain(&prefix, reader);
            Ok(Versioned::Legacy {
                numbers: Vec::deserialize_bounded_alloc(&mut chained, budget)?,
                name: String::deserialize_bounded_alloc(&mut chained, budget)?,
            })
        }
    }
}

fn legacy(numbers: &[u64], name: &str) -> (Vec<u8>, Versioned) {
    let input = to_vec(&(numbers, name)).unwrap();
    let value = Versioned::Legacy {
        numbers: numbers.to_vec(),
        name: name.to_string(),
    };
    (input, value)
}

fn v2(name: &str, numbers: &[u64], labels: &[&str]) -> (Vec<u8>, Versioned) {
    let mut input = vec![VERSION_2];
    input.extend(to_vec(&(name, numbers, labels)).unwrap());
    let value = Versioned::V2 {
        name: name.to_string(),
        numbers: numbers.to_vec(),
        labels: labels.iter().map(|label| label.to_string()).collect(),
    };
    (input, value)
}

#[test]
fn test_versioned_exact_charge() {
    let (input, value) = legacy(&[1, 2, 3], "abc");
    assert_eq!(input[0], 3);
    assert_charge(&input, &value, 24 + 3, 3, 2);

    // Last legacy length before the reserved tag byte.
    let (input, value) = legacy(&[7; 254], "");
    assert_eq!(input[0], 254);
    assert_charge(&input, &value, 254 * 8, 254 * 8, 254 * 8 - 1);

    let (input, value) = v2("ab", &[5], &["x", "yz"]);
    let charge = 2 + 8 + 2 * size_of::<String>() + 1 + 2;
    assert_charge(&input, &value, charge, 2, 1);
}

#[test]
fn test_versioned_false_lengths() {
    // 0x0100_0010 numbers claimed, one present.
    let mut input = vec![0x10, 0, 0, 0x01];
    input.extend(to_vec(&7u64).unwrap());
    let mut reader = CountingReader::new(&input);
    let err = Versioned::deserialize_bounded_alloc(&mut reader, &mut Budget::from_bytes(1000))
        .unwrap_err();
    assert_eq!(limit_exceeded(&err), Some((1000, 0x0100_0010 * 8, 1000)));
    assert_eq!(reader.consumed, 4);
}

#[test]
fn test_versioned_parity_with_plain() {
    let inputs = [legacy(&[1, 2, 3], "abc").0, v2("ab", &[5], &["x", "yz"]).0];
    for input in &inputs {
        assert_bounded_parity::<Versioned>(input);
        assert_truncations_fail_alike::<Versioned>(input);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum MyEnum {
    Zero,
    One(u8),
    Many(Vec<u8>),
}

impl BorshDeserialize for MyEnum {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let tag = u8::deserialize_reader(reader)?;
        Self::deserialize_variant(reader, tag)
    }
}

impl EnumExt for MyEnum {
    fn deserialize_variant<R: Read>(reader: &mut R, tag: u8) -> Result<Self> {
        match tag {
            0 => Ok(MyEnum::Zero),
            1 => Ok(MyEnum::One(u8::deserialize_reader(reader)?)),
            2 => Ok(MyEnum::Many(Vec::deserialize_reader(reader)?)),
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unexpected variant tag: {:?}", tag),
            )),
        }
    }
}

impl BorshDeserializeBounded for MyEnum {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let tag = u8::deserialize_bounded_alloc(reader, budget)?;
        Self::deserialize_variant_bounded_alloc(reader, tag, budget)
    }
}

impl EnumExtBounded for MyEnum {
    fn deserialize_variant_bounded_alloc<R: Read>(
        reader: &mut R,
        tag: u8,
        budget: &mut Budget,
    ) -> Result<Self> {
        match tag {
            0 => Ok(MyEnum::Zero),
            1 => Ok(MyEnum::One(u8::deserialize_bounded_alloc(reader, budget)?)),
            2 => Ok(MyEnum::Many(Vec::deserialize_bounded_alloc(
                reader, budget,
            )?)),
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unexpected variant tag: {:?}", tag),
            )),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct OneOrZero(MyEnum);

impl BorshDeserialize for OneOrZero {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let tag = u8::deserialize_reader(reader)?;
        if tag == 2 {
            Err(Error::new(
                ErrorKind::InvalidData,
                "MyEnum::Many not allowed here",
            ))
        } else {
            MyEnum::deserialize_variant(reader, tag).map(Self)
        }
    }
}

impl BorshDeserializeBounded for OneOrZero {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let tag = u8::deserialize_bounded_alloc(reader, budget)?;
        if tag == 2 {
            Err(Error::new(
                ErrorKind::InvalidData,
                "MyEnum::Many not allowed here",
            ))
        } else {
            MyEnum::deserialize_variant_bounded_alloc(reader, tag, budget).map(Self)
        }
    }
}

fn many_input(bytes: &[u8]) -> Vec<u8> {
    let mut input = vec![2];
    input.extend(to_vec(bytes).unwrap());
    input
}

#[test]
fn test_enum_ext_bounded_values() {
    assert_eq!(
        from_slice_bounded::<OneOrZero>(
            b"\x01\x07",
            &mut Budget::from_bytes(size_of::<OneOrZero>())
        )
        .unwrap(),
        OneOrZero(MyEnum::One(7))
    );

    // `OneOrZero` rejects `Many` before its length is read and charged.
    let input = [2, 0xff, 0xff, 0xff, 0xff];
    assert_eq!(
        error_of(from_slice_bounded::<OneOrZero>(
            &input,
            &mut Budget::from_bytes(size_of::<OneOrZero>())
        )),
        (
            ErrorKind::InvalidData,
            "MyEnum::Many not allowed here".to_string()
        )
    );
    let limit = size_of::<MyEnum>();
    let err = from_slice_bounded::<MyEnum>(&input, &mut Budget::from_bytes(limit)).unwrap_err();
    assert_eq!(limit_exceeded(&err), Some((limit, u32::MAX as usize, 0)));
}

#[test]
fn test_enum_ext_bounded_tags_match_plain() {
    for tag in [0, 1, 2, 255u8] {
        let mut input = vec![tag];
        input.extend(to_vec(&[9u8][..]).unwrap());
        assert_bounded_parity::<MyEnum>(&input);
        assert_bounded_parity::<OneOrZero>(&input);
    }
}

#[test]
fn test_enum_ext_bounded_many_charges_its_length() {
    let bytes = vec![7u8; 1000];
    assert_charge(&many_input(&bytes), &MyEnum::Many(bytes), 1000, 1000, 999);
}

/// Elements use plain decoding without charges, so their decoders must not allocate.
#[derive(Debug, PartialEq)]
struct Counted<T>(Vec<T>);

impl<T: BorshDeserialize> BorshDeserialize for Counted<T> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let count = u32::deserialize_reader(reader)?;
        // Without a budget, a count from the input cannot size the vector.
        let mut elements = Vec::new();
        for _ in 0..count {
            push_element(&mut elements, reader)?;
        }
        Ok(Counted(elements))
    }
}

impl<T: BorshDeserialize> BorshDeserializeBounded for Counted<T> {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let count = u32::deserialize_bounded_alloc(reader, budget)?;
        budget.charge_array::<T>(count as usize)?;
        let mut elements = Vec::with_capacity(count as usize);
        for _ in 0..count {
            push_element(&mut elements, reader)?;
        }
        Ok(Counted(elements))
    }
}

/// Out of line so that no caller's frame holds a `T`: a `Huge` would overflow the stack.
#[inline(never)]
fn push_element<T: BorshDeserialize, R: Read>(elements: &mut Vec<T>, reader: &mut R) -> Result<()> {
    elements.push(T::deserialize_reader(reader)?);
    Ok(())
}

/// 4 GiB, so that `2^31` of them exceed `isize::MAX` bytes, the largest allocation size.
#[cfg(target_pointer_width = "64")]
struct Huge(#[allow(dead_code)] [u8; 1 << 32]);

#[cfg(target_pointer_width = "64")]
impl BorshDeserialize for Huge {
    fn deserialize_reader<R: Read>(_reader: &mut R) -> Result<Self> {
        unreachable!("no test decodes a `Huge`")
    }
}

#[test]
fn test_charge_array_before_a_custom_allocation() {
    let input = to_vec(&[1u64, 2, 3][..]).unwrap();
    assert_charge(&input, &Counted(vec![1u64, 2, 3]), 24, 24, 23);
    assert_bounded_parity::<Counted<u64>>(&input);
    assert_truncations_fail_alike::<Counted<u64>>(&input);

    let input = to_vec(&Vec::<u64>::new()).unwrap();
    assert_eq!(
        from_slice_bounded::<Counted<u64>>(
            &input,
            &mut Budget::from_bytes(size_of::<Counted<u64>>())
        )
        .unwrap(),
        Counted(Vec::new())
    );
}

#[test]
fn test_false_count_fails_before_reading_any_element() {
    let mut input = to_vec(&1000u32).unwrap();
    input.extend(to_vec(&[1u64, 2]).unwrap());

    let mut reader = CountingReader::new(&input);
    let mut budget = Budget::from_bytes(100);
    let err = Counted::<u64>::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap_err();
    assert_eq!(limit_exceeded(&err), Some((100, 8000, 100)));
    assert_eq!(reader.consumed, 4);
    assert_eq!(budget.remaining_bytes(), 100);
}

/// Invalid allocation sizes must fail with `requested == usize::MAX` before allocation can panic.
#[cfg(target_pointer_width = "64")]
#[test]
fn test_charge_array_overflow() {
    use super::cases::MIB;

    // (2^31 - 1) · 2^32 bytes is just under `isize::MAX`, a valid size.
    let count = (1u32 << 31) - 1;
    let limit = MIB + size_of::<Counted<Huge>>();
    let err =
        from_slice_bounded::<Counted<Huge>>(&count.to_le_bytes(), &mut Budget::from_bytes(limit))
            .err()
            .unwrap();
    assert_eq!(
        limit_exceeded(&err),
        Some((limit, ((1 << 31) - 1) << 32, MIB))
    );

    // 2^31 · 2^32 bytes is over `isize::MAX`.
    let count = 1u32 << 31;
    let err = Counted::<Huge>::deserialize_bounded_alloc(
        &mut &count.to_le_bytes()[..],
        &mut Budget::from_bytes(usize::MAX),
    )
    .err()
    .unwrap();
    assert_eq!(
        limit_exceeded(&err),
        Some((usize::MAX, usize::MAX, usize::MAX))
    );
}

const ERROR_TRAILING_BYTES_IN_BLOB: &str = "Trailing bytes in blob";

/// A length-prefixed blob containing exactly one encoded value.
#[derive(Debug, PartialEq)]
struct Embedded<T>(T);

impl<T: BorshDeserialize> BorshDeserialize for Embedded<T> {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let blob = Vec::<u8>::deserialize_reader(reader)?;
        let mut rest = &blob[..];
        let value = T::deserialize_reader(&mut rest)?;
        if !rest.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                ERROR_TRAILING_BYTES_IN_BLOB,
            ));
        }
        Ok(Embedded(value))
    }
}

impl<T: BorshDeserializeBounded> BorshDeserializeBounded for Embedded<T> {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let blob = Vec::<u8>::deserialize_bounded_alloc(reader, budget)?;
        let mut rest = &blob[..];
        let value = T::deserialize_bounded_alloc(&mut rest, budget)?;
        if !rest.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                ERROR_TRAILING_BYTES_IN_BLOB,
            ));
        }
        Ok(Embedded(value))
    }
}

fn embedded_input(encoded_value: &[u8]) -> Vec<u8> {
    to_vec(encoded_value).unwrap()
}

#[test]
fn test_sub_decode_shares_the_outer_budget() {
    // A blob of 28 bytes, then 3 numbers. One byte less fails in the inner decode.
    let input = embedded_input(&to_vec(&vec![1u64, 2, 3]).unwrap());
    let value = Embedded(vec![1u64, 2, 3]);
    assert_charge(&input, &value, 28 + 24, 24, 24 - 1);
}

#[test]
fn test_sub_decode_parity_with_plain() {
    let input = embedded_input(&to_vec(&vec![1u64, 2, 3]).unwrap());
    assert_bounded_parity::<Embedded<Vec<u64>>>(&input);
    assert_truncations_fail_alike::<Embedded<Vec<u64>>>(&input);

    let mut blob = to_vec(&5u32).unwrap();
    blob.push(0);
    let input = embedded_input(&blob);
    assert_fails_alike::<Embedded<u32>>(&input);
    let err = from_slice_bounded::<Embedded<u32>>(&input, &mut Budget::from_bytes(PARITY_LIMIT))
        .unwrap_err();
    assert_eq!(err.to_string(), ERROR_TRAILING_BYTES_IN_BLOB);

    // A false length inside the blob, within the limit.
    assert_fails_alike::<Embedded<Vec<u8>>>(&embedded_input(&to_vec(&1000u32).unwrap()));
}

#[derive(Debug, PartialEq)]
struct Uncharged(Vec<u8>);

impl BorshDeserialize for Uncharged {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Vec::<u8>::deserialize_reader(reader).map(Uncharged)
    }
}

impl BorshDeserializeBounded for Uncharged {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, _budget: &mut Budget) -> Result<Self> {
        <Vec<u8> as BorshDeserialize>::deserialize_reader(reader).map(Uncharged)
    }
}

#[test]
fn test_plain_decode_inside_a_bounded_impl_is_not_charged() {
    let input = to_vec(&vec![7u8; 1000]).unwrap();
    let mut budget = Budget::from_bytes(10);
    let value = Uncharged::deserialize_bounded_alloc(&mut &input[..], &mut budget).unwrap();
    assert_eq!(value, Uncharged(vec![7; 1000]));
    assert_eq!(budget.remaining_bytes(), 10);
    assert_bounded_parity::<Uncharged>(&input);

    // Plain decoding ignores the limit and fails on truncated input instead.
    let input = to_vec(&u32::MAX).unwrap();
    let err =
        from_slice_bounded::<Uncharged>(&input, &mut Budget::from_bytes(size_of::<Uncharged>()))
            .unwrap_err();
    assert_eq!(limit_exceeded(&err), None);
    assert_fails_alike::<Uncharged>(&input);
}

#[test]
fn test_one_budget_for_several_values() {
    let (legacy_input, legacy_value) = legacy(&[1, 2], "ab");
    let mut input = legacy_input;
    input.extend(to_vec(&[1u64, 2, 3][..]).unwrap());
    input.extend(many_input(&[9; 10]));
    input.extend(embedded_input(&to_vec("hello").unwrap()));
    // Charges: 16 + 2, then 24, then 10, then 9 + 5.
    let total = 18 + 24 + 10 + 14;

    let mut reader = &input[..];
    let mut budget = Budget::from_bytes(100);
    let value = Versioned::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    assert_eq!(value, legacy_value);
    assert_eq!(budget.remaining_bytes(), 100 - 18);
    let value = Counted::<u64>::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    assert_eq!(value, Counted(vec![1, 2, 3]));
    assert_eq!(budget.remaining_bytes(), 100 - 18 - 24);
    let value = MyEnum::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    assert_eq!(value, MyEnum::Many(vec![9; 10]));
    assert_eq!(budget.remaining_bytes(), 100 - 18 - 24 - 10);
    let value = Embedded::<String>::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    assert_eq!(value, Embedded("hello".to_string()));
    assert_eq!(budget.remaining_bytes(), 100 - total);
    assert!(reader.is_empty());

    // One byte short: the last value fails on its text; its blob charge is kept.
    let limit = total - 1;
    let mut reader = &input[..];
    let mut budget = Budget::from_bytes(limit);
    Versioned::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    Counted::<u64>::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    MyEnum::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
    assert_eq!(budget.remaining_bytes(), 13);
    let err = Embedded::<String>::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap_err();
    assert_eq!(limit_exceeded(&err), Some((limit, 5, 4)));
    assert_eq!(budget.remaining_bytes(), 4);
}
