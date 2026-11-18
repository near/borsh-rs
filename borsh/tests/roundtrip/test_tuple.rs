use borsh::{from_slice, to_vec};

use crate::common_macro::assert_bounded_parity;

#[test]
fn test_unary_tuple() {
    let expected = (true,);
    let buf = to_vec(&expected).unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(buf);
    let actual = from_slice::<(bool,)>(&buf).expect("failed to deserialize");
    assert_bounded_parity::<(bool,)>(&buf);
    assert_eq!(actual, expected);
}
