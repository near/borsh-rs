Derive macro available if borsh is built with `features = ["derive", "unstable__bounded"]`.

# derive proc-macro for [`BorshDeserializeBounded`](trait@crate::BorshDeserializeBounded) trait

The derived implementation decodes the same bytes as `#[derive(BorshDeserialize)]`, field by
field and variant by variant, and passes the [`Budget`](crate::de::Budget) on to every field.
It honors the same `#[borsh(...)]` attributes, with bounded counterparts for the three that
name plain items: `init`, `deserialize_with` and `bound(deserialize = ...)`. What each type
charges is documented on the [`BorshDeserializeBounded`](trait@crate::BorshDeserializeBounded)
trait.

```rust
use borsh::{
    from_slice_bounded, to_vec, BorshDeserialize, BorshDeserializeBounded, BorshSerialize, Budget,
};

#[derive(BorshSerialize, BorshDeserialize, BorshDeserializeBounded, PartialEq, Debug)]
struct Message {
    id: u64,
    tags: Vec<String>,
}

let message = Message { id: 7, tags: vec!["a".to_string(), "bc".to_string()] };
let encoded = to_vec(&message).unwrap();
// The `Message` itself, two `String`s in the vector, then 1 and 2 bytes in them.
let charge = core::mem::size_of::<Message>() + 2 * core::mem::size_of::<String>() + 3;
assert_eq!(from_slice_bounded::<Message>(&encoded, &mut Budget::from_bytes(charge)).unwrap(), message);
assert!(from_slice_bounded::<Message>(&encoded, &mut Budget::from_bytes(charge - 1)).is_err());
```

Every field type must implement `BorshDeserializeBounded`, so a field that could only be
decoded without a limit is a compile error:

```rust,compile_fail
use borsh::{BorshDeserialize, BorshDeserializeBounded};

#[derive(BorshDeserialize)]
struct PlainOnly(Vec<u8>);

#[derive(BorshDeserializeBounded)]
struct A {
    x: PlainOnly,
}
```

```rust
use borsh::{BorshDeserialize, BorshDeserializeBounded};

#[derive(BorshDeserialize, BorshDeserializeBounded)]
struct Both(Vec<u8>);

#[derive(BorshDeserializeBounded)]
struct A {
    x: Both,
}
```

## Bounds

Generally, `BorshDeserializeBounded` adds `borsh::de::BorshDeserializeBounded` bound to any
type parameter found in item's fields and `core::default::Default` bound to any type parameter
found in item's skipped fields.

```rust
use borsh::BorshDeserializeBounded;

/// impl<U, V> borsh::de::BorshDeserializeBounded for A<U, V>
/// where
///     U: borsh::de::BorshDeserializeBounded,
///     V: borsh::de::BorshDeserializeBounded,
#[derive(BorshDeserializeBounded)]
struct A<U, V> {
    x: U,
    y: V,
}
```

```rust
use borsh::BorshDeserializeBounded;

/// impl<U, V> borsh::de::BorshDeserializeBounded for A<U, V>
/// where
///     U: borsh::de::BorshDeserializeBounded,
///     V: core::default::Default,
#[derive(BorshDeserializeBounded)]
struct A<U, V> {
    x: U,
    #[borsh(skip)]
    y: V,
}
```

## Attributes

They work as described for [`BorshDeserialize`](macro@crate::BorshDeserialize); the
differences are noted below. Multiple `#[borsh(...)]` attributes on the same item or field are
merged.

### 1. `#[borsh(crate = "path::to::borsh")]` (item level attribute)

The same as for `BorshDeserialize`.

