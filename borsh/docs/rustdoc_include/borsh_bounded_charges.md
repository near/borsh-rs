A data-structure that can be de-serialized from binary format by NBOR, like
[`BorshDeserialize`](crate::BorshDeserialize), while charging a memory budget.

Use it to decode untrusted input with a memory limit. The encoding is the same as for
`BorshDeserialize`. Each decode gets a [`Budget`](crate::de::Budget), and every
implementation charges it before it allocates. A charge that does not fit fails the decode
with an error of kind `OutOfMemory` (see [`LimitExceeded`](crate::error::LimitExceeded)).

```rust
use borsh::de::Budget;
use borsh::{from_slice_bounded, to_vec};

let encoded = to_vec(&vec![String::from("abc"); 3]).unwrap();
// The outer vector holds three `String`s; each string holds 3 bytes.
let limit = 3 * core::mem::size_of::<String>() + 3 * 3;
let decoded = from_slice_bounded::<Vec<String>>(&encoded, &mut Budget::new(limit)).unwrap();
assert_eq!(decoded, vec!["abc"; 3]);
assert!(from_slice_bounded::<Vec<String>>(&encoded, &mut Budget::new(limit - 1)).is_err());
```

The trait is implemented for the types listed in [What is charged](#what-is-charged). A
type without an implementation cannot be decoded bounded: that is a compile error, never a
silent fallback to plain decoding. `BorshDeserialize` is not a supertrait, and plain
decoding is unchanged.

# How charging works

- A charge is never refunded.
- The limit caps the total of all charges made against one budget, not the peak memory.
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

- Unless it fails with `LimitExceeded`, a bounded decode returns what `BorshDeserialize`
  returns: the same value, or an error of the same kind and message.
- After a successful decode with a fresh budget, [`used()`](crate::de::Budget::used) is the
  smallest limit that decodes this input.

Both assume that the reader is deterministic, that hand-written implementations follow the
[rules](#writing-an-implementation-by-hand), and that allocations succeed.

# What is charged

`n` is the number of elements read from the length prefix. Collections charge nothing for
`n = 0`. A charge is exact when it equals the bytes the decoder asks the allocator for.

| Type | Charge | Exactness |
|---|---|---|
| integers, `f32`, `f64`, `bool`, `NonZero*`, `()`, `RangeFull`, `core::net` types | 0 | exact |
| `Vec<T>`, `String`, `Box<[T]>`, `Box<str>` | `n·size_of::<T>()` (1 byte per element of strings) | exact |
| `Box<T>` of a sized `T` | `size_of::<T>()`, after `T` is decoded | exact for the standard library's conversions |

Worked example on a 64-bit target:

```rust
use borsh::de::Budget;
use borsh::{to_vec, BorshDeserializeBounded, BorshSerialize};

fn charged<T: BorshSerialize + BorshDeserializeBounded>(value: &T) -> usize {
    let input = to_vec(value).unwrap();
    let mut budget = Budget::new(1 << 20);
    T::deserialize_bounded(&mut input.as_slice(), &mut budget).unwrap();
    budget.used()
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
- allocations made by a custom conversion from `T::Owned` into `Box<T>`;
- error values, reader buffers and allocator overhead.

# Caveats

- The formulas are unstable and may change in any release; a change can move an input's
  threshold in either direction.
- Charges are computed from `size_of` and the standard library's layouts, so they depend on
  the target and the compiler version; the same input can fit a limit with one build and
  exceed it with another. Use the limit to reject inputs locally, not where different
  machines must agree on whether an input is accepted.

# Writing an implementation by hand

Decode each part with its `deserialize_bounded`, and call
[`Budget::charge`](crate::de::Budget::charge) or
[`Budget::charge_array`](crate::de::Budget::charge_array) before an allocation you make
yourself. Use [`EnumExtBounded`](crate::de::EnumExtBounded) to dispatch on a tag you have
already read. An implementation must:

- pass on the budget it was given, and never create a `Budget` of its own;
- never let what it decodes depend on the budget's [`limit`](crate::de::Budget::limit),
  [`used`](crate::de::Budget::used) or [`remaining`](crate::de::Budget::remaining);
- decode the same way as its `BorshDeserialize` implementation, without depending on
  allocated capacity.

```rust
use borsh::de::Budget;
use borsh::io::{ErrorKind, Read, Result};
use borsh::BorshDeserializeBounded;

/// A name, as written and in lowercase.
#[derive(Debug)]
struct Name {
    original: String,
    lowercase: String,
}

impl BorshDeserializeBounded for Name {
    fn deserialize_bounded<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let original = String::deserialize_bounded(reader, budget)?;
        // `to_ascii_lowercase` allocates `original.len()` bytes.
        budget.charge(original.len())?;
        let lowercase = original.to_ascii_lowercase();
        Ok(Name { original, lowercase })
    }
}

let input = borsh::to_vec("Borsh").unwrap();
// 5 bytes for the string and 5 for its lowercase copy.
let name = borsh::from_slice_bounded::<Name>(&input, &mut Budget::new(10)).unwrap();
assert_eq!((name.original.as_str(), name.lowercase.as_str()), ("Borsh", "borsh"));
let err = borsh::from_slice_bounded::<Name>(&input, &mut Budget::new(9)).unwrap_err();
assert_eq!(err.kind(), ErrorKind::OutOfMemory);
```

The budget is passed separately from the reader, so a part can be decoded from any reader,
for example one built with `std::io::Read::chain` to put back a byte that was read ahead.

A type from another crate can only get an implementation in that crate or in borsh, as with
`BorshDeserialize`. Until it has one, wrap it in a type of your own and implement the trait
for the wrapper.
