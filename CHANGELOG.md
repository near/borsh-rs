# Changelog

## 0.9.1
- Eliminated unsafe code from both ser and de of u8 (#26)
- Implemented ser/de for reference count types (#27)
- Added serialization helpers to improve api ergonomics (#34)
- Implemented schema for arrays and fix box bounds (#36)
- Implemented (de)ser for PhantomData (#37)
- Implemented const-generics under feature (#38)
- Added an example of direct BorshSerialize::serialize usage with vector and slice buffers (#29)

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

