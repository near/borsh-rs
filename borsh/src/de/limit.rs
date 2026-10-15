//! Vector allocation charges share a thread-local budget under `de_limit`.
//! Without it, charging is a no-op.

#[cfg(feature = "de_limit")]
pub(crate) use self::thread_budget::{charge, with_limit};

#[cfg(feature = "de_limit")]
mod thread_budget {
    use core::mem::size_of;
    use std::cell::Cell;

    use crate::error::LimitExceeded;
    use crate::io::{Error, ErrorKind, Result};

    std::thread_local! {
        /// `None` distinguishes an inactive limit from an exhausted budget.
        static REMAINING: Cell<Option<usize>> = const { Cell::new(None) };
    }

    /// A rejected charge leaves the budget unchanged, including on overflow.
    #[inline]
    pub(crate) fn charge<T>(count: usize) -> Result<()> {
        let Some(remaining) = REMAINING.get() else {
            return Ok(());
        };
        match count
            .checked_mul(size_of::<T>())
            .and_then(|bytes| remaining.checked_sub(bytes))
        {
            Some(left) => {
                REMAINING.set(Some(left));
                Ok(())
            }
            None => Err(Error::new(ErrorKind::OutOfMemory, LimitExceeded)),
        }
    }

    /// Nested limits cannot increase the remaining budget. Their charges
    /// reach the enclosing scope even during unwinding.
    pub(crate) fn with_limit<R>(limit: usize, f: impl FnOnce() -> R) -> R {
        struct Restore {
            outer: Option<usize>,
            start: usize,
        }

        impl Drop for Restore {
            fn drop(&mut self) {
                // Inner guards drop first; the remaining budget is still
                // `Some` and cannot exceed `start`.
                let used = self.start - REMAINING.get().unwrap_or(self.start);
                REMAINING.set(self.outer.map(|outer| outer - used));
            }
        }

        let outer = REMAINING.get();
        let start = outer.map_or(limit, |outer| outer.min(limit));
        REMAINING.set(Some(start));
        let _restore = Restore { outer, start };
        f()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn assert_exceeded(result: Result<()>) {
            assert_eq!(result.unwrap_err().kind(), ErrorKind::OutOfMemory);
        }

        #[test]
        fn charge_without_limit_always_succeeds() {
            assert_eq!(REMAINING.get(), None);
            charge::<u64>(usize::MAX).unwrap();
            assert_eq!(REMAINING.get(), None);
        }

        #[test]
        fn failed_charge_leaves_budget_unchanged() {
            with_limit(100, || {
                charge::<u32>(10).unwrap();
                assert_eq!(REMAINING.get(), Some(60));
                assert_exceeded(charge::<u8>(61));
                assert_eq!(REMAINING.get(), Some(60));
                // Exercise byte-count overflow without consuming the budget.
                assert_exceeded(charge::<u64>(usize::MAX));
                assert_eq!(REMAINING.get(), Some(60));
                charge::<u8>(60).unwrap();
                charge::<u8>(0).unwrap();
                assert_eq!(REMAINING.get(), Some(0));
            });
            assert_eq!(REMAINING.get(), None);
        }

        #[test]
        fn overflow_fails_even_with_maximal_limit() {
            with_limit(usize::MAX, || {
                assert_exceeded(charge::<u16>(usize::MAX));
                assert_eq!(REMAINING.get(), Some(usize::MAX));
            });
        }

        #[test]
        fn nested_scope_charges_outer_scope() {
            with_limit(100, || {
                charge::<u8>(10).unwrap();
                with_limit(usize::MAX, || {
                    assert_eq!(REMAINING.get(), Some(90));
                    charge::<u8>(5).unwrap();
                });
                assert_eq!(REMAINING.get(), Some(85));
                with_limit(20, || {
                    assert_eq!(REMAINING.get(), Some(20));
                    charge::<u8>(3).unwrap();
                    assert_exceeded(charge::<u8>(18));
                });
                assert_eq!(REMAINING.get(), Some(82));
            });
            assert_eq!(REMAINING.get(), None);
        }
    }
}

#[cfg(not(feature = "de_limit"))]
#[inline]
// `T` keeps call sites the same as with `de_limit`.
#[allow(clippy::extra_unused_type_parameters)]
pub(crate) fn charge<T>(_count: usize) -> crate::io::Result<()> {
    Ok(())
}
