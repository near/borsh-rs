use alloc::{rc::Rc, sync::Arc, vec};
use core::mem::size_of;

use borsh::io::{Read, Result};
use borsh::{from_slice_bounded, BorshDeserializeBounded, Budget};

use crate::common_macro::limit_exceeded;

#[allow(unused)]
#[derive(Clone)]
enum Chain {
    End,
    Rc(Rc<Chain>),
    Arc(Arc<Chain>),
}

impl BorshDeserializeBounded for Chain {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        Ok(match u8::deserialize_bounded_alloc(reader, budget)? {
            0 => Chain::End,
            1 => Chain::Rc(Rc::deserialize_bounded_alloc(reader, budget)?),
            _ => Chain::Arc(Arc::deserialize_bounded_alloc(reader, budget)?),
        })
    }
}

#[test]
fn test_deep_rc_chains_fail_on_the_way_down() {
    let limit = 10 * size_of::<Chain>();
    for tag in [1u8, 2] {
        let input = vec![tag; 1_000_000];
        let err = from_slice_bounded::<Chain>(&input, &mut Budget::from_bytes(limit))
            .err()
            .expect("the chain never ends");
        // The root and 9 boxes fit.
        assert_eq!(
            limit_exceeded(&err),
            Some((limit, size_of::<Chain>(), 0)),
            "tag {}",
            tag
        );
    }
}
