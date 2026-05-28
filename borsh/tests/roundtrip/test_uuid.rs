use borsh::BorshDeserialize;
use uuid::Uuid;

#[test]
fn test_uuid_roundtrip() {
    let original_uuid = Uuid::from_bytes([
        0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7,
        0xd8,
    ]);
    let serialized_uuid = borsh::to_vec(&original_uuid).unwrap();

    // Borsh encodes a `Uuid` as its raw 16 bytes, no length prefix.
    assert_eq!(serialized_uuid.as_slice(), original_uuid.as_bytes());

    let deserialized_uuid = Uuid::try_from_slice(&serialized_uuid).unwrap();
    assert_eq!(original_uuid, deserialized_uuid);
}
