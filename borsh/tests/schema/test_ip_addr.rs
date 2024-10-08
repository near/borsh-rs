use crate::common_macro::schema_imports::*;
use std::net::IpAddr;

#[test]
fn ip_addr_schema() {
    let actual_name = IpAddr::declaration();
    let mut actual_defs = insta::assert_snapshot!(format!("{:#?}", defs));
    IpAddr::add_definitions_recursively(&mut actual_defs);
    assert_eq!("IpAddr", actual_name);
}