(example is not tested, as there's usually no `reexporter` crate during doc build)
```rust,ignore
use reexporter::borsh::BorshDeserializeBounded;

#[derive(BorshDeserializeBounded)]
#[borsh(crate = "reexporter::borsh")]
struct B {
    x: u64,
    y: i32,
    c: String,
}
```

### 2. `#[borsh(init_bounded = ...)]` (item level attribute)

###### syntax

Attribute's value is a path to a method, as for `init`.

###### usage

The method runs right after decoding, in place of `init`, and gets the budget:

```rust,ignore
fn init_bounded(&mut self, budget: &mut borsh::Budget) -> borsh::io::Result<()>
```

It must charge the budget before each allocation the value keeps, fail only with an error from
the budget, and otherwise change the value as `init` does, so that both derives decode the same
value.

```rust
use borsh::{BorshDeserialize, BorshDeserializeBounded, Budget};

#[derive(BorshDeserialize, BorshDeserializeBounded)]
#[borsh(init = init, init_bounded = init_bounded)]
struct Name {
    original: String,
    #[borsh(skip)]
    lowercase: String,
}

impl Name {
    fn init(&mut self) {
        self.lowercase = self.original.to_ascii_lowercase();
    }

    fn init_bounded(&mut self, budget: &mut Budget) -> borsh::io::Result<()> {
        // `to_ascii_lowercase` allocates `original.len()` bytes.
        budget.charge_bytes(self.original.len())?;
        self.init();
        Ok(())
    }
}

let input = borsh::to_vec("Borsh").unwrap();
// The `Name` itself, 5 bytes for the string and 5 for its lowercase copy.
let limit = core::mem::size_of::<Name>() + 10;
let name = borsh::from_slice_bounded::<Name>(&input, &mut Budget::from_bytes(limit)).unwrap();
assert_eq!(name.lowercase, "borsh");
assert!(borsh::from_slice_bounded::<Name>(&input, &mut Budget::from_bytes(limit - 1)).is_err());
```

`init` alone is rejected by this derive, since its allocations would not be charged.
`init_bounded` alone is allowed, and `BorshDeserialize` ignores it.

```rust,compile_fail
use borsh::BorshDeserializeBounded;

#[derive(BorshDeserializeBounded)]
#[borsh(init = init)]
struct A {
    x: u64,
}

impl A {
    fn init(&mut self) {}
}
```

### 3. `borsh(use_discriminant=<bool>)` (item level attribute)

The same as for `BorshDeserialize`: the derived [`EnumExtBounded`](crate::de::EnumExtBounded)
implementation reads the same tags.

```rust
use borsh::BorshDeserializeBounded;

#[derive(BorshDeserializeBounded)]
#[borsh(use_discriminant = true)]
enum B {
    A,
    B = 10,
}
```

### 4. `#[borsh(skip)]` (field level attribute)

The field gets `Default::default()`, whose allocations are not charged, and its type
parameters get a `core::default::Default` bound instead of `borsh::de::BorshDeserializeBounded`.

```rust
use borsh::BorshDeserializeBounded;

#[derive(BorshDeserializeBounded)]
struct A {
    x: u64,
    #[borsh(skip)]
    y: f32,
}
```

### 5. `#[borsh(bound(deserialize_bounded = ...))]` (field level attribute)

###### syntax

Attribute takes literal string value, which is a comma-separated list of syn's
[WherePredicate](https://docs.rs/syn/latest/syn/enum.WherePredicate.html)-s, which may be
empty.

###### usage

It replaces the bounds `BorshDeserializeBounded` derives for the field, like
`bound(deserialize = ...)` does for `BorshDeserialize`. The two are independent: each derive
only reads its own key, so a field that overrides the plain bounds usually needs both.

```rust
use borsh::{BorshDeserialize, BorshDeserializeBounded};
#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;
use core::hash::Hash;

#[cfg(any(feature = "hashbrown", feature = "std"))]
#[derive(BorshDeserialize, BorshDeserializeBounded)]
struct A<T, U> {
    a: String,
    #[borsh(bound(
        deserialize =
        "T: Ord + Hash + Eq + borsh::de::BorshDeserialize,
         U: borsh::de::BorshDeserialize",
        deserialize_bounded =
        "T: Ord + Hash + Eq + borsh::de::BorshDeserializeBounded,
         U: borsh::de::BorshDeserializeBounded"
    ))]
    b: HashMap<T, U>,
}
```

### 6. `#[borsh(deserialize_with_bounded = ...)]` (field level attribute)

###### syntax

Attribute takes literal string value, which is a syn's
[ExprPath](https://docs.rs/syn/latest/syn/struct.ExprPath.html).

###### usage

The function decodes the annotated field. It has the signature of a `deserialize_with`
function plus the budget, and should charge what it allocates (see the trait documentation):

```rust,ignore
fn deserialize_field<R: borsh::io::Read>(
    reader: &mut R,
    budget: &mut borsh::de::Budget,
) -> borsh::io::Result<FieldType>
```

A field with `deserialize_with` must also have `deserialize_with_bounded`, so that the
bounded derive never decodes it without a limit. Both functions should read the same bytes.

```rust
use borsh::{BorshDeserialize, BorshDeserializeBounded};

/// this a stub module, representing a 3rd party crate
mod third_party {
    pub struct Blob(pub Vec<u8>);
}

mod blob_impl {
    use super::third_party::Blob;
    use borsh::{io, BorshDeserialize, BorshDeserializeBounded, Budget};

    pub fn deserialize<R: io::Read>(reader: &mut R) -> io::Result<Blob> {
        Ok(Blob(Vec::<u8>::deserialize_reader(reader)?))
    }

    pub fn deserialize_bounded<R: io::Read>(
        reader: &mut R,
        budget: &mut Budget,
    ) -> io::Result<Blob> {
        Ok(Blob(Vec::<u8>::deserialize_bounded_alloc(reader, budget)?))
    }
}

#[derive(BorshDeserialize, BorshDeserializeBounded)]
struct B {
    #[borsh(
        deserialize_with = "blob_impl::deserialize",
        deserialize_with_bounded = "blob_impl::deserialize_bounded",
    )]
    x: third_party::Blob,
    y: String,
}
# fn main() {
# }
```

`deserialize_with` alone is rejected by this derive, and accepted by `BorshDeserialize`:

```rust,compile_fail
use borsh::{BorshDeserialize, BorshDeserializeBounded};

mod blob_impl {
    use borsh::{io, BorshDeserialize};

    pub fn deserialize<R: io::Read>(reader: &mut R) -> io::Result<Vec<u8>> {
        Vec::<u8>::deserialize_reader(reader)
    }
}

#[derive(BorshDeserialize, BorshDeserializeBounded)]
struct B {
    #[borsh(deserialize_with = "blob_impl::deserialize")]
    x: Vec<u8>,
}
# fn main() {
# }
```

`deserialize_with_bounded` alone is allowed, for a field whose type has a plain
implementation but no bounded one.

###### interaction with `#[borsh(skip)]`

`#[borsh(deserialize_with_bounded = ...)]` is not allowed to be used simultaneously with
`#[borsh(skip)]`, in any derive:

```rust,compile_fail
use borsh::BorshDeserializeBounded;

mod blob_impl {
    use borsh::{io, BorshDeserializeBounded, Budget};

    pub fn deserialize_bounded<R: io::Read>(
        reader: &mut R,
        budget: &mut Budget,
    ) -> io::Result<Vec<u8>> {
        Vec::<u8>::deserialize_bounded_alloc(reader, budget)
    }
}

#[derive(BorshDeserializeBounded)]
struct B {
    #[borsh(skip, deserialize_with_bounded = "blob_impl::deserialize_bounded")]
    x: Vec<u8>,
}
# fn main() {
# }
```

```rust
use borsh::BorshDeserializeBounded;

mod blob_impl {
    use borsh::{io, BorshDeserializeBounded, Budget};

    pub fn deserialize_bounded<R: io::Read>(
        reader: &mut R,
        budget: &mut Budget,
    ) -> io::Result<Vec<u8>> {
        Vec::<u8>::deserialize_bounded_alloc(reader, budget)
    }
}

#[derive(BorshDeserializeBounded)]
struct B {
    #[borsh(deserialize_with_bounded = "blob_impl::deserialize_bounded")]
    x: Vec<u8>,
}
# fn main() {
# }
```

## Unions

Unions are not supported, as with `BorshDeserialize`.

## Adding bounded decoding to existing types

1. Add `BorshDeserializeBounded` next to `BorshDeserialize` on every type reachable from the
   values you decode with a limit. The compiler points at each field type that is missing it.
2. For each `deserialize_with = "f"`, write a bounded `f` that takes the budget, and add
   `deserialize_with_bounded`.
3. For each `init = f`, write a bounded `f` that takes the budget, and add `init_bounded`.
4. For each `bound(deserialize = "...")`, add the matching `deserialize_bounded = "..."`.
5. Write a bounded twin of each hand-written `BorshDeserialize` implementation; see
   [`BorshDeserializeBounded`](trait@crate::BorshDeserializeBounded) for how.
6. Decode with [`from_slice_bounded`] or [`from_reader_bounded`].
