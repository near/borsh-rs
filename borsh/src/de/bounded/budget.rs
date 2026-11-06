use core::alloc::Layout;
use core::fmt;

use crate::io::{Error, ErrorKind, Result};

/// Memory budget available for deserialization. Decoding charges the value it returns and
/// every allocation against it.
///
/// To share one limit across several values, pass the same budget to each decode: to each
/// [`from_slice_bounded`](crate::from_slice_bounded) call, or to
/// [`deserialize_bounded_alloc`](crate::BorshDeserializeBounded::deserialize_bounded_alloc)
/// for values that follow each other in one reader. A failed decode keeps the charges it made.
///
/// ```
/// use borsh::{BorshDeserializeBounded, Budget};
///
/// let input = borsh::to_vec(&(vec![1u32, 2, 3], String::from("abc"))).unwrap();
/// let mut reader = &input[..];
/// let mut budget = Budget::new(64);
/// let numbers = Vec::<u32>::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
/// let text = String::deserialize_bounded_alloc(&mut reader, &mut budget).unwrap();
/// assert_eq!((numbers, text.as_str()), (vec![1, 2, 3], "abc"));
/// // `deserialize_bounded_alloc` charges only the allocations.
/// assert_eq!(budget.used(), 12 + 3);
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

    /// Charges `bytes`. Fails with an error of kind `OutOfMemory` (see [`LimitExceeded`]) only
    /// when `bytes` is more than [`remaining`](Budget::remaining); a failed charge leaves the
    /// budget unchanged.
    #[inline]
    pub fn charge(&mut self, bytes: usize) -> Result<()> {
        if bytes > self.remaining {
            return Err(self.exceeded(bytes));
        }
        self.remaining -= bytes;
        Ok(())
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

    /// Like `charge`, but `None` (an overflow or an invalid allocation size) always fails,
    /// even with a limit of `usize::MAX`.
    #[inline]
    pub(crate) fn charge_checked(&mut self, bytes: Option<usize>) -> Result<()> {
        match bytes {
            Some(bytes) => self.charge(bytes),
            None => Err(self.exceeded(usize::MAX)),
        }
    }

    #[cold]
    #[inline(never)]
    pub(super) fn exceeded(&self, requested: usize) -> Error {
        let exceeded = LimitExceeded {
            limit: self.limit,
            requested,
            remaining: self.remaining,
        };
        #[cfg(feature = "std")]
        {
            Error::new(ErrorKind::OutOfMemory, exceeded)
        }
        #[cfg(not(feature = "std"))]
        {
            use crate::__private::maybestd::string::ToString;
            Error::new(ErrorKind::OutOfMemory, exceeded.to_string())
        }
    }
}

/// The error of a charge that does not fit the remaining [`Budget`].
///
/// It reaches the caller as an [`io::Error`](crate::io::Error) of kind
/// [`OutOfMemory`](crate::io::ErrorKind::OutOfMemory). With the `std` feature,
/// [`from_io_error`](LimitExceeded::from_io_error) gets it back:
///
/// ```
/// use borsh::error::LimitExceeded;
/// use borsh::Budget;
///
/// // Claims a string of 5 bytes.
/// let input = 5u32.to_le_bytes();
/// // Room for the `String` itself and 4 bytes.
/// let limit = core::mem::size_of::<String>() + 4;
/// let err = borsh::from_slice_bounded::<String>(&input, &mut Budget::new(limit)).unwrap_err();
/// # #[cfg(feature = "std")]
/// # {
/// let exceeded = LimitExceeded::from_io_error(&err).unwrap();
/// assert_eq!((exceeded.limit, exceeded.requested, exceeded.remaining), (limit, 5, 4));
/// # }
/// ```
///
/// Without `std`, `io::Error` only keeps the message, so check
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
    /// The limit error inside `err`, or `None` if `err` is not one.
    #[cfg(feature = "std")]
    pub fn from_io_error(err: &Error) -> Option<&LimitExceeded> {
        err.get_ref()?.downcast_ref()
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
    use super::super::tests::assert_exceeded;
    use super::*;

    #[test]
    fn new_budget_has_its_limit_and_nothing_used() {
        for limit in [0, 1, 100, usize::MAX] {
            let budget = Budget::new(limit);
            assert_eq!(
                (budget.limit(), budget.used(), budget.remaining()),
                (limit, 0, limit)
            );
        }
        const BUDGET: Budget = Budget::new(7);
        const LIMIT_AND_USED: (usize, usize) = (BUDGET.limit(), BUDGET.used());
        assert_eq!(LIMIT_AND_USED, (7, 0));
    }

    #[test]
    fn charges_add_up_to_the_limit() {
        let mut budget = Budget::new(1000);
        budget.charge(5).unwrap();
        budget.charge(0).unwrap();
        budget.charge_array::<u64>(10).unwrap();
        budget.charge_array::<()>(1_000_000).unwrap();
        budget.charge_array::<[u8; 3]>(5).unwrap();
        assert_eq!(budget.used(), 5 + 80 + 15);
        budget.charge(900).unwrap();
        assert_eq!((budget.used(), budget.remaining()), (1000, 0));
        budget.charge(0).unwrap();
        assert_exceeded(budget.charge(1), 1000, 1, 0);
    }

    #[test]
    fn failed_charge_leaves_budget_unchanged() {
        let mut budget = Budget::new(100);
        budget.charge_array::<u32>(10).unwrap();
        // `remaining` in each error shows that the previous failure changed nothing.
        assert_exceeded(budget.charge(61), 100, 61, 60);
        assert_exceeded(budget.charge_array::<u64>(usize::MAX), 100, usize::MAX, 60);
        assert_exceeded(budget.charge_checked(None), 100, usize::MAX, 60);
        assert_eq!((budget.used(), budget.remaining()), (40, 60));
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
        assert_eq!((budget.used(), budget.remaining()), (usize::MAX, 0));
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
        let mut budget = Budget::new(100);
        budget.charge(40).unwrap();
        assert_eq!(budget.charge(61).unwrap_err().to_string(), expected);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_io_error_finds_only_limit_errors() {
        let err = Budget::new(0).charge(1).unwrap_err();
        let expected = LimitExceeded {
            limit: 0,
            requested: 1,
            remaining: 0,
        };
        assert_eq!(LimitExceeded::from_io_error(&err), Some(&expected));
        let message = Error::new(ErrorKind::OutOfMemory, "x");
        assert_eq!(LimitExceeded::from_io_error(&message), None);
        let kind_only = Error::from(ErrorKind::OutOfMemory);
        assert_eq!(LimitExceeded::from_io_error(&kind_only), None);
    }
}
