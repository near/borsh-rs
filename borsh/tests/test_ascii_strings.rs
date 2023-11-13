#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "ascii")]

use borsh::{from_slice, to_vec};

extern crate alloc;
use alloc::string::String;

/// Verifies serialisation and deserialisation of an ASCII string `value`.
fn check_ascii(value: &str) -> alloc::vec::Vec<u8> {
    // Caller promises value is ASCII.
    let ascii_str = ascii::AsciiStr::from_ascii(&value).unwrap();
    let buf = to_vec(ascii_str).unwrap();
    // AsciiStr and AsciiString serialise the same way String does.
    assert_eq!(buf, to_vec(&ascii::AsciiString::from(ascii_str)).unwrap());
    // Check round trip.
    let got = from_slice::<ascii::AsciiString>(&buf).unwrap();
    assert_eq!(ascii_str, got);
    buf
}

macro_rules! test_ascii_string {
    ($test_name: ident, $str: expr, $snap:expr) => {
        #[test]
        fn $test_name() {
            let value = String::from($str);
            let _buf = check_ascii(&value);
            #[cfg(feature = "std")]
            if $snap {
                insta::assert_debug_snapshot!(_buf);
            }
        }
    };
}

test_ascii_string!(test_empty_string, "", true);
test_ascii_string!(test_a, "a", true);
test_ascii_string!(test_hello_world, "hello world", true);
test_ascii_string!(test_x_1024, "x".repeat(1024), true);
test_ascii_string!(test_x_4096, "x".repeat(4096), false);
test_ascii_string!(test_x_65535, "x".repeat(65535), false);
test_ascii_string!(test_hello_10, "hello world!".repeat(30), true);
test_ascii_string!(test_hello_1000, "hello Achilles!".repeat(1000), false);

#[test]
fn test_ascii_char() {
    use ascii::AsciiChar;

    let buf = to_vec(&AsciiChar::Dot).unwrap();
    assert_eq!(".".as_bytes(), buf);
    assert_eq!(AsciiChar::Dot, from_slice::<AsciiChar>(&buf).unwrap());

    from_slice::<AsciiChar>(&[b'\x80']).unwrap_err();
}

mod de_errors {
    use alloc::string::ToString;
    use borsh::from_slice;

    #[test]
    fn test_non_ascii() {
        let buf = borsh::to_vec(&[0xbf, 0xf3, 0xb3, 0x77][..]).unwrap();
        assert_eq!(
            from_slice::<ascii::AsciiString>(&buf)
                .unwrap_err()
                .to_string(),
            "the byte at index 0 is not ASCII"
        );

        let buf = borsh::to_vec("żółw").unwrap();
        assert_eq!(
            from_slice::<ascii::AsciiString>(&buf)
                .unwrap_err()
                .to_string(),
            "the byte at index 0 is not ASCII"
        );

        assert_eq!(
            from_slice::<ascii::AsciiChar>(&[0xbf])
                .unwrap_err()
                .to_string(),
            "not an ASCII character"
        );
    }
}

#[cfg(feature = "unstable__schema")]
mod schema {
    use alloc::{collections::BTreeMap, string::ToString};
    use borsh::schema::{BorshSchema, Definition};
    macro_rules! map(
        () => { BTreeMap::new() };
        { $($key:expr => $value:expr),+ } => {
            {
                let mut m = BTreeMap::new();
                $(
                    m.insert($key.to_string(), $value);
                )+
                m
            }
         };
        );

    #[test]
    fn test_ascii_strings() {
        assert_eq!("AsciiString", ascii::AsciiStr::declaration());
        assert_eq!("AsciiString", ascii::AsciiString::declaration());
        assert_eq!("AsciiChar", ascii::AsciiChar::declaration());

        let want_char = map! {
            "AsciiChar" => Definition::Primitive(1)
        };
        let mut actual_defs = map!();
        ascii::AsciiChar::add_definitions_recursively(&mut actual_defs);
        assert_eq!(want_char, actual_defs);

        let want = map! {
            "AsciiString" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "AsciiChar".to_string()
            },
            "AsciiChar" => Definition::Primitive(1)
        };

        let mut actual_defs = map!();
        ascii::AsciiStr::add_definitions_recursively(&mut actual_defs);
        assert_eq!(want, actual_defs);

        let mut actual_defs = map!();
        ascii::AsciiString::add_definitions_recursively(&mut actual_defs);
        assert_eq!(want, actual_defs);
    }
}
