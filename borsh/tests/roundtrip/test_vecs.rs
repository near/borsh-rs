use borsh::{from_slice, to_vec};

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

macro_rules! test_vec {
    ($v: expr, $t: ty, $snap: expr) => {
        let buf = to_vec(&$v).unwrap();
        #[cfg(feature = "std")]
        if $snap {
            insta::assert_debug_snapshot!(buf);
        }
        let actual_v: Vec<$t> = from_slice(&buf).expect("failed to deserialize");
        assert_eq!(actual_v, $v);
    };
}

macro_rules! test_vecs {
    ($test_name: ident, $el: expr, $t: ty) => {
        #[test]
        fn $test_name() {
            test_vec!(Vec::<$t>::new(), $t, true);
            test_vec!(vec![$el], $t, true);
            test_vec!(vec![$el; 10], $t, true);
            test_vec!(vec![$el; 100], $t, true);
            test_vec!(vec![$el; 1000], $t, false); // one assumes that the concept has been proved
            test_vec!(vec![$el; 10000], $t, false);
        }
    };
}

test_vecs!(test_vec_u8, 100u8, u8);
test_vecs!(test_vec_i8, 100i8, i8);
test_vecs!(test_vec_u32, 1000000000u32, u32);
test_vecs!(test_vec_f32, 1000000000.0f32, f32);
test_vecs!(test_vec_string, "a".to_string(), String);
test_vecs!(test_vec_vec_u8, vec![100u8; 10], Vec<u8>);
test_vecs!(test_vec_vec_u32, vec![100u32; 10], Vec<u32>);

#[test]
fn test_decoded_byte_vector_capacity_equals_length() {
    for len in [1, (1 << 20) - 1, 1 << 20, (1 << 20) + 1, 3 << 20] {
        let decoded = from_slice::<Vec<u8>>(&to_vec(&vec![7u8; len]).unwrap()).unwrap();
        assert_eq!(decoded.len(), len);
        assert_eq!(decoded.capacity(), len);
    }
}
