use crate::common_macro::schema_imports::*;
use borsh::{try_from_slice_with_schema, try_to_vec_with_schema, BorshDeserialize, BorshSerialize};

#[test]
fn borrowed_fields_decode_as_owned() {
    #[derive(BorshSchema, BorshSerialize, BorshDeserialize, Debug, PartialEq)]
    struct Message<S, T>(S, T);

    let values = [2, u16::MAX];
    let bytes = try_to_vec_with_schema(&Message("borsh", &values[..])).unwrap();
    let decoded: Message<String, Vec<u16>> = try_from_slice_with_schema(&bytes).unwrap();
    assert_eq!(decoded, Message(String::from("borsh"), values.to_vec()));
}

#[test]
fn nested_references_share_the_inner_schema() {
    assert_eq!(schema_container_of::<&&u32>(), schema_container_of::<u32>());
}
