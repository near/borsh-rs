//! For honest inputs, limits at or above the charge decode and limits below fail on the limit.

use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::fmt::Debug;
use core::mem::size_of;

use borsh::{
    from_reader_bounded, from_slice_bounded, to_vec, BorshDeserializeBounded, BorshSerialize,
    Budget,
};

use super::common::charged;
use crate::common_macro::{is_limit_exceeded, PARITY_LIMIT};

fn assert_threshold<T>(value: &T)
where
    T: BorshSerialize + BorshDeserializeBounded + PartialEq + Debug,
{
    let input = to_vec(value).unwrap();
    let used = size_of::<T>() + charged::<T>(&input);

    for limit in [used, used + 1, PARITY_LIMIT] {
        assert_eq!(
            &from_slice_bounded::<T>(&input, &mut Budget::from_bytes(limit)).unwrap(),
            value
        );
        assert_eq!(
            &from_reader_bounded::<_, T>(&mut &input[..], &mut Budget::from_bytes(limit)).unwrap(),
            value
        );
    }
    if used > 0 {
        for limit in [used - 1, used / 2, 0] {
            let err = from_slice_bounded::<T>(&input, &mut Budget::from_bytes(limit)).unwrap_err();
            assert!(is_limit_exceeded(&err), "{:?} at {}: {}", value, limit, err);
            let err = from_reader_bounded::<_, T>(&mut &input[..], &mut Budget::from_bytes(limit))
                .unwrap_err();
            assert!(is_limit_exceeded(&err), "{:?} at {}: {}", value, limit, err);
        }
    }
}

fn strings(n: usize) -> Vec<String> {
    (0..n).map(|i| "x".repeat(i)).collect()
}

#[test]
fn test_threshold_nested_sequences() {
    assert_threshold(&vec![strings(3), strings(0), strings(20)]);
    assert_threshold(&(
        String::from("abc"),
        vec![(1u8, String::from("d")), (2, String::new())],
        [vec![1u16, 2], vec![], vec![3]],
    ));
    assert_threshold(&vec![Some(Box::new([1u32, 2, 3])), None]);
    assert_threshold(
        &(0..10u8)
            .map(|i| (0..i).collect::<LinkedList<_>>())
            .collect::<VecDeque<_>>(),
    );
    assert_threshold::<core::result::Result<Vec<u8>, String>>(&Ok(vec![1, 2, 3]));
    assert_threshold::<core::result::Result<Vec<u8>, String>>(&Err("no".to_string()));
    assert_threshold(&core::cell::RefCell::new(vec![1u64, 2]));
    assert_threshold(&Box::<[Box<str>]>::from(vec![
        Box::from("a"),
        Box::from("bc"),
    ]));
}

#[test]
fn test_threshold_maps_and_sets() {
    let map = (0..50u64)
        .map(|k| (k.to_string(), vec![k; k as usize % 7]))
        .collect::<BTreeMap<_, _>>();
    assert_threshold(&map);
    assert_threshold(&(0..30).map(|k| strings(k % 4)).collect::<BTreeSet<_>>());

    #[cfg(hash_collections)]
    {
        #[cfg(feature = "hashbrown")]
        use hashbrown::{HashMap, HashSet};
        #[cfg(feature = "std")]
        use std::collections::{HashMap, HashSet};
        let map = (0..40u32)
            .map(|k| {
                (
                    k,
                    (0..k % 5).map(|i| i.to_string()).collect::<BTreeSet<_>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        assert_threshold(&map);
        assert_threshold(&(0..20u16).collect::<HashSet<_>>());
    }
}

#[cfg(feature = "rc")]
#[test]
fn test_threshold_rc() {
    use alloc::{rc::Rc, sync::Arc};

    assert_threshold(&Rc::new(vec![Rc::<str>::from("abc"), Rc::from("")]));
    assert_threshold(&Arc::<[String]>::from(strings(4)));
}
