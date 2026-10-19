use core::alloc::Layout;

use crate::error::LimitExceeded;
use crate::io::{Error, Result};

/// The memory a bounded decode may still charge.
///
/// [`from_slice_bounded`](crate::from_slice_bounded) and
/// [`from_reader_bounded`](crate::from_reader_bounded) create one per decode. Every
/// [`BorshDeserializeBounded`](crate::BorshDeserializeBounded) implementation charges it
/// before it allocates, and a charge that does not fit fails with
/// [`LimitExceeded`](crate::error::LimitExceeded). Charges are never refunded.
///
/// Rules for implementations:
/// - pass on the budget you were given;
/// - never create a `Budget` inside an implementation;
/// - never let what you decode depend on [`remaining`](Budget::remaining).
///
/// To share one limit across several values, create a budget yourself and call
/// [`deserialize_bounded`](crate::BorshDeserializeBounded::deserialize_bounded) on each:
///
/// ```
/// use borsh::de::Budget;
/// use borsh::BorshDeserializeBounded;
///
/// let input = borsh::to_vec(&(vec![1u32, 2, 3], String::from("abc"))).unwrap();
/// let mut reader = &input[..];
/// let mut budget = Budget::new(64);
/// let numbers = Vec::<u32>::deserialize_bounded(&mut reader, &mut budget).unwrap();
/// let text = String::deserialize_bounded(&mut reader, &mut budget).unwrap();
/// assert_eq!((numbers, text.as_str()), (vec![1, 2, 3], "abc"));
/// assert_eq!(budget.remaining(), 64 - 12 - 3);
/// ```
#[derive(Debug)]
pub struct Budget {
    limit: usize,
    remaining: usize,
}

impl Budget {
    /// Creates a budget that allows `limit` bytes of charges in total.
    pub const fn new(limit: usize) -> Self {
        Budget {
            limit,
            remaining: limit,
        }
    }

    /// The bytes that can still be charged.
    pub const fn remaining(&self) -> usize {
        self.remaining
    }

    /// Charges `bytes`. Fails with [`LimitExceeded`] only when `bytes` is more than
    /// [`remaining`](Budget::remaining); a failed charge leaves the budget unchanged.
    #[inline]
    pub fn charge(&mut self, bytes: usize) -> Result<()> {
        self.charge_checked(Some(bytes))
    }

    /// Charges the size of an array of `len` values of `T`, as allocated by
    /// `Vec::<T>::with_capacity(len)`.
    ///
    /// A size that overflows or is too large for any allocation fails with
    /// `requested = usize::MAX`, whatever the limit.
    #[inline]
    pub fn charge_array<T>(&mut self, len: usize) -> Result<()> {
        self.charge_checked(Layout::array::<T>(len).ok().map(|layout| layout.size()))
    }

    /// `None` means an overflow or invalid allocation size and always fails.
    #[inline]
    pub(crate) fn charge_checked(&mut self, bytes: Option<usize>) -> Result<()> {
        match bytes {
            Some(bytes) if bytes <= self.remaining => {
                self.remaining -= bytes;
                Ok(())
            }
            Some(bytes) => Err(self.exceeded(bytes)),
            None => Err(self.exceeded(usize::MAX)),
        }
    }

    #[cold]
    #[inline(never)]
    fn exceeded(&self, requested: usize) -> Error {
        LimitExceeded {
            limit: self.limit,
            requested,
            remaining: self.remaining,
        }
        .into_io_error()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::ErrorKind;

    #[track_caller]
    fn assert_exceeded(result: Result<()>, limit: usize, requested: usize, remaining: usize) {
        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::OutOfMemory);
        let expected = LimitExceeded {
            limit,
            requested,
            remaining,
        };
        #[cfg(feature = "std")]
        assert_eq!(
            err.get_ref()
                .and_then(|inner| inner.downcast_ref::<LimitExceeded>()),
            Some(&expected)
        );
        #[cfg(not(feature = "std"))]
        {
            use crate::__private::maybestd::string::ToString;
            assert_eq!(err.to_string(), expected.to_string());
        }
    }

    #[test]
    fn exact_amount_passes() {
        let mut budget = Budget::new(10);
        budget.charge(4).unwrap();
        budget.charge(6).unwrap();
        assert_eq!(budget.remaining(), 0);
        budget.charge(0).unwrap();
        assert_exceeded(budget.charge(1), 10, 1, 0);
    }

    #[test]
    fn failed_charge_leaves_budget_unchanged() {
        let mut budget = Budget::new(100);
        budget.charge_array::<u32>(10).unwrap();
        assert_eq!(budget.remaining(), 60);
        assert_exceeded(budget.charge(61), 100, 61, 60);
        assert_eq!(budget.remaining(), 60);
        assert_exceeded(budget.charge_array::<u64>(usize::MAX), 100, usize::MAX, 60);
        assert_eq!(budget.remaining(), 60);
        assert_exceeded(budget.charge_checked(None), 100, usize::MAX, 60);
        assert_eq!(budget.remaining(), 60);
        budget.charge(60).unwrap();
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn overflow_fails_even_with_maximal_limit() {
        let mut budget = Budget::new(usize::MAX);
        assert_exceeded(
            budget.charge_array::<u16>(usize::MAX),
            usize::MAX,
            usize::MAX,
            usize::MAX,
        );
        // Larger than `isize::MAX`, so no allocation can have this size.
        assert_exceeded(
            budget.charge_array::<u8>(usize::MAX),
            usize::MAX,
            usize::MAX,
            usize::MAX,
        );
        assert_exceeded(
            budget.charge_checked(None),
            usize::MAX,
            usize::MAX,
            usize::MAX,
        );
        assert_eq!(budget.remaining(), usize::MAX);
        // A plain byte count of `usize::MAX` is not an overflow.
        budget.charge(usize::MAX).unwrap();
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn charge_array_charges_the_vector_size() {
        let mut budget = Budget::new(1000);
        budget.charge_array::<u64>(10).unwrap();
        budget.charge_array::<()>(1_000_000).unwrap();
        budget.charge_array::<[u8; 3]>(5).unwrap();
        assert_eq!(budget.remaining(), 1000 - 80 - 15);
    }

    #[test]
    fn limit_exceeded_display() {
        use crate::__private::maybestd::string::ToString;
        let exceeded = LimitExceeded {
            limit: 100,
            requested: 61,
            remaining: 60,
        };
        let expected = "Memory limit exceeded: 61 bytes requested, 60 of 100 bytes remaining";
        assert_eq!(exceeded.to_string(), expected);
        assert_eq!(exceeded.into_io_error().to_string(), expected);
    }
}
