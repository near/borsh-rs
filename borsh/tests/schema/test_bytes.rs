use crate::common_macro::schema_imports::*;
use bytes::{Bytes, BytesMut};

#[test]
fn bytes_schema() {
    let bytes_schema = BorshSchemaContainer::for_type::<Bytes>();
    let bytes_mut_schema = BorshSchemaContainer::for_type::<BytesMut>();
    insta::assert_snapshot!(format!("{:#?}", (bytes_schema, bytes_mut_schema)));
}
