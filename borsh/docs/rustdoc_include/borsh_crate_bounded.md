# Decoding with a memory limit

Available if borsh is built with `features = ["unstable__bounded"]`; this API is unstable
and not covered by semver. [`from_slice_bounded`] and [`from_reader_bounded`] decode a type
that implements [`BorshDeserializeBounded`] from the usual encoding, charging a
[`de::Budget`] before every allocation. A decode that would exceed the limit fails with an
error of kind `OutOfMemory` (see [`error::LimitExceeded`]).
