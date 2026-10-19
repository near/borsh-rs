A data-structure that can be de-serialized from binary format by NBOR, like
[`BorshDeserialize`](crate::BorshDeserialize), while charging a memory limit.

Use it to decode untrusted input with a memory limit. The encoding is the same as for
`BorshDeserialize`; only the memory accounting is added. Each decode gets a
[`Budget`](crate::de::Budget), and every implementation charges it before it allocates.
When a charge does not fit, decoding stops with a
[`LimitExceeded`](crate::error::LimitExceeded) error.

```rust
use borsh::{from_slice_bounded, to_vec};

let encoded = to_vec(&vec![String::from("abc"); 3]).unwrap();
// The outer vector holds three `String`s; each string holds 3 bytes.
let limit = 3 * core::mem::size_of::<String>() + 3 * 3;
let decoded = from_slice_bounded::<Vec<String>>(&encoded, limit).unwrap();
assert_eq!(decoded, vec!["abc"; 3]);
assert!(from_slice_bounded::<Vec<String>>(&encoded, limit - 1).is_err());
```

The trait is implemented for the types listed in [What is charged](#what-is-charged). A type without an
implementation cannot be decoded bounded: that is a compile error, never a silent fallback
to plain decoding. `BorshDeserialize` is not a supertrait, and plain decoding is unchanged.

# How charging works

- A charge is made before the allocation it pays for, and it is never refunded.
- The limit caps the total of all charges in one decode, not the peak memory.
- A collection charges for all its elements as soon as it has read its length prefix, so a
  length that cannot fit fails before any element is read.
- Charges only depend on the input, never on how much of the budget is left.

# Allocation

- **Exact, up front.** Once a length is charged, the decoder allocates room for exactly
  that many elements. A decoded `Vec` or `String` therefore has its capacity
  equal to its length, where plain decoding may leave more capacity. The values are equal
  either way.
- **The limit is also the largest buffer.** A false length prefix within the budget makes
  the decoder allocate up to the remaining budget before the input runs out and decoding
  fails as truncated input. Pick a limit the process can actually provide.
- **Allocation failure aborts**, as with plain decoding.

# Guarantees

**Threshold.** Let `U` be the bytes charged by a successful decode of some input. Then every
limit of at least `U` decodes that input to the same value, and every limit below `U` fails
with `LimitExceeded`. This assumes that the reader and any hand-written code are
deterministic, that charges do not depend on the budget, and that allocations succeed.

**Parity with plain decoding.** With a limit of at least `U`, a bounded decode returns the
same value as `BorshDeserialize`, or fails with the same error kind and message. Simple
types delegate to their plain implementation, and containers keep the plain order of
checks, the same reads and the same constructors. Hand-written implementations, conversion
hooks and `#[borsh(init)]` functions must decode the same way in both traits, and must not
depend on allocated capacity.

Where plain decoding fails, bounded decoding can fail with `LimitExceeded` instead, when the
input claims more than the limit allows. It also fails with `LimitExceeded`, with
`requested = usize::MAX`, when a charge overflows `usize` or is larger than any allocation
can be, and on 16-bit targets when a length prefix does not fit `usize`.

# What is charged

`n` is the number of elements read from the length prefix. Collections charge nothing for
`n = 0`.

| Type | Charge | Exactness |
|---|---|---|
| integers, `f32`, `f64`, `bool`, `NonZero*`, `()`, `RangeFull`, `core::net` types | 0 | exact |
| `Vec<T>`, `String`, `Box<[T]>`, `Box<str>` | `n·size_of::<T>()` (1 byte per element of strings) | exact |
| `Box<T>` of a sized `T` | `size_of::<T>()`, after `T` is decoded | exact for the standard library's conversions |

Worked examples on a 64-bit target:

```rust
use borsh::de::Budget;
use borsh::{to_vec, BorshDeserializeBounded, BorshSerialize};

fn charged<T: BorshSerialize + BorshDeserializeBounded>(value: &T) -> usize {
    let input = to_vec(value).unwrap();
    let limit = 1 << 20;
    let mut budget = Budget::new(limit);
    T::deserialize_bounded(&mut input.as_slice(), &mut budget).unwrap();
    limit - budget.remaining()
}

# #[cfg(target_pointer_width = "64")]
# {
assert_eq!(charged(&vec![0u64; 1000]), 8000);
# }
```

# What is not charged

- stack memory and recursion depth;
- `#[borsh(init)]` functions and `Default` values of skipped fields;
- allocations inside hand-written implementations, unless they charge for them or decode
  their parts with `deserialize_bounded`; plain `BorshDeserialize` calls made from there are
  not charged either;
- error values, reader buffers and allocator overhead.

# Caveats

- `Box<T>` charges only the box: `size_of::<T>()` for a sized `T`, nothing more for an
  unsized one, whose box reuses the decoded buffer. Allocations made by a custom conversion
  from `T::Owned` into `Box<T>` are not charged.
- The formulas may be refined in a minor release, with a changelog entry. A refinement can
  move the threshold of an input in either direction.

# Writing an implementation by hand

Decode each part with its `deserialize_bounded` and pass on the budget you were given. Call
[`Budget::charge`](crate::de::Budget::charge) or
[`Budget::charge_array`](crate::de::Budget::charge_array) before an allocation you make
yourself, and use [`EnumExtBounded`](crate::de::EnumExtBounded) to dispatch on a tag you
have already read. Because the budget is passed separately from the reader, a part can be
decoded from any reader, for example one that puts back a byte that was read ahead:

```rust
use borsh::de::Budget;
use borsh::io::{Read, Result};
use borsh::BorshDeserializeBounded;

/// A list of numbers that was written without a version tag before version 1 added a name.
#[derive(Debug, PartialEq)]
enum Versioned {
    Legacy(Vec<u64>),
    V1 { name: String, numbers: Vec<u64> },
}

impl BorshDeserializeBounded for Versioned {
    fn deserialize_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let tag = u8::deserialize_bounded(reader, budget)?;
        if tag == 0xff {
            Ok(Versioned::V1 {
                name: String::deserialize_bounded(reader, budget)?,
                numbers: Vec::deserialize_bounded(reader, budget)?,
            })
        } else {
            // A legacy value starts with its length; `tag` was its first byte.
            # #[cfg(feature = "std")]
            # {
            let tag = [tag];
            let mut legacy = (&tag[..]).chain(reader);
            Ok(Versioned::Legacy(Vec::deserialize_bounded(&mut legacy, budget)?))
            # }
            # #[cfg(not(feature = "std"))]
            # unimplemented!()
        }
    }
}

# #[cfg(feature = "std")]
# {
let legacy = borsh::to_vec(&vec![7u64, 8]).unwrap();
let decoded = borsh::from_slice_bounded::<Versioned>(&legacy, 16).unwrap();
assert_eq!(decoded, Versioned::Legacy(vec![7, 8]));
# }
```

A type from another crate can only get an implementation in that crate or in borsh, as with
`BorshDeserialize`. Until it has one, wrap it in a type of your own and implement the trait
for the wrapper.
