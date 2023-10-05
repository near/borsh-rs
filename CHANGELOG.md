# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0](https://github.com/near/borsh-rs/compare/borsh-v0.10.3...borsh-v1.0.0) - 2023-10-03

> The year is 2653 and the best yet-to-be citizens of the Terran Federation are fighting 
> and mostly just dying in a relentless interstellar war against the Arachnids.
> Yet the structure of our society has changed through the course of this confrontation. 
> 
> The members of the Arachnid brain caste and queens have infiltrated the circles of our 
> most influential political and industrial leaders. Either directly, or via the Arachnid technology
> called "Brain Bugs". This tech alone can accomplish what the Arachnid starship paratroopers
> will not ever be capable to do.
>
> Simple, straightforward and performant serialization libraries can set us in course to remedy this dangerous
> stalemate situation by cleaning the minds of its users from even the tiniest of Brain Bugs.

Robert A. Heinlein, 1959 (a newspaper ad)
---

### [Thanks]

`borsh-rs` `1.0.0` release was first conceived and then brought into existence by minds of:

- Amirhossein Akhlaghpour @Mehrbod2002
- Benji Smith @Benjins
- dj8yf0μl @dj8yfo
- iho @iho
- Jacob Lindahl @encody
- Pavel Lazureykis @lazureykis
- Tomas Zemanovic @tzemanovic

Contributors, who imposed powerful impact on the past, present and future of this library are specially recognized:

- Michal Nazarewicz @mina86 - for revisiting `BorshSchema` feature, rethinking it, bringing up great ideas and coming up with the
  fairly involved algorithm of `max_serialized_size` implementation.
- Alex Kladov @matklad - for maintaining a superhuman ability of context switching in under 2 minutes and scanning through 15k lines of code
  in under 10 minutes, while leaving out under 1% relevant details.   
