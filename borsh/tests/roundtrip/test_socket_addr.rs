use alloc::{vec, vec::Vec};
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

const LINK_LOCAL: Ipv6Addr = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1);

#[test]
fn test_socket_addr_v4_roundtrip() {
    let original = SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), 8080);
    let encoded = borsh::to_vec(&original).expect("Serialization failed");
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);
    let decoded = borsh::from_slice::<SocketAddrV4>(&encoded).expect("Deserialization failed");
    assert_eq!(original, decoded);
}

#[test]
fn test_socket_addr_v6_roundtrip() {
    let original = SocketAddrV6::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1), 8080, 0, 0);
    let encoded = borsh::to_vec(&original).expect("Serialization failed");
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);
    assert_eq!(encoded.len(), 18);
    let decoded = borsh::from_slice::<SocketAddrV6>(&encoded).expect("Deserialization failed");
    assert_eq!(original, decoded);
}

/// Pins the current (lossy) encoding: `flowinfo` and `scope_id` are not serialized and
/// deserialize as `0`. Changing this is a wire-format break.
#[test]
fn test_socket_addr_v6_drops_flowinfo_and_scope_id() {
    let original = SocketAddrV6::new(LINK_LOCAL, 8080, 0x000a_bcde, 2);
    let encoded = borsh::to_vec(&original).expect("Serialization failed");
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);

    let stripped = SocketAddrV6::new(LINK_LOCAL, 8080, 0, 0);
    assert_eq!(encoded, borsh::to_vec(&stripped).unwrap());

    let decoded = borsh::from_slice::<SocketAddrV6>(&encoded).expect("Deserialization failed");
    assert_eq!(decoded, stripped);
    assert_eq!(decoded.flowinfo(), 0);
    assert_eq!(decoded.scope_id(), 0);
    assert_ne!(decoded, original);
}

#[test]
fn test_socket_addr_enum_roundtrip() {
    let original = vec![
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), 8080)),
        SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
            8080,
            0,
            0,
        )),
    ];
    let encoded = borsh::to_vec(&original).expect("Serialization failed");
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);
    let decoded = borsh::from_slice::<Vec<SocketAddr>>(&encoded).expect("Deserialization failed");
    assert_eq!(original, decoded);
}

/// The `SocketAddr::V6` variant inherits the lossy `SocketAddrV6` encoding.
#[test]
fn test_socket_addr_enum_v6_drops_flowinfo_and_scope_id() {
    let original = SocketAddr::V6(SocketAddrV6::new(LINK_LOCAL, 8080, 0x000a_bcde, 2));
    let encoded = borsh::to_vec(&original).expect("Serialization failed");
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);

    let stripped = SocketAddr::V6(SocketAddrV6::new(LINK_LOCAL, 8080, 0, 0));
    assert_eq!(encoded, borsh::to_vec(&stripped).unwrap());

    let decoded = borsh::from_slice::<SocketAddr>(&encoded).expect("Deserialization failed");
    assert_eq!(decoded, stripped);
    assert_ne!(decoded, original);
}
