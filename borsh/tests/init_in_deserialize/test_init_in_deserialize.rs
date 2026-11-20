use borsh::{from_slice, to_vec, BorshDeserialize, BorshSerialize};

use crate::common_macro::assert_bounded_parity;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
#[cfg_attr(feature = "unstable__bounded", derive(borsh::BorshDeserializeBounded))]
#[borsh(init=init)]
struct A {
    lazy: Option<u64>,
}

impl A {
    pub fn init(&mut self) {
        if let Some(v) = self.lazy.as_mut() {
            *v *= 10;
        }
    }
}
#[test]
fn test_simple_struct() {
    let a = A { lazy: Some(5) };

    let encoded_a = to_vec(&a).unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded_a);

    let decoded_a = from_slice::<A>(&encoded_a).unwrap();
    // Not `assert_bounded_parity`: `init` overflows on its mutated inputs, in both decoders.
    #[cfg(feature = "unstable__bounded")]
    {
        use crate::common_macro::PARITY_LIMIT;
        use borsh::{from_slice_bounded, Budget};

        let bounded_a = from_slice_bounded::<A>(&encoded_a, &mut Budget::from_bytes(PARITY_LIMIT));
        assert_eq!(bounded_a.unwrap(), decoded_a);
    }
    let expected_a = A { lazy: Some(50) };
    assert_eq!(expected_a, decoded_a);
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
#[cfg_attr(feature = "unstable__bounded", derive(borsh::BorshDeserializeBounded))]
#[borsh(init=initialization_method)]
enum AEnum {
    A,
    B,
    C,
}

impl AEnum {
    pub fn initialization_method(&mut self) {
        *self = AEnum::C;
    }
}

#[test]
fn test_simple_enum() {
    let a = AEnum::B;
    let encoded_a = to_vec(&a).unwrap();

    #[cfg(feature = "std")]
    insta::assert_debug_snapshot!(encoded_a);

    let decoded_a = from_slice::<AEnum>(&encoded_a).unwrap();
    assert_bounded_parity::<AEnum>(&encoded_a);
    assert_eq!(AEnum::C, decoded_a);
}
