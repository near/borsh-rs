A data-structure that can be deserialized from Borsh, like
[`BorshDeserialize`](crate::BorshDeserialize), while charging a memory budget.

Use it to decode untrusted input with a memory limit. The encoding is the same as for
`BorshDeserialize`. Each decode gets a [`Budget`](crate::de::Budget), and every
implementation charges it before it allocates. A charge that does not fit fails the decode
with an error of kind `OutOfMemory` (see [`LimitExceeded`](crate::error::LimitExceeded)).

```rust
use borsh::{from_slice_bounded, to_vec, Budget};
use core::mem::size_of;

let encoded = to_vec(&vec![String::from("abc"); 3]).unwrap();
// The `Vec` itself, its buffer of three `String`s, and 3 bytes for each string.
let limit = size_of::<Vec<String>>() + 3 * size_of::<String>() + 3 * 3;
let decoded = from_slice_bounded::<Vec<String>>(&encoded, &mut Budget::from_bytes(limit)).unwrap();
assert_eq!(decoded, vec!["abc"; 3]);
assert!(from_slice_bounded::<Vec<String>>(&encoded, &mut Budget::from_bytes(limit - 1)).is_err());
```

The trait is implemented for the same types as `BorshDeserialize`, except those of the
`ascii`, `bytes`, `bson`, `indexmap`, `uuid` and `unstable__schema` features. A type without
an implementation cannot be decoded bounded: that is a compile error, never a silent
fallback to plain decoding. `BorshDeserialize` is not a supertrait.

# How charging works

- A charge is never refunded.
- The limit caps the total of all charges made against one budget, not the peak memory:
  copying a growing buffer can briefly need almost twice the charged bytes. Temporaries are
  charged too, such as the vector a map, set or list is built from.
- A collection charges for all its elements as soon as it has read its length prefix, so a
  length that cannot fit fails before any element is read.
- Charges only depend on the input, never on how much of the budget is left.

# Allocation

- **Grown, ending exact.** A buffer starts small, as with plain decoding, and grows as
  elements arrive, ending at exactly the charged length. A decoded `Vec`, `VecDeque` or
  `String` therefore has its capacity equal to its length, where plain decoding may leave
  more capacity. The values are equal either way.
- **A false length costs no more than plain decoding.** A length prefix within the budget
  that the input does not back allocates no more than plain decoding does before decoding
  fails as truncated input.
- **Allocation failure aborts**, as with plain decoding.

# Guarantees

- Unless it fails with `LimitExceeded`, a bounded decode returns what `BorshDeserialize`
  returns: the same value, or an error of the same kind and message.
- After a successful decode with a fresh budget,
  [`used_bytes()`](crate::de::Budget::used_bytes) is the smallest limit that decodes this input.

