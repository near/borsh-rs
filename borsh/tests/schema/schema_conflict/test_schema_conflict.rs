use crate::common_macro::schema_imports::*;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};

struct ConflictingSchema;

impl BorshSchema for ConflictingSchema {
    #[inline]
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        let fields = Fields::Empty;
        let def = Definition::Struct { fields };
        add_definition(Self::declaration(), def, definitions);
    }
    #[inline]
    fn declaration() -> Declaration {
        "i64".into()
    }
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_conflict() {
    let mut defs = Default::default();
    <Vec<i64> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);

    <ConflictingSchema as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_vec() {
    let mut defs = Default::default();
    <Vec<i64> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <Vec<ConflictingSchema> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_range() {
    let mut defs = Default::default();
    <core::ops::Range<i64> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <core::ops::Range<ConflictingSchema> as borsh::BorshSchema>::add_definitions_recursively(
        &mut defs,
    );
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_slice() {
    let mut defs = Default::default();
    <[i64] as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <[ConflictingSchema] as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_array() {
    let mut defs = Default::default();
    <[i64; 10] as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <[ConflictingSchema; 10] as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_option() {
    let mut defs = Default::default();
    <Option<i64> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <Option<ConflictingSchema> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[allow(unused)]
#[derive(borsh::BorshSchema)]
struct GenericStruct<T> {
    field: T,
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_struct() {
    let mut defs = Default::default();
    <GenericStruct<i64> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <GenericStruct<ConflictingSchema> as borsh::BorshSchema>::add_definitions_recursively(
        &mut defs,
    );
}

#[allow(unused)]
#[derive(borsh::BorshSchema)]
struct SelfConflictingStruct {
    field_1: i64,
    field_2: ConflictingSchema,
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_self_conflicting_struct() {
    let mut defs = Default::default();
    <SelfConflictingStruct as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[allow(unused)]
#[derive(borsh::BorshSchema)]
enum GenericEnum<T> {
    A { field: T },
    B(u64),
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_enum() {
    let mut defs = Default::default();
    <GenericEnum<i64> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <GenericEnum<ConflictingSchema> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[allow(unused)]
#[derive(borsh::BorshSchema)]
enum SelfConflictingEnum {
    A { field: i64 },
    B { field: ConflictingSchema },
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_self_conflicting_enum() {
    let mut defs = Default::default();
    <SelfConflictingEnum as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_result() {
    let mut defs = Default::default();
    <Result<u8, i64> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <Result<u8, ConflictingSchema> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_btreemap() {
    let mut defs = Default::default();
    <BTreeMap<i64, u8> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <BTreeMap<ConflictingSchema, u8> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_btreeset() {
    let mut defs = Default::default();
    <BTreeSet<i64> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <BTreeSet<ConflictingSchema> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

#[test]
#[should_panic(expected = "Redefining type schema for i64")]
fn test_implicit_conflict_tuple() {
    let mut defs = Default::default();
    <(i64, u8) as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    <(ConflictingSchema, u8) as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

macro_rules! linked_node {
    ($module:ident, $next:ident, $leaf:ty) => {
        #[allow(unused)]
        mod $module {
            use super::*;

            #[derive(borsh::BorshSchema)]
            pub struct Node {
                next: Vec<super::$next::Node>,
                payload: Leaf,
            }

            #[derive(borsh::BorshSchema)]
            pub struct Leaf {
                value: $leaf,
            }
        }
    };
}

linked_node!(a, b, u32);
linked_node!(b, c, u32);
linked_node!(c, d, u32);
linked_node!(d, d, u128);

#[test]
#[should_panic(expected = "Redefining type schema for Leaf")]
fn test_implicit_conflict_behind_recursive_structs() {
    let mut defs = Default::default();
    <a::Node as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
}

linked_node!(e, f, u32);
linked_node!(f, f, u32);

#[test]
fn test_recursive_same_named_structs_without_conflict() {
    let mut defs = Default::default();
    <e::Node as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    assert_eq!(
        defs.keys().collect::<Vec<_>>(),
        ["Leaf", "Node", "Vec<Node>", "u32"]
    );
}

struct EmptyName;

impl BorshSchema for EmptyName {
    fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
        add_definition(Self::declaration(), Definition::Primitive(1), definitions);
    }
    fn declaration() -> Declaration {
        "".into()
    }
}

#[allow(unused)]
#[derive(borsh::BorshSchema)]
struct EmptyNameNode {
    next: Vec<EmptyNameNode>,
    value: EmptyName,
}

#[test]
fn test_empty_declaration_behind_recursive_struct() {
    let mut defs = Default::default();
    <EmptyNameNode as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    assert_eq!(
        defs.keys().collect::<Vec<_>>(),
        ["", "EmptyNameNode", "Vec<EmptyNameNode>"]
    );
}

#[allow(unused)]
#[derive(borsh::BorshSchema)]
struct Signed<T> {
    payload: T,
}

#[allow(unused)]
#[derive(borsh::BorshSchema)]
struct Pair<A, B> {
    a: A,
    b: B,
}

fn walk_pair<A: BorshSchema, B: BorshSchema>(_: &A, _: &B) -> BTreeMap<Declaration, Definition> {
    let mut defs = Default::default();
    <Pair<A, B> as borsh::BorshSchema>::add_definitions_recursively(&mut defs);
    defs
}

macro_rules! block_local_x {
    ($declaration:literal) => {{
        struct X;
        impl BorshSchema for X {
            fn add_definitions_recursively(definitions: &mut BTreeMap<Declaration, Definition>) {
                add_definition(Self::declaration(), Definition::Primitive(1), definitions);
            }
            fn declaration() -> Declaration {
                $declaration.into()
            }
        }
        Signed { payload: X }
    }};
}

#[test]
fn test_same_type_name_different_declarations() {
    // Both `X`s print the same `type_name`, so the walk must not take the second
    // `Signed<X>` for a cycle and skip `X2`.
    let first = block_local_x!("X1");
    let second = block_local_x!("X2");
    assert_eq!(
        walk_pair(&first, &second).keys().collect::<Vec<_>>(),
        ["Pair<Signed<X1>, Signed<X2>>", "Signed<X1>", "Signed<X2>", "X1", "X2"]
    );
}
