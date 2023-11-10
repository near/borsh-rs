#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use borsh::{from_slice, to_vec};

/// Verifies serialisation and deserialisation of the given string.
///
/// Returns serialised representation of the string.
fn check_string(value: &str) -> alloc::vec::Vec<u8> {
    // Encoding is the same as Vec<u8> with UTF-8 encoded string.
    let buf = to_vec(value.as_bytes()).unwrap();
    assert_eq!(buf, to_vec(value).unwrap());
    assert_eq!(buf, to_vec(&value.to_string()).unwrap());
    // Check round trip.
    assert_eq!(value, from_slice::<String>(&buf).unwrap());
    buf
}

/// Verifies serialisation and deserialisation of an ASCII string `value`.
#[cfg(feature = "ascii")]
fn check_ascii(value: &str, buf: alloc::vec::Vec<u8>) {
    // Caller promises value is ASCII.
    let ascii_str = ascii::AsciiStr::from_ascii(&value).unwrap();
    // AsciiStr and AsciiString serialise the same way String does.
    assert_eq!(buf, to_vec(ascii_str).unwrap());
    assert_eq!(buf, to_vec(&ascii::AsciiString::from(ascii_str)).unwrap());
    // Check round trip.
    let got = from_slice::<ascii::AsciiString>(&buf).unwrap();
    assert_eq!(ascii_str, got);
}

/// Verifies that deserialisation of a non-ASCII string serialised in `buf`
/// fails.
#[cfg(feature = "ascii")]
fn check_non_ascii(_value: &str, buf: alloc::vec::Vec<u8>) {
    from_slice::<ascii::AsciiString>(&buf).unwrap_err();
}

macro_rules! test_string {
    ($test_name: ident, $str: expr, $assert_ascii:ident, $snap:expr) => {
        #[test]
        fn $test_name() {
            let value = String::from($str);
            let buf = check_string(&value);
            #[cfg(feature = "std")]
            if $snap {
                insta::assert_debug_snapshot!(buf);
            }
            #[cfg(feature = "ascii")]
            $assert_ascii(&value, buf)
        }
    };
}

test_string!(test_empty_string, "", check_ascii, true);
test_string!(test_a, "a", check_ascii, true);
test_string!(test_hello_world, "hello world", check_ascii, true);
test_string!(test_x_1024, "x".repeat(1024), check_ascii, true);
test_string!(test_x_4096, "x".repeat(4096), check_ascii, false);
test_string!(test_x_65535, "x".repeat(65535), check_ascii, false);
test_string!(test_hello_10, "hello world!".repeat(30), check_ascii, true);
test_string!(
    test_hello_1000,
    "hello world!".repeat(1000),
    check_ascii,
    false
);
test_string!(test_non_ascii, "💩", check_non_ascii, true);

#[cfg(feature = "ascii")]
#[test]
fn test_ascii_char() {
    use ascii::AsciiChar;

    let buf = to_vec(&AsciiChar::Dot).unwrap();
    assert_eq!(".".as_bytes(), buf);
    assert_eq!(AsciiChar::Dot, from_slice::<AsciiChar>(&buf).unwrap());

    from_slice::<AsciiChar>(&[b'\x80']).unwrap_err();
}
