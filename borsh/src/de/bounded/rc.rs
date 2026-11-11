use core::alloc::Layout;
use core::sync::atomic::AtomicUsize;

use crate::__private::maybestd::{boxed::Box, rc::Rc, sync::Arc};
use crate::io::{Read, Result};

use super::{size, BorshDeserializeBounded, Budget};

/// Requires the `rc` Cargo feature. Each decoded `Rc` has a strong count of 1.
impl<T: ?Sized> BorshDeserializeBounded for Rc<T>
where
    Box<T>: BorshDeserializeBounded,
{
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let boxed = Box::<T>::deserialize_bounded_alloc(reader, budget)?;
        // `Rc::from` copies the value into a new block.
        budget.charge_checked(size::rc_block::<usize>(Layout::for_value(&*boxed)))?;
        Ok(boxed.into())
    }
}

/// Requires the `rc` Cargo feature. Each decoded `Arc` has a strong count of 1.
impl<T: ?Sized> BorshDeserializeBounded for Arc<T>
where
    Box<T>: BorshDeserializeBounded,
{
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let boxed = Box::<T>::deserialize_bounded_alloc(reader, budget)?;
        // `Arc::from` copies the value into a new block.
        budget.charge_checked(size::rc_block::<AtomicUsize>(Layout::for_value(&*boxed)))?;
        Ok(boxed.into())
    }
}
