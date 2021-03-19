# Changelog

## vNext

- *BREAKING CHANGE*: `is_u8` method is removed from the public API.
  Consumers of the API don't need to call or override it, and doing
  so might cause undefined behavior.

## 0.9.0
- *BREAKING CHANGE*: `is_u8` optimization helper is now unsafe since it may
  cause undefined behavior if it returns `true` for the type that is not safe
  to Copy (#21)
- Extended the schema impls to support longer arrays to match the
  de/serialization impls (#22)

## 0.8.2
- Avoid collisions of imports due to derive-generated code (#14)

## 0.8.1
- Added support for BTreeMap, BTreeSet, BinaryHeap, LinkedList, and VecDeque

## 0.8.0
- Add no_std support.

## 0.7.2
- Implement `BorshSerialize` for reference fields (`&T`)

## 0.7.1
- Implement `BorshSerialize` for `&T` if `T` implements `BorshSerialize`.

## 0.7.0

- Extended `Box<T>` implementation for `?Sized` types (`[T]`, `str`, ...).
- Added support for `std::borrow::Cow`
- Avoid silent integer casts since they can lead to hidden security issues.
- Removed `Cargo.lock` as it is advised for lib crates.
