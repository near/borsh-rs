#![cfg_attr(not(feature = "std"), no_std)]

use borsh::{from_slice, to_vec};
use core::{matches, ops::Deref};
extern crate alloc;
use alloc::string::ToString;

#[cfg(feature = "std")]
use std::borrow::Cow;

#[cfg(not(feature = "std"))]
use alloc::{borrow::Cow, vec};

#[test]
fn test_cow_str() {
    let input: Cow<'_, str> = Cow::Borrowed("static input");

    let encoded = to_vec(&input).unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);

    let out: Cow<'_, str> = from_slice(&encoded).unwrap();

    assert!(matches!(out, Cow::Owned(..)));

    assert_eq!(input, out);
    assert_eq!(out, "static input");
}

#[test]
fn test_cow_byte_slice() {
    let arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let input: Cow<'_, [u8]> = Cow::Borrowed(&arr);

    let encoded = to_vec(&input).unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);

    let out: Cow<'_, [u8]> = from_slice(&encoded).unwrap();

    assert!(matches!(out, Cow::Owned(..)));

    assert_eq!(input, out);
    assert_eq!(out, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
}

#[test]
fn test_cow_slice_of_cow_str() {
    let arr = [
        Cow::Borrowed("first static input"),
        Cow::Owned("second static input".to_string()),
    ];
    let input: Cow<'_, [Cow<'_, str>]> = Cow::Borrowed(&arr);

    let encoded = to_vec(&input).unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);

    let out: Cow<'_, [Cow<'_, str>]> = from_slice(&encoded).unwrap();

    assert!(matches!(out, Cow::Owned(..)));

    for element in out.deref() {
        assert!(matches!(element, Cow::Owned(..)));
    }

    assert_eq!(input, out);
    assert_eq!(
        out,
        vec![
            Cow::Borrowed("first static input"),
            Cow::Borrowed("second static input"),
        ]
    );
}

#[macro_use]
mod common_macro;

#[cfg(feature = "unstable__schema")]
mod schema {

    use super::common_macro::schema_imports::*;
    #[cfg(feature = "std")]
    use std::borrow::Cow;

    #[cfg(not(feature = "std"))]
    use alloc::borrow::Cow;

    #[test]
    fn test_cow_str() {
        assert_eq!("String", <Cow<'_, str> as BorshSchema>::declaration());

        let mut actual_defs = schema_map!();
        <Cow<'_, str> as BorshSchema>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(
            schema_map! {
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );
    }

    #[test]
    fn test_cow_byte_slice() {
        assert_eq!("Vec<u8>", <Cow<'_, [u8]> as BorshSchema>::declaration());

        let mut actual_defs = schema_map!();
        <Cow<'_, [u8]> as BorshSchema>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(
            schema_map! {
                "Vec<u8>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string(),
                },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );
    }

    #[test]
    fn test_cow_slice_of_cow_str() {
        assert_eq!(
            "Vec<String>",
            <Cow<'_, [Cow<'_, str>]> as BorshSchema>::declaration()
        );

        let mut actual_defs = schema_map!();
        <Cow<'_, [Cow<'_, str>]> as BorshSchema>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(
            schema_map! {
                "Vec<String>" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "String".to_string(),
                },
                "String" => Definition::Sequence {
                    length_width: Definition::DEFAULT_LENGTH_WIDTH,
                    length_range: Definition::DEFAULT_LENGTH_RANGE,
                    elements: "u8".to_string()
                },
                "u8" => Definition::Primitive(1)
            },
            actual_defs
        );
    }
}
