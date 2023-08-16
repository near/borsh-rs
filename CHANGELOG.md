# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-alpha.3](https://github.com/near/borsh-rs/compare/borsh-v1.0.0-alpha.2...borsh-v1.0.0-alpha.3) - 2023-08-16

### Other
- update dependencies
- split up schema derive functions ([#191](https://github.com/near/borsh-rs/pull/191))

## [1.0.0-alpha.2](https://github.com/near/borsh-rs/compare/borsh-v1.0.0-alpha.1...borsh-v1.0.0-alpha.2) - 2023-08-10

### Other
- [**breaking**] borsh_init to borsh(init).  ([#187](https://github.com/near/borsh-rs/pull/187))

## [1.0.0-alpha.1](https://github.com/near/borsh-rs/compare/borsh-v0.11.0...borsh-v1.0.0-alpha.1) - 2023-08-07

### Bug Fixes

- Unused fields warn, fields for inner structs of derived BorshSchema method (#172)
- #[borsh_skip] on field of struct enum variant (BorshSerialize) (#174)
- Filter out foreign attributes in `BorshSchema` derive for enum (#177)

### Documentation

- Create a brief documentation of crate's features (#159)
- Mention `schema` feature in doc.rs (#166)

### Features

- Forbid Vectors of Zero-sized types from de-/serialization to resolve the RUSTSEC-2023-0033 (#145)
- Add top-level `from_slice` and `from_reader` helper functions to make the API nicer (#142)
- [**breaking**] Add `#[borsh(use_discriminant = <bool>)]` attribute that changes enum discriminant de- and serialization behavior
- [**breaking**] Remove `BinaryHeap` support (#161)
- Sets/maps benches for reference point (#164)
- Enforce canonicity on `HashSet/BTreeSet/HashMap/BTreeMap` (#162)
- [**breaking**] Support recursive structures! (#178)
  - `BorshSerialize`, `BorshDeserialize`, `BorshSchema` derives may break
  - derives may require patching bounds with `#[borsh(bound(..))]` / `#[borsh(schema(params = ...))]`
- Bounds for ser/de derive and schema_params for schema derive attributes (#180)
- Derive attribute for 3rd party structs/enums as fields (#182)

### Miscellaneous Tasks

- Bump proc-macro-crate versions  (#149)
- Add tests job for MSRV (1.65.0) (#151)
- [**breaking**] Hide maybestd from public interface, despite it being technically available by new name of __maybestd (#153)
- Fix broken reference-style link in minimum supported version badge (#154)
- Remove a bunch of clippy-related TODOs (uninlined_format_args) (#156)
- Simpler bounds on Rc/Arc impls (#167)
- Invited @dj8yfo to CODEOWNERS (#169)
- [**breaking**] Replace ErrorKind::InvalidInput with ErrorKind::InvalidData as per original std::io meaning (#170)

### Refactor

- [**breaking**] Make `hashbrown` dependency optional, `hashbrown` feature (#155)
- [**breaking**] `BorshSchemaContainer` fields non-pub, `HashMap` -> `BTreeMap` in schema everywhere (#165)
- [**breaking**] Move derive under #[cfg(feature = "derive")] (#168)
- Introduce `__private` module with macro runtime (#171)
- [**breaking**] Unsplit and removal of *-internal crates (#185)
  - `borsh-schema-derive-internal` and `borsh-derive-internal` crates won't be published anymore

### Testing

- Add `insta` snapshots to borsh/tests (#157)
- `insta` tests for prettified `TokenStream`-s in `borsh*derive-internal` (#176)

### Ci

- Only release-plz after other checks pass

## [0.11.0](https://github.com/near/borsh-rs/compare/borsh-v0.10.3...borsh-v0.11.0) - 2023-05-31

### Added
- add BorshSchema for PhantomData, BTreeMap and BTreeSet ([#93](https://github.com/near/borsh-rs/pull/93))
- Add optional bson::oid::ObjectId support ([#135](https://github.com/near/borsh-rs/pull/135))
- [**breaking**] ser/de enum discriminant ([#138](https://github.com/near/borsh-rs/pull/138))

### Fixed
- no-std tests did not run due to dev-dependencies re-enabling std feature ([#144](https://github.com/near/borsh-rs/pull/144))

### Other
- use release-plz and specify common rust version correctly ([#134](https://github.com/near/borsh-rs/pull/134))
- Upgrade plain-HTTP links to HTTPS in Cargo.toml files ([#141](https://github.com/near/borsh-rs/pull/141))

## [0.10.3] - 2023-03-22

- Add optional bytes/bytesmut support

## [0.10.2] - 2023-02-14

- Prevent unbound allocation for vectors on deserialization

## [0.10.1] - 2023-02-08

- Implemented (de)ser for `core::ops::range`
- Introduce de::EnumExt trait with deserialize_variant method

## [0.10.0] - 2023-01-19

- Fix no-std feature (some of the imports incorrectly used `std::` instead of `crate::maybestd::`)
- Fix borsh-schema derives with `for` bounds
- Implemented BorshSchema for HashSet
- Add support for isize, usize types
- Delete schema for char
- Implement ser/de and schema for (T,)
- Add clone impls to borsh schema types
- Remove unnecessary trait bounds requirements for array
- *BREAKING CHANGE*: `BorshDeserialize` now works by receiving an `&mut std::io::Read`
  instead of a `&mut &[u8]`. This is a breaking change for code that provides custom
  implementations of `BorshDeserialize`; there is no impact on code that uses only the
  derive macro.
- Added `BorshDeserialize::try_from_reader` and `BorshDeserialize::deserialize_reader`.
- Upgrade hashbrown version to be `>=0.11,<0.14` to allow wider range of versions.

## [0.9.3] - 2022-02-03

- Fix `no_std` compatibility.
- Reduce code bloat in derived `BorshSerialize` impl for enums.

## [0.9.2] - 2022-01-25

- Upgrade hashbrown from `0.9` to `0.11`. This can breakage in the rare case
  that you use borsh schema together with no-std support and rely on a specific
  version hashbrown of `SchemaContainer`. This is considered to be obscure
  enough to not warrant a semver bump.

## [0.9.1] - 2021-07-14

- Eliminated unsafe code from both ser and de of u8 (#26)
- Implemented ser/de for reference count types (#27)
- Added serialization helpers to improve api ergonomics (#34)
- Implemented schema for arrays and fix box bounds (#36)
- Implemented (de)ser for PhantomData (#37)
- Implemented const-generics under feature (#38)
- Added an example of direct BorshSerialize::serialize usage with vector and slice buffers (#29)

## [0.9.0] - 2021-03-18

- *BREAKING CHANGE*: `is_u8` optimization helper is now unsafe since it may
  cause undefined behavior if it returns `true` for the type that is not safe
  to Copy (#21)
- Extended the schema impls to support longer arrays to match the
  de/serialization impls (#22)

## [0.8.2] - 2021-03-04

- Avoid collisions of imports due to derive-generated code (#14)

## [0.8.1] - 2021-01-13

- Added support for BTreeMap, BTreeSet, BinaryHeap, LinkedList, and VecDeque

## [0.8.0] - 2021-01-11

- Add no_std support.

## [0.7.2] - 2021-01-14

- Implement `BorshSerialize` for reference fields (`&T`)

## 0.7.1 - 2020-08-24

- Implement `BorshSerialize` for `&T` if `T` implements `BorshSerialize`.

## 0.7.0 - 2020-06-17

- Extended `Box<T>` implementation for `?Sized` types (`[T]`, `str`, ...).
- Added support for `std::borrow::Cow`
- Avoid silent integer casts since they can lead to hidden security issues.
- Removed `Cargo.lock` as it is advised for lib crates.

[unreleased]: https://github.com/near/borsh-rs/compare/v0.9.3...HEAD
[0.9.3]: https://github.com/near/borsh-rs/compare/v0.9.2...v0.9.3
[0.9.2]: https://github.com/near/borsh-rs/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/near/borsh-rs/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/near/borsh-rs/compare/v0.8.2...v0.9.0
[0.8.2]: https://github.com/near/borsh-rs/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/near/borsh-rs/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/near/borsh-rs/compare/v0.7.2...v0.8.0
[0.7.2]: https://github.com/near/borsh-rs/releases/tag/v0.7.2
