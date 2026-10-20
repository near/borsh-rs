# Decoding with a memory limit

Available if borsh is built with `features = ["unstable__bounded"]`. This API is unstable
and not covered by semver.

A decoded value can need much more memory than its encoding: a length prefix promises a
large collection, and boxes, maps and many small vectors all allocate. To decode untrusted
input with a memory limit, use [`from_slice_bounded`] or [`from_reader_bounded`] with a type
that implements [`BorshDeserializeBounded`] and a [`de::Budget`] created with the limit. The
encoding is unchanged; every allocation is charged against the budget first, and a decode
that would exceed the limit fails with [`error::LimitExceeded`]. What is charged is
documented on [`BorshDeserializeBounded`].

```rust
use borsh::de::Budget;
use borsh::{from_slice_bounded, to_vec};

let encoded = to_vec(&vec![7u64; 1000]).unwrap();
assert!(from_slice_bounded::<Vec<u64>>(&encoded, &mut Budget::new(8000)).is_ok());
let err = from_slice_bounded::<Vec<u64>>(&encoded, &mut Budget::new(7999)).unwrap_err();
assert_eq!(err.kind(), borsh::io::ErrorKind::OutOfMemory);
```
