//! Local names must not capture paths or bindings in the generated bounded decoder.

/// Regression for <https://github.com/near/borsh-rs/issues/11>.
/// Skipped fields are excluded because a local `core` hides their generated
/// `core::default::Default` paths in both derives.
mod std_and_core {
    mod std {}
    mod core {}

    #[allow(dead_code)]
    #[derive(borsh::BorshDeserializeBounded)]
    struct Named<T> {
        x: u64,
        y: alloc::vec::Vec<T>,
    }

    #[allow(dead_code)]
    #[derive(borsh::BorshDeserialize, borsh::BorshDeserializeBounded)]
    #[borsh(use_discriminant = true)]
    #[repr(u8)]
    enum WithFields<T> {
        X { f: T } = 3,
        Y(u8),
        Z = 10,
    }
}

mod shadowed_names {
    use alloc::vec::Vec;

    #[allow(dead_code)]
    trait BorshDeserialize {}
    #[allow(dead_code)]
    trait BorshDeserializeBounded {}
    #[allow(dead_code)]
    trait EnumExt {}
    #[allow(dead_code)]
    trait EnumExtBounded {}
    #[allow(dead_code)]
    struct Budget;
    #[allow(dead_code)]
    trait Read {}
    #[allow(dead_code)]
    struct Error;
    #[allow(dead_code)]
    enum ErrorKind {
        InvalidData,
    }
    #[allow(dead_code)]
    struct Result;
    #[allow(dead_code)]
    trait Default {}
    #[allow(dead_code)]
    struct __R;

    #[allow(dead_code)]
    mod de {}
    #[allow(dead_code)]
    mod io {}
    #[allow(dead_code)]
    mod __private {}
    #[allow(dead_code)]
    mod maybestd {}

    #[allow(unused_macros)]
    macro_rules! format {
        ($($tokens:tt)*) => {
            compile_error!("the derive used a local `format!`")
        };
    }

    #[allow(dead_code)]
    fn deserialize_reader() {}
    #[allow(dead_code)]
    fn deserialize_variant() {}
    #[allow(dead_code)]
    fn deserialize_bounded_alloc() {}
    #[allow(dead_code)]
    fn deserialize_variant_bounded_alloc() {}
    #[allow(dead_code)]
    fn reader() {}
    #[allow(dead_code)]
    fn budget() {}
    #[allow(dead_code)]
    fn tag() {}
    #[allow(dead_code)]
    fn variant_tag() {}
    #[allow(dead_code)]
    fn return_value() {}

    #[allow(dead_code)]
    fn decode_bytes<R2: ::borsh::io::Read>(
        reader: &mut R2,
        budget: &mut ::borsh::de::Budget,
    ) -> ::borsh::io::Result<Vec<u8>> {
        <Vec<u8> as ::borsh::BorshDeserializeBounded>::deserialize_bounded_alloc(reader, budget)
    }

    #[allow(dead_code)]
    #[derive(::borsh::BorshDeserializeBounded)]
    #[borsh(init = init)]
    struct Named<T, U> {
        x: T,
        #[borsh(skip)]
        y: U,
        #[borsh(deserialize_with_bounded = "decode_bytes")]
        z: Vec<u8>,
    }

    impl<T, U> Named<T, U> {
        #[allow(dead_code)]
        fn init(&mut self) {}
    }

    #[allow(dead_code)]
    #[derive(::borsh::BorshDeserializeBounded)]
    #[borsh(init = init)]
    enum WithFields<T> {
        A {
            x: T,
            #[borsh(skip)]
            y: u8,
        },
        B(#[borsh(deserialize_with_bounded = "decode_bytes")] Vec<u8>),
        C,
    }

    impl<T> WithFields<T> {
        #[allow(dead_code)]
        fn init(&mut self) {}
    }
}

mod shadowed_borsh {
    #[allow(dead_code)]
    mod borsh {}

    mod reexport {
        pub use ::borsh as renamed;
    }

    #[allow(dead_code)]
    #[derive(::borsh::BorshDeserialize, ::borsh::BorshDeserializeBounded)]
    #[borsh(crate = "::borsh")]
    struct Absolute<T> {
        x: T,
        #[borsh(skip)]
        y: u8,
    }

    #[allow(dead_code)]
    #[derive(::borsh::BorshDeserialize, ::borsh::BorshDeserializeBounded)]
    #[borsh(crate = "reexport::renamed")]
    enum Renamed<T> {
        A(T),
        B { x: u8 },
    }
}

/// The generated budget binding must not capture a custom decoder named `budget`
/// or collide with generic parameters.
mod budget_names {
    #![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

    use borsh::io::{Read, Result};
    use borsh::{BorshDeserializeBounded, Budget};

    fn budget<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<u8> {
        u8::deserialize_bounded_alloc(reader, budget)
    }

    #[derive(borsh::BorshDeserializeBounded)]
    struct WithFunction {
        #[borsh(deserialize_with_bounded = "budget")]
        x: u8,
    }

    #[derive(borsh::BorshDeserialize, borsh::BorshDeserializeBounded)]
    struct WithConstGeneric<const budget: usize> {
        x: [u8; budget],
    }

    #[derive(borsh::BorshDeserialize, borsh::BorshDeserializeBounded)]
    struct WithReservedConstGeneric<const __budget: usize> {
        x: [u8; __budget],
    }

    #[derive(borsh::BorshDeserialize, borsh::BorshDeserializeBounded)]
    enum WithReservedNames<__budget, const __budget_: usize> {
        A(__budget, [u8; __budget_]),
    }

    #[derive(borsh::BorshDeserialize, borsh::BorshDeserializeBounded)]
    struct WithRawConstGeneric<const r#__budget: usize> {
        x: [u8; r#__budget],
    }
}
