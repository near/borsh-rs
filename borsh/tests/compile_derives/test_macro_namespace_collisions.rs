// Borsh macros should not collide with the local modules:
// https://github.com/near/borsh-rs/issues/11
mod std {}
mod core {}

#[allow(unused)]
#[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
struct A;

#[allow(unused)]
#[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
enum B {
    C,
    D,
}

// Neither a variant field named `writer` nor variants named `Ok` and `Err` in scope may capture
// names in the generated code, and `#[borsh(skip)]` must not reach `Default` through `mod core`.
#[allow(unused)]
#[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
enum Sink {
    File {
        writer: u8,
        #[borsh(skip)]
        cache: u8,
    },
    Ok,
    Err(u8),
}

#[allow(unused_imports)]
use Sink::*;

// A trait in scope with its own `into` method on `&str` must not make the generated schema code
// ambiguous.
#[cfg(feature = "unstable__schema")]
mod schema_into_collision {
    use alloc::string::String;

    #[allow(unused)]
    trait OtherInto {
        fn into(self) -> String;
    }

    impl OtherInto for &str {
        fn into(self) -> String {
            String::from(self)
        }
    }

    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    struct Unit;

    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    struct Named<T> {
        field: T,
    }

    #[allow(unused)]
    #[derive(borsh::BorshSchema)]
    enum Variants {
        A,
        B(u8),
    }
}

// A skipped generic field adds a `Default` bound to the generated where clause, which must not
// resolve through the local `mod core`.
#[allow(unused)]
#[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
struct SkippedGeneric<T, U> {
    a: T,
    #[borsh(skip)]
    b: U,
}
