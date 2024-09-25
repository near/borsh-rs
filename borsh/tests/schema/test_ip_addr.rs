use crate::common_macro::schema_imports::*;
use std::net::IpAddr;
#[test]
fn ip_addr_schema() {
    let actual_name = IpAddr::declaration();
    let mut actual_defs = insta::assert_snapshot!(format!("{:#?}", defs));
    IpAddr::add_definitions_recursively(&mut actual_defs);
    assert_eq!("IpAddr", actual_name);
    assert_eq!(
        schema_map! {
            // TODO: add correct schema assertion
            "u16" => Definition::Primitive(2)
        },
        actual_defs
    );
}
