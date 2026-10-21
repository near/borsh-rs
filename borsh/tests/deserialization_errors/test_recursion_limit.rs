//! Deeply nested input for a recursive type must be rejected with an error
//! instead of overflowing the stack (which aborts the process).
//!
//! Every test that decodes deep input runs on a thread with a fixed 2 MiB stack
//! (the default for spawned threads), so the outcome doesn't depend on the stack
//! size of the test harness thread.

use alloc::{boxed::Box, string::ToString, vec, vec::Vec};

use borsh::de::{from_slice_with_recursion_limit, DEFAULT_RECURSION_LIMIT};
use borsh::io::ErrorKind;
use borsh::{from_slice, to_vec, BorshDeserialize, BorshSerialize};

const ERROR_RECURSION_LIMIT_EXCEEDED: &str = "Recursion limit exceeded";
const STACK_SIZE: usize = 2 * 1024 * 1024;

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
enum Tree {
    Leaf,
    Node(Box<Tree>),
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
struct VecNode(Vec<VecNode>);

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
struct List {
    value: u64,
    next: Option<Box<List>>,
}

// Mutually recursive pair: neither type mentions itself directly.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
struct Even(Option<Box<Odd>>);

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
struct Odd(Vec<Even>);

fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

fn assert_recursion_limit_error<T: core::fmt::Debug>(result: borsh::io::Result<T>) {
    let err = result.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert_eq!(err.to_string(), ERROR_RECURSION_LIMIT_EXCEEDED);
}

/// `depth` levels of `Tree::Node` around a `Tree::Leaf`.
fn tree_bytes(depth: usize) -> Vec<u8> {
    let mut data = vec![1u8; depth];
    data.push(0);
    data
}

/// `depth` levels of single-element `VecNode`s around an empty one.
fn vec_node_bytes(depth: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(4 * (depth + 1));
    for _ in 0..depth {
        data.extend_from_slice(&1u32.to_le_bytes());
    }
    data.extend_from_slice(&0u32.to_le_bytes());
    data
}

/// A `List` with `depth` boxed successors.
fn list_bytes(depth: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(9 * (depth + 1));
    for i in 0..depth {
        data.extend_from_slice(&(i as u64).to_le_bytes());
        data.push(1);
    }
    data.extend_from_slice(&(depth as u64).to_le_bytes());
    data.push(0);
    data
}

fn deep_tree(depth: usize) -> Tree {
    let mut tree = Tree::Leaf;
    for _ in 0..depth {
        tree = Tree::Node(Box::new(tree));
    }
    tree
}

#[test]
fn test_deep_box_enum_is_rejected() {
    // 1 MiB of `Node` tags: overflowed a 2 MiB stack and aborted before the limit.
    let data = tree_bytes(1024 * 1024);
    on_small_stack(move || {
        assert_recursion_limit_error(from_slice::<Tree>(&data));
        assert_recursion_limit_error(Tree::try_from_slice(&data));
        assert_recursion_limit_error(Tree::deserialize(&mut data.as_slice()));
        assert_recursion_limit_error(borsh::from_reader::<_, Tree>(&mut data.as_slice()));
    });
}

#[test]
fn test_deep_vec_struct_is_rejected() {
    let data = vec_node_bytes(256 * 1024);
    on_small_stack(move || assert_recursion_limit_error(from_slice::<VecNode>(&data)));
}

#[test]
fn test_deep_option_box_chain_is_rejected() {
    let data = list_bytes(256 * 1024);
    on_small_stack(move || assert_recursion_limit_error(from_slice::<List>(&data)));
}

#[test]
fn test_deep_mutual_recursion_is_rejected() {
    // Even(Some(Odd(vec![Even(Some(Odd(vec![ ... ])))]))) repeated.
    let mut data = Vec::new();
    for _ in 0..128 * 1024 {
        data.push(1);
        data.extend_from_slice(&1u32.to_le_bytes());
    }
    data.push(0);
    on_small_stack(move || assert_recursion_limit_error(from_slice::<Even>(&data)));
}

#[test]
fn test_limit_boundary() {
    on_small_stack(|| {
        let at_limit = tree_bytes(DEFAULT_RECURSION_LIMIT);
        let value = from_slice::<Tree>(&at_limit).unwrap();
        assert_eq!(to_vec(&value).unwrap(), at_limit);
        assert_recursion_limit_error(from_slice::<Tree>(&tree_bytes(DEFAULT_RECURSION_LIMIT + 1)));

        let at_limit = vec_node_bytes(DEFAULT_RECURSION_LIMIT);
        let value = from_slice::<VecNode>(&at_limit).unwrap();
        assert_eq!(to_vec(&value).unwrap(), at_limit);
        assert_recursion_limit_error(from_slice::<VecNode>(&vec_node_bytes(
            DEFAULT_RECURSION_LIMIT + 1,
        )));

        let at_limit = list_bytes(DEFAULT_RECURSION_LIMIT);
        let value = from_slice::<List>(&at_limit).unwrap();
        assert_eq!(to_vec(&value).unwrap(), at_limit);
        assert_recursion_limit_error(from_slice::<List>(&list_bytes(DEFAULT_RECURSION_LIMIT + 1)));
    });
}

#[test]
fn test_depth_is_released_after_error() {
    on_small_stack(|| {
        assert_recursion_limit_error(from_slice::<Tree>(&tree_bytes(DEFAULT_RECURSION_LIMIT + 1)));
        // Truncated input fails part-way through the nesting.
        assert!(from_slice::<Tree>(&tree_bytes(100)[..50]).is_err());
        // Neither failure leaks depth into the next call.
        let at_limit = tree_bytes(DEFAULT_RECURSION_LIMIT);
        assert!(from_slice::<Tree>(&at_limit).is_ok());
    });
}

#[test]
fn test_wide_input_is_not_limited() {
    // Siblings don't accumulate depth: only nesting counts.
    let wide: Vec<Box<Tree>> = (0..10 * DEFAULT_RECURSION_LIMIT)
        .map(|_| Box::new(deep_tree(8)))
        .collect();
    let data = to_vec(&wide).unwrap();
    on_small_stack(move || {
        assert_eq!(from_slice::<Vec<Box<Tree>>>(&data).unwrap(), wide);
    });
}

#[test]
fn test_moderate_depth_roundtrip() {
    on_small_stack(|| {
        let tree = deep_tree(200);
        let data = to_vec(&tree).unwrap();
        assert_eq!(from_slice::<Tree>(&data).unwrap(), tree);

        let mut node = VecNode(vec![]);
        for _ in 0..200 {
            node = VecNode(vec![node.clone(), VecNode(vec![])]);
        }
        let data = to_vec(&node).unwrap();
        assert_eq!(from_slice::<VecNode>(&data).unwrap(), node);

        let mut even = Even(None);
        for _ in 0..100 {
            even = Even(Some(Box::new(Odd(vec![even]))));
        }
        let data = to_vec(&even).unwrap();
        assert_eq!(from_slice::<Even>(&data).unwrap(), even);
    });
}

#[test]
fn test_custom_limit() {
    on_small_stack(|| {
        // Lower than the default.
        assert!(from_slice_with_recursion_limit::<Tree>(&tree_bytes(10), 10).is_ok());
        assert_recursion_limit_error(from_slice_with_recursion_limit::<Tree>(&tree_bytes(11), 10));
        assert_recursion_limit_error(from_slice_with_recursion_limit::<Tree>(&tree_bytes(1), 0));
        assert!(from_slice_with_recursion_limit::<Tree>(&tree_bytes(0), 0).is_ok());

        // Higher than the default.
        let depth = 2 * DEFAULT_RECURSION_LIMIT;
        let data = tree_bytes(depth);
        assert_recursion_limit_error(from_slice::<Tree>(&data));
        let value = from_slice_with_recursion_limit::<Tree>(&data, depth).unwrap();
        assert_eq!(to_vec(&value).unwrap(), data);
        assert_recursion_limit_error(from_slice_with_recursion_limit::<Tree>(&data, depth - 1));

        // The previous limit is restored afterwards.
        assert_recursion_limit_error(from_slice::<Tree>(&data));
    });
}

#[test]
fn test_custom_limit_with_reader() {
    on_small_stack(|| {
        let data = tree_bytes(20);
        let mut reader = data.as_slice();
        assert_recursion_limit_error(borsh::de::from_reader_with_recursion_limit::<_, Tree>(
            &mut reader,
            19,
        ));
        let mut reader = data.as_slice();
        assert!(borsh::de::from_reader_with_recursion_limit::<_, Tree>(&mut reader, 20).is_ok());
    });
}
