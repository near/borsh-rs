use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
enum A {
    B,
    C {
        #[borsh_exclude_from_where]
        d: Vec<A>,
    },
}

#[test]
fn test_recursive_enum() {
    let a = A::C { d: vec![A::B] };
    let encoded = a.try_to_vec().unwrap();
    let decoded = A::try_from_slice(&encoded).unwrap();

    assert_eq!(A::C { d: vec![A::B] }, decoded);
}