Both assume that the reader is deterministic, that hand-written implementations follow the
[rules](#writing-an-implementation-by-hand), and that allocations succeed.

# What is charged

A successful `from_slice_bounded` or `from_reader_bounded` call charges `size_of::<T>()`
plus the allocation charges below.

`n` is the number of elements read from the length prefix, `p` is `size_of::<usize>()`, and
`E` is `(K, V)` for maps and `T` for sets. Collections charge nothing for `n = 0`; an `Rc` or
`Arc` of an empty slice or string still charges its block. A charge is exact when it equals
the final size of the allocations it covers, temporaries included.

| Type | Allocation charge | Exactness |
|---|---|---|
| integers, `f32`, `f64`, `bool`, `NonZero*`, `()`, `RangeFull`, `PhantomData`, `core::net` types | 0 | exact |
| `Option`, `Result`, tuples, ranges, `Cell`, `RefCell`, `[T; N]` | 0, plus what their contents charge | exact |
| `Vec<T>`, `VecDeque<T>`, `String`, `Box<[T]>`, `Box<str>`, `Cow<[T]>`, `Cow<str>` | `n·size_of::<T>()` (1 byte per element of strings) | exact |
| `Box<T>` of a sized `T` | `size_of::<T::Owned>()`, before `T` is decoded | exact for the standard library's conversions |
| `Rc<T>`, `Arc<T>` (feature `rc`) | what `Box<T>` charges, plus `rc_block` of the value, after it is decoded | exact |
| `LinkedList<T>` | `n·size_of::<T>() + n·size_of::<(usize, usize, T)>()` | exact under current rustc |
| `HashMap<K, V, S>`, `HashSet<T, S>` | `n·size_of::<E>() + table(E, n)` | upper bound |
| `BTreeMap<K, V>`, `BTreeSet<T>` | `n·size_of::<E>() + sort(E, n) + nodes(K, V, n)` | upper bound |

With the formulas:

- `table(E, n) = b·(size_of::<E>() + 1) + 16`, with
  `b = max(16, (8n/7).next_power_of_two())`. It is exact on x86 with SSE2 when `n ≥ 8`.
- `sort(E, n)` is the scratch memory of the stable sort: 0 for `n < 2`, `(n/2)·size_of::<E>()`
  for `n ≤ 20`, and `max(n, 48)·size_of::<E>() + 224·p` above.
- `nodes(K, V, n) = L·leaf + I·internal`, with `L = n/12 + 1` leaves of
  `leaf = size_of::<(usize, u16, u16, [K; 11], [V; 11])>()` bytes (`V = ()` for sets), and
  `I` internal nodes of `leaf + 12·p` bytes padded to alignment. `I` is the sum of
  `ceil(x/12)` over the levels above the leaves, starting from `x = L`. Duplicate keys build
  fewer nodes.
- `rc_block(value)` is two reference counts (`usize` for `Rc`, `AtomicUsize` for `Arc`)
  followed by the value, padded to the alignment of both.

Worked examples on a 64-bit target:

```rust
use borsh::{from_slice_bounded, to_vec, BorshDeserializeBounded, BorshSerialize, Budget};
use std::collections::{BTreeMap, LinkedList};

fn charged<T: BorshSerialize + BorshDeserializeBounded>(value: &T) -> usize {
    let input = to_vec(value).unwrap();
    let mut budget = Budget::from_bytes(1_000_000);
    from_slice_bounded::<T>(&input, &mut budget).unwrap();
    budget.used_bytes()
}

# #[cfg(target_pointer_width = "64")]
# {
assert_eq!(charged(&vec![0u64; 1000]), 8000 + 24);
assert_eq!(charged(&(0..100u64).collect::<LinkedList<_>>()), 3200 + 24);
let pairs = || (0..1000u64).map(|k| (k, k));
assert_eq!(charged(&pairs().collect::<BTreeMap<_, _>>()), 52224 + 24);
# #[cfg(feature = "std")]
assert_eq!(charged(&pairs().collect::<std::collections::HashMap<_, _>>()), 50832 + 48);
# #[cfg(feature = "rc")]
# {
assert_eq!(charged(&std::rc::Rc::new(7u8)), 1 + 24 + 8);
assert_eq!(charged(&std::sync::Arc::<str>::from("a".repeat(5001))), 5001 + 5024 + 16);
# }
# }
```

# What is not charged

- stack used while decoding (temporaries, recursion depth);
- allocations of `#[borsh(init)]` functions and of `Default` values of skipped fields;
- allocations inside hand-written implementations, unless they charge for them or decode
  their parts with `deserialize_bounded_alloc`; plain `BorshDeserialize` calls made from
  there are not charged either;
- allocations made by a custom conversion from `T::Owned` into `Box<T>`;
- allocations of the hasher (`S::default()`) and of user `Hash`, `Eq`, `Ord` and `Drop`
  implementations;
- error values, reader buffers and allocator overhead.

# Caveats

- The `LinkedList`, `BTreeMap` and hash table formulas mirror private standard library and
  hashbrown internals, checked only on the toolchains and versions borsh is tested with.
- The formulas are unstable and may change in any release; a change can move an input's
  threshold in either direction.
- Charges are computed from `size_of`, so they depend on the target and on compiler and
  dependency versions; the same input can fit a limit with one build and exceed it with
  another. Use the limit to reject inputs locally, not where different machines must agree
  on whether an input is accepted.

# Writing an implementation by hand

Decode each part with its `deserialize_bounded_alloc`, and call
[`Budget::charge_bytes`](crate::de::Budget::charge_bytes) or
[`Budget::charge_array`](crate::de::Budget::charge_array) before an allocation you make
yourself. Use [`EnumExtBounded`](crate::de::EnumExtBounded) to dispatch on a tag you have
already read. An implementation must:

- pass on the budget it was given, and never create a `Budget` of its own;
- never let what it decodes depend on the budget's
  [`limit_bytes`](crate::de::Budget::limit_bytes), [`used_bytes`](crate::de::Budget::used_bytes)
  or [`remaining_bytes`](crate::de::Budget::remaining_bytes);
- decode the same way as its `BorshDeserialize` implementation, without depending on
  allocated capacity.

```rust
use borsh::io::{ErrorKind, Read, Result};
use borsh::{BorshDeserializeBounded, Budget};

/// A name, as written and in lowercase.
#[derive(Debug)]
struct Name {
    original: String,
    lowercase: String,
}

impl BorshDeserializeBounded for Name {
    fn deserialize_bounded_alloc<R: Read>(reader: &mut R, budget: &mut Budget) -> Result<Self> {
        let original = String::deserialize_bounded_alloc(reader, budget)?;
        // `to_ascii_lowercase` allocates `original.len()` bytes.
        budget.charge_bytes(original.len())?;
        let lowercase = original.to_ascii_lowercase();
        Ok(Name { original, lowercase })
    }
}

let input = borsh::to_vec("Borsh").unwrap();
// The `Name` itself, 5 bytes for the string and 5 for its lowercase copy.
let limit = core::mem::size_of::<Name>() + 10;
let name = borsh::from_slice_bounded::<Name>(&input, &mut Budget::from_bytes(limit)).unwrap();
assert_eq!((name.original.as_str(), name.lowercase.as_str()), ("Borsh", "borsh"));
let err =
    borsh::from_slice_bounded::<Name>(&input, &mut Budget::from_bytes(limit - 1)).unwrap_err();
assert_eq!(err.kind(), ErrorKind::OutOfMemory);
```

The budget is passed separately from the reader, so a part can be decoded from any reader,
for example one built with `std::io::Read::chain` to put back a byte that was read ahead.

A type from another crate can only get an implementation in that crate or in borsh, as with
`BorshDeserialize`. Until it has one, wrap it in a type of your own and implement the trait
for the wrapper.
