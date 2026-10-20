use core::alloc::Layout;
use core::fmt;

use crate::io::{Error, ErrorKind, Result};

/// The memory a bounded decode may still charge.
///
/// [`from_slice_bounded`](crate::from_slice_bounded) and
/// [`from_reader_bounded`](crate::from_reader_bounded) take one. Every
/// [`BorshDeserializeBounded`](crate::BorshDeserializeBounded) implementation charges it
/// before it allocates, and a charge that does not fit fails with
/// [`LimitExceeded`](crate::error::LimitExceeded). Charges are never refunded.
///
/// Rules for implementations:
/// - pass on the budget you were given;
/// - never create a `Budget` inside an implementation;
/// - never let what you decode depend on [`limit`](Budget::limit), [`used`](Budget::used) or
///   [`remaining`](Budget::remaining).
///
/// To share one limit across several values, pass the same budget to each decode, for example
/// to [`deserialize_bounded`](crate::BorshDeserializeBounded::deserialize_bounded) for values
/// that follow each other in one reader:
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

    /// The limit this budget was created with.
    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// The bytes charged so far: [`limit`](Budget::limit) minus [`remaining`](Budget::remaining).
    pub const fn used(&self) -> usize {
        self.limit - self.remaining
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

/// The error a bounded decode returns when a charge does not fit the remaining memory limit.
///
/// It reaches the caller wrapped in an [`io::Error`](crate::io::Error) of kind
/// [`OutOfMemory`](crate::io::ErrorKind::OutOfMemory).
///
/// With the `std` feature, it is the error's inner error:
///
/// ```
/// use borsh::de::Budget;
/// use borsh::error::LimitExceeded;
///
/// // Claims 1000 `u64`s, which need 8000 bytes.
/// let input = 1000u32.to_le_bytes();
/// let err = borsh::from_slice_bounded::<Vec<u64>>(&input, &mut Budget::new(100)).unwrap_err();
/// # #[cfg(feature = "std")]
/// # {
/// let exceeded = err
///     .get_ref()
///     .and_then(|inner| inner.downcast_ref::<LimitExceeded>())
///     .unwrap();
/// assert_eq!((exceeded.limit, exceeded.requested, exceeded.remaining), (100, 8000, 100));
/// # }
/// ```
///
/// Without `std`, `io::Error` only keeps a message, so check
/// `err.kind() == ErrorKind::OutOfMemory`. A reader or a hand-written implementation can
/// return that kind too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct LimitExceeded {
    /// The limit the budget was created with.
    pub limit: usize,
    /// The bytes the failed charge asked for. `usize::MAX` when the amount overflowed or is
    /// not a valid allocation size.
    pub requested: usize,
    /// The bytes that were still available when the charge failed.
    pub remaining: usize,
}

impl LimitExceeded {
    pub(crate) fn into_io_error(self) -> Error {
        #[cfg(feature = "std")]
        {
            Error::new(ErrorKind::OutOfMemory, self)
        }
        #[cfg(not(feature = "std"))]
        {
            use crate::__private::maybestd::string::ToString;
            Error::new(ErrorKind::OutOfMemory, self.to_string())
        }
    }
}

impl fmt::Display for LimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Memory limit exceeded: {} bytes requested, {} of {} bytes remaining",
            self.requested, self.remaining, self.limit
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LimitExceeded {}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn fresh_budget_has_its_limit_and_nothing_used() {
        for limit in [0, 1, 100, usize::MAX] {
            let budget = Budget::new(limit);
            assert_eq!((budget.limit(), budget.used()), (limit, 0));
        }
        const BUDGET: Budget = Budget::new(7);
        const LIMIT_AND_USED: (usize, usize) = (BUDGET.limit(), BUDGET.used());
        assert_eq!(LIMIT_AND_USED, (7, 0));
    }

    #[test]
    fn used_is_the_sum_of_charges() {
        let mut budget = Budget::new(100);
        budget.charge(10).unwrap();
        assert_eq!(budget.used(), 10);
        budget.charge_array::<u32>(5).unwrap();
        budget.charge(0).unwrap();
        assert_eq!(budget.used(), 10 + 20);
        budget.charge(70).unwrap();
        assert_eq!((budget.limit(), budget.used()), (100, 100));

        let mut budget = Budget::new(usize::MAX);
        budget.charge(usize::MAX).unwrap();
        assert_eq!((budget.limit(), budget.used()), (usize::MAX, usize::MAX));
    }

    #[test]
    fn failed_charge_leaves_used_unchanged() {
        let mut budget = Budget::new(100);
        budget.charge(40).unwrap();
        assert_exceeded(budget.charge(61), 100, 61, 60);
        assert_eq!(budget.used(), 40);
        assert_exceeded(budget.charge_array::<u64>(usize::MAX), 100, usize::MAX, 60);
        assert_eq!((budget.limit(), budget.used()), (100, 40));
    }

    #[test]
    fn entry_points_charge_the_budget_they_are_given() {
        use crate::__private::maybestd::vec::Vec;
        use crate::{from_reader_bounded, from_slice_bounded};

        let input = [3, 0, 0, 0, 1, 2, 3, 0];
        let (exact, trailing) = (&input[..7], &input[..]);
        let mut budget = Budget::new(20);
        assert_eq!(
            from_slice_bounded::<Vec<u8>>(exact, &mut budget).unwrap(),
            [1, 2, 3]
        );
        assert_eq!(
            from_reader_bounded::<_, Vec<u8>>(&mut &exact[..], &mut budget).unwrap(),
            [1, 2, 3]
        );
        assert_eq!(budget.used(), 6);
        // Each call still rejects trailing bytes, and keeps the charge made before.
        let err = from_slice_bounded::<Vec<u8>>(trailing, &mut budget).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        let err = from_reader_bounded::<_, Vec<u8>>(&mut &trailing[..], &mut budget).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert_eq!(budget.used(), 12);
        let claims_100 = 100u32.to_le_bytes();
        let result = from_slice_bounded::<Vec<u8>>(&claims_100, &mut budget);
        assert_exceeded(result.map(drop), 20, 100, 8);
        assert_eq!(budget.used(), 12);
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
