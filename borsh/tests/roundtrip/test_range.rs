use crate::common_macro::assert_bounded_parity;

#[test]
fn test_ranges() {
    let want = (1..2, 3..=4, 5.., ..6, ..=7, ..);

    let encoded = borsh::to_vec(&want).unwrap();
    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded);

    let got = borsh::from_slice(&encoded).unwrap();
    assert_bounded_parity::<(
        core::ops::Range<i32>,
        core::ops::RangeInclusive<i32>,
        core::ops::RangeFrom<i32>,
        core::ops::RangeTo<i32>,
        core::ops::RangeToInclusive<i32>,
        core::ops::RangeFull,
    )>(&encoded);
    assert_eq!(want, got);
}
