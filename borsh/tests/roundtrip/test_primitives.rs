use borsh::{from_slice, to_vec};

macro_rules! test_primitive {
    ($test_name: ident, $v: expr, $t: ty) => {
        #[test]
        fn $test_name() {
            let expected: $t = $v;

            let buf = to_vec(&expected).unwrap();
            #[cfg(feature = "std")]
            insta::assert_debug_snapshot!(buf);
            let actual = from_slice::<$t>(&buf).expect("failed to deserialize");
            assert_eq!(actual, expected);
        }
    };
}

test_primitive!(test_isize_neg, -100isize, isize);
test_primitive!(test_isize_pos, 100isize, isize);

test_primitive!(test_usize, 100usize, usize);
test_primitive!(test_usize_min, usize::MIN, usize);

// isize/usize are serialized as i64/u64, so their extreme values differ by
// pointer width. Test the values that fit in 32 bits on all platforms, and
// gate isize::MIN/MAX and usize::MAX on 64-bit targets.
test_primitive!(test_isize_min_i32, i32::MIN as isize, isize);
test_primitive!(test_isize_max_i32, i32::MAX as isize, isize);
test_primitive!(test_usize_max_u32, u32::MAX as usize, usize);

#[cfg(target_pointer_width = "64")]
test_primitive!(test_isize_min, isize::MIN, isize);
#[cfg(target_pointer_width = "64")]
test_primitive!(test_isize_max, isize::MAX, isize);
#[cfg(target_pointer_width = "64")]
test_primitive!(test_usize_max, usize::MAX, usize);
