use borsh::Budget;

use super::cases::cases;
use crate::common_macro::limit_exceeded;

#[test]
fn test_exact_charge_passes_and_one_byte_less_fails() {
    for case in cases() {
        let mut budget = Budget::from_bytes(case.charge);
        (case.decode)(&case.input, &mut budget)
            .unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        assert_eq!(budget.remaining_bytes(), 0, "{}", case.name);

        let mut budget = Budget::from_bytes(case.charge + 1000);
        (case.decode)(&case.input, &mut budget)
            .unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        assert_eq!(budget.remaining_bytes(), 1000, "{}", case.name);

        match case.below {
            None => assert_eq!(case.charge, 0, "{}", case.name),
            Some((requested, remaining)) => {
                let limit = case.charge - 1;
                let mut budget = Budget::from_bytes(limit);
                let err = (case.decode)(&case.input, &mut budget).unwrap_err();
                assert_eq!(
                    limit_exceeded(&err),
                    Some((limit, requested, remaining)),
                    "{}: {}",
                    case.name,
                    err
                );
                assert_eq!(budget.remaining_bytes(), remaining, "{}", case.name);
            }
        }
    }
}

#[cfg(feature = "de_strict_order")]
#[test]
fn test_duplicate_keys_are_rejected_like_plain() {
    use alloc::collections::{BTreeMap, BTreeSet};
    use alloc::vec;
    use borsh::to_vec;

    use super::common::assert_fails_alike;

    let entries = to_vec(&vec![(1u64, 10u64), (1, 11), (2, 12)]).unwrap();
    let keys = to_vec(&vec![1u32, 1, 2]).unwrap();
    assert_fails_alike::<BTreeMap<u64, u64>>(&entries);
    assert_fails_alike::<BTreeSet<u32>>(&keys);
    #[cfg(hash_collections)]
    {
        #[cfg(feature = "hashbrown")]
        use hashbrown::{HashMap, HashSet};
        #[cfg(feature = "std")]
        use std::collections::{HashMap, HashSet};
        assert_fails_alike::<HashMap<u64, u64>>(&entries);
        assert_fails_alike::<HashSet<u32>>(&keys);
    }
}

#[cfg(not(feature = "de_strict_order"))]
#[test]
fn test_duplicate_keys_resolve_like_plain() {
    use alloc::collections::{BTreeMap, BTreeSet};
    use alloc::vec;
    use borsh::{from_slice_bounded, to_vec};

    use crate::common_macro::{assert_bounded_parity, PARITY_LIMIT};

    let entries = to_vec(&vec![(1u64, 10u64), (1, 11), (2, 12)]).unwrap();
    let keys = to_vec(&vec![1u32, 1, 2]).unwrap();

    assert_bounded_parity::<BTreeMap<u64, u64>>(&entries);
    assert_bounded_parity::<BTreeSet<u32>>(&keys);
    let map =
        from_slice_bounded::<BTreeMap<u64, u64>>(&entries, &mut Budget::from_bytes(PARITY_LIMIT))
            .unwrap();
    assert_eq!((map.len(), map[&1]), (2, 11));
    #[cfg(hash_collections)]
    {
        #[cfg(feature = "hashbrown")]
        use hashbrown::{HashMap, HashSet};
        #[cfg(feature = "std")]
        use std::collections::{HashMap, HashSet};
        assert_bounded_parity::<HashMap<u64, u64>>(&entries);
        assert_bounded_parity::<HashSet<u32>>(&keys);
    }
}
