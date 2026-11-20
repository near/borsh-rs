use alloc::{boxed::Box, string::String, vec::Vec};

use borsh::BorshDeserializeBounded;

/// The derived bound is `T: BorshDeserializeBounded`; `Box<List<T>>` also needs
/// `List<T>: Clone`, so the override adds `T: Clone`.
#[allow(dead_code)]
#[derive(Clone, BorshDeserializeBounded)]
enum List<T> {
    Nil,
    Cons(
        #[borsh(bound(deserialize_bounded = "T: borsh::de::BorshDeserializeBounded + Clone"))] T,
        Box<List<T>>,
    ),
}

#[allow(dead_code)]
#[derive(BorshDeserializeBounded)]
struct Forest<T> {
    trees: Vec<Tree<T>>,
}

#[allow(dead_code)]
#[derive(BorshDeserializeBounded)]
enum Tree<T> {
    Leaf(T),
    Branch { label: String, children: Forest<T> },
}

#[allow(dead_code)]
fn instantiations() {
    fn check<T: BorshDeserializeBounded>() {}

    check::<List<String>>();
    check::<Forest<u8>>();
}
