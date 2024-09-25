
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

macro_rules! test_ip_roundtrip {
    ($test_name:ident, $original:expr) => {
        #[test]
        fn $test_name() {
            let original = $original;
            let encoded = borsh::to_vec(&original).expect("Serialization failed");
            let decoded = borsh::from_slice::<IpAddr>(&encoded).expect("Deserialization failed");
            assert_eq!(original, decoded);
        }
    };
}

test_ip_roundtrip!(test_ipv4_addr_roundtrip, IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)));
test_ip_roundtrip!(test_ipv6_addr_roundtrip, IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)));

macro_rules! test_ipaddr_vec_roundtrip {
    ($test_name:ident, $original:expr) => {
        #[test]
        fn $test_name() {
            let original = $original;
            let encoded = borsh::to_vec(&original).expect("Serialization failed");
            let decoded = borsh::from_slice::<Vec<IpAddr>>(&encoded).expect("Deserialization failed");
            assert_eq!(original, decoded);
        }
    };
}

test_ipaddr_vec_roundtrip!(test_ip_addr_vec_roundtrip, vec![
    IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)),
    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
    IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)),
]);
