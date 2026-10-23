//! Bounds how deeply nested values may be while they are deserialized.
//!
//! A Rust type can only refer to itself through indirection, and every
//! collection and smart pointer in this crate deserializes through
//! `Box<T>` or the element loop of `Vec<T>`. Each of those enters a
//! `RecursionGuard` for the duration of its inner deserialization, so the
//! number of live guards on a thread is the current nesting depth. Exceeding
//! the limit returns an error instead of overflowing the stack.
//!
//! The counter is thread-local, so it needs `std`. Without `std` the guard is
//! a no-op.

/// Default limit on nesting depth for [`BorshDeserialize`](super::BorshDeserialize).
///
/// Every `Box<T>` and every non-empty `Vec<T>` (and the collections built on
/// it, such as `VecDeque`, `BTreeMap` or `HashSet`) that is being
/// deserialized counts as one level, so `Vec<Box<T>>` uses two levels per
/// level of nesting. Use [`from_slice_with_recursion_limit`](super::from_slice_with_recursion_limit)
/// or [`from_reader_with_recursion_limit`](super::from_reader_with_recursion_limit)
/// to deserialize with a different limit.
///
/// The limit only applies when borsh is built with the `std` feature.
pub const DEFAULT_RECURSION_LIMIT: usize = 1024;

#[cfg(feature = "std")]
pub(crate) use imp::*;
#[cfg(not(feature = "std"))]
pub(crate) use noop::*;

#[cfg(feature = "std")]
mod imp {
    use core::cell::Cell;

    use super::DEFAULT_RECURSION_LIMIT;
    use crate::io::{Error, ErrorKind, Result};

    const ERROR_RECURSION_LIMIT_EXCEEDED: &str = "Recursion limit exceeded";

    struct State {
        depth: Cell<usize>,
        limit: Cell<usize>,
    }

    std::thread_local! {
        static STATE: State = const {
            State {
                depth: Cell::new(0),
                limit: Cell::new(DEFAULT_RECURSION_LIMIT),
            }
        };
    }

    /// One level of nesting. Releases the level when dropped, including on
    /// early return and unwinding.
    pub(crate) struct RecursionGuard {
        // Thread-local state: keep the guard on the thread that created it.
        _not_send: core::marker::PhantomData<*const ()>,
    }

    impl RecursionGuard {
        #[inline]
        pub(crate) fn enter() -> Result<Self> {
            STATE.with(|state| {
                let depth = state.depth.get();
                if depth >= state.limit.get() {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        ERROR_RECURSION_LIMIT_EXCEEDED,
                    ));
                }
                state.depth.set(depth + 1);
                Ok(RecursionGuard {
                    _not_send: core::marker::PhantomData,
                })
            })
        }
    }

    impl Drop for RecursionGuard {
        #[inline]
        fn drop(&mut self) {
            STATE.with(|state| state.depth.set(state.depth.get() - 1));
        }
    }

    /// Runs `f` with the nesting limit set to `limit`, restoring the previous
    /// limit afterwards. Depth already in use by an enclosing call still counts.
    pub(crate) fn with_limit<T>(limit: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
        struct Restore(usize);
        impl Drop for Restore {
            fn drop(&mut self) {
                STATE.with(|state| state.limit.set(self.0));
            }
        }
        let _restore = Restore(STATE.with(|state| state.limit.replace(limit)));
        f()
    }
}

#[cfg(not(feature = "std"))]
mod noop {
    use crate::io::Result;

    pub(crate) struct RecursionGuard;

    impl RecursionGuard {
        #[inline(always)]
        pub(crate) fn enter() -> Result<Self> {
            Ok(RecursionGuard)
        }
    }
}