- Marco Ieni @MarcoIeni - for developing [release-plz](https://github.com/MarcoIeni/release-plz) automation.
- Vlad Frolov @frol - for keeping an eye on the big picture and striking just the right balance between 
  performance and versatility, ease of use and extensibility and tons of other such hard to reconcile pairs.   

### [Migration guides]

This section contains links to short documents, describing problems encountered during update of `borsh`
version to `v1.0.0` for related repositories.

- [v0.10.2 -> v1.0.0 for `nearcore`](./docs/migration_guides/v0.10.2_to_v1.0.0_nearcore.md)
- [v0.9.3 -> v1.0.0 for `near-sdk-rs`](./docs/migration_guides/v0.9_to_v1.0.0_near_sdk_rs.md)

### [Summary of changes]

- Library's structure was made more modular and optimized with respect to visibility
  of its public/private constituents and ease of access to them.
- `borsh`'s traits derives and their attributes had their capabilities extended and unified,
  both with respect to external interfaces and internal implementation. Please visit [borsh_derive](https://docs.rs/borsh-derive/1.0.0/borsh_derive/)
  documentation pages if you're interested in more of the details.
- The consistency property of deserialization, declared in [Borsh Specification](https://borsh.io/), became an
  opt-in feature for hash collections.
- Support of explicit enum discriminants was added to derives of `borsh` traits. 
  It has been added in somewhat limited form, only allowing the values of `u8` range literals.

  ```rust
  use borsh::{BorshSerialize, BorshDeserialize, BorshSchema};

  <<<<<<< borsh-v0.10.3
  #[derive(BorshDeserialize, BorshSerialize, BorshSchema)]
  pub enum CurveType {
      ED25519 = 0, // 0 as u8 in enum tag
      SECP256K1 = 2, // 1 as u8 in enum tag
  }
  =======
  #[derive(BorshDeserialize, BorshSerialize, BorshSchema)]
  #[borsh(use_discriminant=false)]
  pub enum CurveType {
      ED25519 = 0, // 0 as u8 in enum tag
      SECP256K1 = 2, // 1 as u8 in enum tag
  }
  // vs
  #[derive(BorshDeserialize, BorshSerialize, BorshSchema)]
  #[borsh(use_discriminant=true)]
  pub enum CurveType {
      ED25519 = 0, // 0 as u8 in enum tag
      SECP256K1 = 2, // 2 as u8 in enum tag
  }
  >>>>>>> borsh-v1.0.0
  ```
- [RUSTSEC-2023-0033](https://rustsec.org/advisories/RUSTSEC-2023-0033.html) has been resolved.
  It has been resolved by forbidding collections with dynamic runtime length to contain zero-sized types
  with runtime errors, happening on serialization or deserialization.
  Arrays with non-`Copy` and non-`Clone` ZST singletons of length > 1 gracefully panic on deserialization,
  not causing memory faults. 
  
  Using collections with dynamic runtime length for containing ZSTs was also deemed
  wasteful of CPU cycles and a way to perform dos attacks.
  Such a case is now flagged as error when using new `BorshSchemaContainer::validate` method for user-defined
  types or instantiations of `BorshSchema`-supporting types with inappropriate parameters, defined by the library:

  ```rust
  let schema = BorshSchemaContainer::for_type::<Vec<core::ops::RangeFull>>();
  assert_eq!(
      Err(
        SchemaContainerValidateError::ZSTSequence("Vec<RangeFull>".to_string())
      ), 
      schema.validate()
  );
  ```
- `BorshSchema` was extended with `max_serialized_size` implementation, which now unlocks support of `borsh`
  by a plethora of bounded types to express statically defined size limits of serialized representation of these types.  
- schema `BorshSchemaContainer` api was made future-proof.
- schema `Definition` was extended with more variants, fields and details to uncover some of the 
  implied details of serialization format.
  `BorshSchema` can now express a wider range of types. All types, which have `BorshSchema` defined by the library,
  now have a `Definition`.
- schema `Declaration`-s were renamed to follow Rust-first rule and not be a mix of Rust types naming/syntax and syntax
  from other languages.

  ```rust
  use borsh::schema::BorshSchema;

  <<<<<<< borsh-v0.10.3
  assert_eq!("nil", <()>::declaration());
  assert_eq!("string", <String>::declaration());
  assert_eq!("Array<u64, 42>", <[u64; 42]>::declaration());
  assert_eq!("Tuple<u8, bool, f32>", <(u8, bool, f32)>::declaration());
  =======
  assert_eq!("()", <()>::declaration());
  assert_eq!("String", <String>::declaration());
  assert_eq!("[u64; 42]", <[u64; 42]>::declaration());
  assert_eq!("(u8, bool, f32)", <(u8, bool, f32)>::declaration());
  >>>>>>> borsh-v1.0.0
  ```

### [Stability guarantee]

- `borsh`'s serialization format is guaranteed to NOT change throughout 1.x releases.
- `borsh`'s public APIs not gated by `unstable__schema` feature are guaranteed to NOT break
 throughout 1.x releases.
- It's perceived, that new feature requests may potentially come for `BorshSchema` from outside of `near` ecosystem,
thus `borsh`'s public APIs gated by `unstable__schema` MAY break throughout 1.x releases.

 
## [1.0.0-alpha.6](https://github.com/near/borsh-rs/compare/borsh-v1.0.0-alpha.5...borsh-v1.0.0-alpha.6) - 2023-10-02

### Added
- add `borsh::object_length` helper ([#236](https://github.com/near/borsh-rs/pull/236))

### Other
- add examples for `borsh::to_vec`, `borsh::to_writer`, `borsh::object_length` ([#238](https://github.com/near/borsh-rs/pull/238))
- [**breaking**] completely remove deprecated `BorshSerialize::try_to_vec` ([#221](https://github.com/near/borsh-rs/pull/221))

## [1.0.0-alpha.5](https://github.com/near/borsh-rs/compare/borsh-v1.0.0-alpha.4...borsh-v1.0.0-alpha.5) - 2023-09-26

### Added
- [**breaking**] add `DiscriminantValue` to `Definition::Enum::variants` tuples ([#232](https://github.com/near/borsh-rs/pull/232))
- [**breaking**] add `length_width` to `schema::Definition::Sequence` ([#229](https://github.com/near/borsh-rs/pull/229))
- add definition of `String`/`str` ([#226](https://github.com/near/borsh-rs/pull/226))
- [**breaking**] add `Definition::Sequence::length_range` field ([#220](https://github.com/near/borsh-rs/pull/220))
- [**breaking**] add `Definition::Primitive` ([#222](https://github.com/near/borsh-rs/pull/222))
- max_size: various small refactoring ([#223](https://github.com/near/borsh-rs/pull/223))
- check `Definition::Enum`’s `tag_width` when validating schema ([#224](https://github.com/near/borsh-rs/pull/224))
- add (de)serialisation + schema for more `core::ops::Range...` types (full, open-ended, inclusive) ([#213](https://github.com/near/borsh-rs/pull/213))
- add `BorshSchema` implementation for `core::num::NonZero...` integers ([#214](https://github.com/near/borsh-rs/pull/214))
- [**breaking**] introduce `borsh::io` with either items of `std:io` or private `borsh::nostd_io` module reexported (`std` or `no_std`) ([#212](https://github.com/near/borsh-rs/pull/212))
- Introduce `borsh::max_serialized_size` function, `borsh::schema::BorshSchemaContainer::for_type` method ([#209](https://github.com/near/borsh-rs/pull/209))

### Other
- [**breaking**] rename `"Tuple<T0, T1, T2...>"` -> `"(T0, T1, T2...)"` (`schema::Declaration`) ([#234](https://github.com/near/borsh-rs/pull/234))
- [**breaking**] rename `"nil"` -> `"()"`, `"string"` -> `"String"`, `"nonzero_u16"` -> `"NonZeroU16"` (`schema::Declaration`) ([#233](https://github.com/near/borsh-rs/pull/233))
- [**breaking**] rename `"Array<T0, N>"` -> `"[T0; N]"` (`schema::Declaration`) ([#235](https://github.com/near/borsh-rs/pull/235))
- [**breaking**] split `ValidationError` from `MaxSizeError`; `validate` and `max_serialized_size` made `BorshSchemaContainer`'s methods ([#219](https://github.com/near/borsh-rs/pull/219))
- [**breaking**] declare and rename schema feature to be unstable__ (may break in 1.x versions)
- Add Definition::Enum::tag_width field ([#215](https://github.com/near/borsh-rs/pull/215))

## [1.0.0-alpha.4](https://github.com/near/borsh-rs/compare/borsh-v1.0.0-alpha.3...borsh-v1.0.0-alpha.4) - 2023-09-04

### Added
- [**breaking**] raise bound on keys in hashcollections `PartialOrd` -> `Ord` ([#203](https://github.com/near/borsh-rs/pull/203))
- forbid most collections from containing zst elements/keys ([#202](https://github.com/near/borsh-rs/pull/202))
- add `#[borsh(crate = ...)]` item-level attribute ([#210](https://github.com/near/borsh-rs/pull/210))
- forbid multiple `borsh` attr occurencies ([#199](https://github.com/near/borsh-rs/pull/199))

### Other
- various flaws correction ([#205](https://github.com/near/borsh-rs/pull/205))
- [**breaking**] deprecate `try_to_vec` method from `BorshSerialize` ([#206](https://github.com/near/borsh-rs/pull/206))
- [**breaking**] make `BorshSchema::add_definition` default implementation a free-standing func ([#204](https://github.com/near/borsh-rs/pull/204))
- remove `#[non_exhaustive]` on `borsh::schema::Definition` ([#200](https://github.com/near/borsh-rs/pull/200))

## [1.0.0-alpha.3](https://github.com/near/borsh-rs/compare/borsh-v1.0.0-alpha.2...borsh-v1.0.0-alpha.3) - 2023-08-16

### Other
- [**breaking**] renamed #[borsh_skip] to #[borsh(skip)] ([#192](https://github.com/near/borsh-rs/pull/192))
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
