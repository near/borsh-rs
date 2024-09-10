use crate::common_macro::schema_imports::*;

#[cfg(feature = "std")]
use std::net::IpAddr;

#[test]
fn ip_addr_schema() {
    let actual_name = IpAddr::declaration();
    let mut actual_defs = schema_map!();
    IpAddr::add_definitions_recursively(&mut actual_defs);

    assert_eq!("IpAddr", actual_name);
    assert_eq!(
        schema_map! {
            "IpAddr" => Definition::Union {
                variants: vec![
                    "IpAddr::V4".to_string(),
                    "IpAddr::V6".to_string()
                ],
            },
            "IpAddr::V4" => Definition::Tuple {
                elements: vec!["u8".to_string(), "u8".to_string(), "u8".to_string(), "u8".to_string()],
            },
            "IpAddr::V6" => Definition::Tuple {
                elements: vec![
                    "u16".to_string(), "u16".to_string(), "u16".to_string(), "u16".to_string(),
                    "u16".to_string(), "u16".to_string(), "u16".to_string(), "u16".to_string(),
                ],
            },
            "u8" => Definition::Primitive(1),
            "u16" => Definition::Primitive(2),
        },
        actual_defs
    );
}
