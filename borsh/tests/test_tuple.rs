use borsh::{from_slice, BorshSerialize};

#[test]
fn test_unary_tuple() {
    let expected = (true,);
    let buf = expected.try_to_vec().unwrap();
    let actual = from_slice::<(bool,)>(&buf).expect("failed to deserialize");
    assert_eq!(actual, expected);
}
