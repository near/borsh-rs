#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "rc")]

#[cfg(feature = "std")]
pub use std::{rc, sync};

extern crate alloc;
#[cfg(not(feature = "std"))]
pub use alloc::{rc, sync};

use borsh::{from_slice, to_vec};

#[test]
fn test_rc_roundtrip() {
    let value = rc::Rc::new(8u8);
    let serialized = to_vec(&value).unwrap();
    let deserialized = from_slice::<rc::Rc<u8>>(&serialized).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_slice_rc() {
    let original: &[i32] = &[1, 2, 3, 4, 6, 9, 10];
    let shared: rc::Rc<[i32]> = rc::Rc::from(original);
    let serialized = to_vec(&shared).unwrap();
    let deserialized = from_slice::<rc::Rc<[i32]>>(&serialized).unwrap();
    assert_eq!(original, &*deserialized);
}

#[test]
fn test_arc_roundtrip() {
    let value = sync::Arc::new(8u8);
    let serialized = to_vec(&value).unwrap();
    let deserialized = from_slice::<sync::Arc<u8>>(&serialized).unwrap();
    assert_eq!(value, deserialized);
}

#[test]
fn test_slice_arc() {
    let original: &[i32] = &[1, 2, 3, 4, 6, 9, 10];
    let shared: sync::Arc<[i32]> = sync::Arc::from(original);
    let serialized = to_vec(&shared).unwrap();
    let deserialized = from_slice::<sync::Arc<[i32]>>(&serialized).unwrap();
    assert_eq!(original, &*deserialized);
}

#[cfg(feature = "unstable__schema")]
mod schema {
    use super::{rc, sync};
    use alloc::{
        collections::BTreeMap,
        string::{String, ToString},
    };
    use borsh::schema::{BorshSchema, Definition};
    macro_rules! map(
        () => { BTreeMap::new() };
        { $($key:expr => $value:expr),+ } => {
            {
                let mut m = BTreeMap::new();
                $(
                    m.insert($key.to_string(), $value);
                )+
                m
            }
         };
        );
    fn common_map_i32() -> BTreeMap<String, Definition> {
        map! {

            "i32" => Definition::Primitive(4)
        }
    }

    fn common_map_slice_i32() -> BTreeMap<String, Definition> {
        map! {
            "Vec<i32>" => Definition::Sequence {
                length_width: Definition::DEFAULT_LENGTH_WIDTH,
                length_range: Definition::DEFAULT_LENGTH_RANGE,
                elements: "i32".to_string()
            },
            "i32" => Definition::Primitive(4)
        }
    }

    #[test]
    fn test_rc() {
        assert_eq!("i32", <rc::Rc<i32> as BorshSchema>::declaration());

        let mut actual_defs = map!();
        <rc::Rc<i32> as BorshSchema>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(common_map_i32(), actual_defs);
    }

    #[test]
    fn test_slice_rc() {
        assert_eq!("Vec<i32>", <rc::Rc<[i32]> as BorshSchema>::declaration());
        let mut actual_defs = map!();
        <rc::Rc<[i32]> as BorshSchema>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(common_map_slice_i32(), actual_defs);
    }

    #[test]
    fn test_arc() {
        assert_eq!("i32", <sync::Arc<i32> as BorshSchema>::declaration());
        let mut actual_defs = map!();
        <sync::Arc<i32> as BorshSchema>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(common_map_i32(), actual_defs);
    }

    #[test]
    fn test_slice_arc() {
        assert_eq!("Vec<i32>", <sync::Arc<[i32]> as BorshSchema>::declaration());
        let mut actual_defs = map!();
        <sync::Arc<[i32]> as BorshSchema>::add_definitions_recursively(&mut actual_defs);
        assert_eq!(common_map_slice_i32(), actual_defs);
    }
}
