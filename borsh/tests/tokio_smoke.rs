// Smoke tests that ensure that we don't accidentally remove top-level
// re-exports in a minor release.

use borsh::tokio::{to_vec, to_writer, AsyncBorshDeserialize};

#[tokio::test]
async fn test_to_vec() {
    let value = 42u8;
    let serialized = to_vec(&value).await.unwrap();
    let deserialized = u8::try_from_slice(&serialized).await.unwrap();
    assert_eq!(value, deserialized);
}

#[tokio::test]
async fn test_to_writer() {
    let value = 42u8;
    let mut serialized = vec![0; 1];
    to_writer(&mut serialized, &value).await.unwrap();
    let deserialized = u8::try_from_slice(&serialized).await.unwrap();
    assert_eq!(value, deserialized);
}
