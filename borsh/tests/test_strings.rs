#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
use borsh::{from_slice, BorshSerialize};

macro_rules! test_string {
    ($test_name: ident, $str: expr, $snap: expr) => {
        #[test]
        fn $test_name() {
            let s = $str.to_string();
            let buf = s.try_to_vec().unwrap();
            #[cfg(feature = "std")]
            if $snap {
                insta::assert_debug_snapshot!(buf);
            }
            let actual_s = from_slice::<String>(&buf).expect("failed to deserialize a string");
            assert_eq!(actual_s, s);
        }
    };
}

test_string!(test_empty_string, "", true);
test_string!(test_a, "a", true);
test_string!(test_hello_world, "hello world", true);
test_string!(test_x_1024, "x".repeat(1024), true);
test_string!(test_x_4096, "x".repeat(4096), false);
test_string!(test_x_65535, "x".repeat(65535), false);
test_string!(test_hello_10, "hello world!".repeat(30), true);
test_string!(test_hello_1000, "hello world!".repeat(1000), false);
test_string!(test_non_ascii, "💩", true);
