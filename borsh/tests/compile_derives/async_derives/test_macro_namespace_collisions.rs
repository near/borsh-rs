// Borsh macros should not collide with the local modules:
// https://github.com/near/borsh-rs/issues/11
mod std {}
mod core {}

#[derive(borsh::BorshSerializeAsync, borsh::BorshDeserializeAsync)]
struct A;

#[derive(borsh::BorshSerializeAsync, borsh::BorshDeserializeAsync)]
enum B {
    C,
    D,
}

#[derive(borsh::BorshSerializeAsync, borsh::BorshDeserializeAsync)]
struct C {
    x: u64,
    #[allow(unused)]
    #[borsh(skip)]
    y: u64,
}
