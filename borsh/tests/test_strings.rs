#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
use borsh::{from_slice, to_vec};

macro_rules! test_string {
    ($test_name: ident, $str: expr, $snap: expr) => {
        mod $test_name {
            use super::*;

            #[test]
            fn test_string() {
                let value = String::from($str);

                // Encoding is the same as Vec<u8> with UTF-8 encoded string.
                let buf = to_vec(value.as_bytes()).unwrap();
                assert_eq!(buf, to_vec(value.as_str()).unwrap());
                assert_eq!(buf, to_vec(&value).unwrap());

                #[cfg(feature = "std")]
                if $snap {
                    insta::assert_debug_snapshot!(buf);
                }

                assert_eq!(value, from_slice::<String>(&buf).unwrap());
            }

            #[cfg(feature = "ascii")]
            #[test]
            fn test_ascii() {
                let value = String::from($str);

                let buf = to_vec(&value).unwrap();
                if let Ok(ascii_str) = ascii::AsciiStr::from_ascii(&value) {
                    assert_eq!(buf, to_vec(ascii_str).unwrap());
                    assert_eq!(buf, to_vec(&ascii::AsciiString::from(ascii_str)).unwrap());
                    let got = from_slice::<ascii::AsciiString>(&buf).unwrap();
                    assert_eq!(value, got.as_str());
                } else {
                    from_slice::<ascii::AsciiString>(&buf).unwrap_err();
                }
            }
        }
    };
}

test_string!(empty_string, "", true);
test_string!(a, "a", true);
test_string!(hello_world, "hello world", true);
test_string!(x_1024, "x".repeat(1024), true);
test_string!(x_4096, "x".repeat(4096), false);
test_string!(x_65535, "x".repeat(65535), false);
test_string!(hello_10, "hello world!".repeat(30), true);
test_string!(hello_1000, "hello world!".repeat(1000), false);
test_string!(non_ascii, "💩", true);

#[test]
fn test_non_utf8() {
    let data: [u8; 4] = [0xbf, 0xf3, 0xb3, 0x77];
    let buf = to_vec(&data[..]).unwrap();
    from_slice::<String>(&buf).unwrap_err();
    #[cfg(feature = "ascii")]
    from_slice::<ascii::AsciiString>(&buf).unwrap_err();
}

#[cfg(feature = "ascii")]
#[test]
fn test_ascii_char() {
    use ascii::AsciiChar;
    use ascii::AsciiChar::Dot;

    let buf = to_vec(&Dot).unwrap();
    assert_eq!(".".as_bytes(), buf);
    assert_eq!(Dot, from_slice::<AsciiChar>(&buf).unwrap());

    from_slice::<AsciiChar>(&[b'\x80']).unwrap_err();
}
