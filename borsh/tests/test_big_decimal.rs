#![cfg(feature = "bigdecimal")]
use bigdecimal::BigDecimal;
use borsh::{BorshDeserialize, BorshSerialize};
use std::str::FromStr;

#[test]
fn test_bigdecimal() {
    let bigdecimals = vec![
        BigDecimal::from(0),
        BigDecimal::from_str("-0.0").unwrap(),
        BigDecimal::from_str("3.14159265358979323846").unwrap(),
        BigDecimal::from(256),
        BigDecimal::from(666),
        BigDecimal::from(-42),
        BigDecimal::from_str(&"7".repeat(1024)).unwrap(),
    ];
    for bigdecimal in bigdecimals {
        let serialized = bigdecimal.try_to_vec().unwrap();
        let deserialized =
            <BigDecimal>::try_from_slice(&serialized).expect("failed to deserialize BigDecimal");

        assert_eq!(deserialized, bigdecimal);
    }
}
