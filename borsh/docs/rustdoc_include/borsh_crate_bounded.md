# Decoding with a memory limit

Available if borsh is built with `features = ["unstable__bounded"]`. This API is unstable
and not covered by semver.

A decoded value can need much more memory than its encoding: a length prefix promises a
large collection, and boxes, maps and many small vectors all allocate. To decode untrusted
input with a memory limit, use [`from_slice_bounded`] or [`from_reader_bounded`] with a type
that implements [`BorshDeserializeBounded`]. The encoding is unchanged; every allocation is
charged against the limit first, and a decode that would exceed it fails with
[`error::LimitExceeded`]. What is charged is documented on [`BorshDeserializeBounded`].

```rust
use borsh::{from_slice_bounded, to_vec};

let encoded = to_vec(&vec![7u64; 1000]).unwrap();
assert!(from_slice_bounded::<Vec<u64>>(&encoded, 8000).is_ok());
let err = from_slice_bounded::<Vec<u64>>(&encoded, 7999).unwrap_err();
assert_eq!(err.kind(), borsh::io::ErrorKind::OutOfMemory);
```
