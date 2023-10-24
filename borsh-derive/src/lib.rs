#![recursion_limit = "128"]
#![cfg_attr(
    feature = "force_exhaustive_checks",
    feature(non_exhaustive_omitted_patterns_lint)
)]

extern crate proc_macro;
use proc_macro::TokenStream;
#[cfg(feature = "schema")]
use proc_macro2::Span;
use syn::{DeriveInput, Error, ItemEnum, ItemStruct, ItemUnion, Path};

///  by convention, local to borsh-derive crate, imports from proc_macro (1) are not allowed in `internals` module or in any of its submodules.
mod internals;

use crate::internals::attributes::item;

#[cfg(feature = "schema")]
use internals::schema;
use internals::{cratename, deserialize, serialize};

fn check_attrs_get_cratename(input: &TokenStream) -> Result<Path, Error> {
    let input = input.clone();

    let derive_input = syn::parse::<DeriveInput>(input)?;

    item::check_attributes(&derive_input)?;

    cratename::get(&derive_input.attrs)
}

/**
# derive proc-macro for `borsh::ser::BorshSerialize` trait

## Bounds

Generally, `BorshSerialize` adds `borsh::ser::BorshSerialize` bound to any type parameter
found in item's fields.

```ignore
/// impl<U, V> borsh::ser::BorshSerialize for A<U, V>
/// where
///     U: borsh::ser::BorshSerialize,
///     V: borsh::ser::BorshSerialize,
#[derive(BorshSerialize)]
struct A<U, V> {
    x: U,
    y: V,
}
```

```ignore
/// impl<U, V> borsh::ser::BorshSerialize for A<U, V>
/// where
///     U: borsh::ser::BorshSerialize,
#[derive(BorshSerialize)]
struct A<U, V> {
    x: U,
    #[borsh(skip)]
    y: V,
}
```

## Attributes

### 1. `#[borsh(crate = "path::to::borsh")]` (item level attribute)

###### syntax

Attribute takes literal string value, which is the syn's [Path] to `borsh` crate used.

###### usage

Attribute is optional.

1. If the attribute is not provided, [crate_name](proc_macro_crate::crate_name) is used to find a version of `borsh`
in `[dependencies]` of the relevant `Cargo.toml`. If there is no match, a compilation error, similar to the following, is raised:

```bash
 1  error: proc-macro derive panicked
   --> path/to/file.rs:27:10
    |
 27 | #[derive(BorshSerialize, BorshDeserialize)]
    |          ^^^^^^^^^^^^^^
    |
    = help: message: called `Result::unwrap()` on an `Err` value: CrateNotFound { crate_name: "borsh", path: "/path/to/Cargo.toml" }
```

2. If the attribute is provided, the check for `borsh` in `[dependencies]` of the relevant `Cargo.toml` is skipped.

Examples of usage:

```ignore
use reexporter::borsh::BorshSerialize;

// specifying the attribute removes need for a direct import of `borsh` into `[dependencies]`
#[derive(BorshSerialize)]
#[borsh(crate = "reexporter::borsh")]
struct B {
    x: u64,
    y: i32,
    c: String,
}
```

```ignore
use reexporter::borsh::{self, BorshSerialize};

// specifying the attribute removes need for a direct import of `borsh` into `[dependencies]`
#[derive(BorshSerialize)]
#[borsh(crate = "borsh")]
struct B {
    x: u64,
    y: i32,
    c: String,
}
```

### 2. `borsh(use_discriminant=<bool>)` (item level attribute)
This attribute is only applicable to enums.
`use_discriminant` allows to override the default behavior of serialization of enums with explicit discriminant.
`use_discriminant` is `false` behaves like version of borsh of 0.10.3.
You must specify `use_discriminant` for all enums with explicit discriminants in your project.

This is equivalent of borsh version 0.10.3 (explicit discriminant is ignored and this enum is equivalent to `A` without explicit discriminant):
```ignore
#[derive(BorshSerialize)]
#[borsh(use_discriminant = false)]
enum A {
    A
    B = 10,
}
```

To have explicit discriminant value serialized as is, you must specify `borsh(use_discriminant=true)` for enum.
```ignore
#[derive(BorshSerialize)]
#[borsh(use_discriminant = true)]
enum B {
    A
    B = 10,
}
```

###### borsh, expressions, evaluating to `isize`, as discriminant
This case is not supported:
```ignore
const fn discrim() -> isize {
    0x14
}

#[derive(BorshSerialize)]
#[borsh(use_discriminant = true)]
enum X {
    A,
    B = discrim(), // expressions, evaluating to `isize`, which are allowed outside of `borsh` context
    C,
    D,
    E = 10,
    F,
}
```

###### borsh explicit discriminant does not support literal values outside of u8 range
This is not supported:
```ignore
#[derive(BorshSerialize)]
#[borsh(use_discriminant = true)]
enum X {
    A,
    B = 0x100, // literal values outside of `u8` range
    C,
    D,
    E = 10,
    F,
}
```

### 3. `#[borsh(skip)]` (field level attribute)

`#[borsh(skip)]` makes derive skip serializing annotated field.

`#[borsh(skip)]` makes derive skip adding any type parameters, present in the field, to parameters bound by `borsh::ser::BorshSerialize`.

```ignore
#[derive(BorshSerialize)]
struct A {
    x: u64,
    #[borsh(skip)]
    y: f32,
}
```

### 4. `#[borsh(bound(serialize = ...))]` (field level attribute)

###### syntax

Attribute takes literal string value, which is a comma-separated list of syn's [WherePredicate](syn::WherePredicate)-s, which may be empty.

###### usage

Attribute adds possibility to override bounds for `BorshSerialize` in order to enable:

1. removal of bounds on type parameters from struct/enum definition itself and moving them to the trait's implementation block.
2. fixing complex cases, when derive hasn't figured out the right bounds on type parameters automatically.

```ignore
/// additional bound `T: Ord` (required by `HashMap`) is injected into
/// derived trait implementation via attribute to avoid adding the bounds on the struct itself
#[derive(BorshSerialize)]
struct A<T, U> {
    a: String,
    #[borsh(bound(serialize =
        "T: borsh::ser::BorshSerialize + Ord,
         U: borsh::ser::BorshSerialize"))]
    b: HashMap<T, U>,
}
```


```ignore
/// derive here figures the bound erroneously as `T: borsh::ser::BorshSerialize`
#[derive(BorshSerialize)]
struct A<T, V>
where
    T: TraitName,
{
    #[borsh(bound(serialize = "<T as TraitName>::Associated: borsh::ser::BorshSerialize"))]
    field: <T as TraitName>::Associated,
    another: V,
}
```

###### interaction with `#[borsh(skip)]`

`#[borsh(bound(serialize = ...))]` replaces bounds, which are derived automatically,
irrelevant of whether `#[borsh(skip)]` attribute is present.

### 5. `#[borsh(serialize_with = ...)]` (field level attribute)

###### syntax

Attribute takes literal string value, which is a syn's [ExprPath](syn::ExprPath).

###### usage

Attribute adds possibility to specify full path of function, optionally qualified with generics,
with which to serialize the annotated field.

It may be used when `BorshSerialize` cannot be implemented for field's type, if it's from foreign crate.

It may be used to override the implementation of serialization for some other reason.

```ignore
use indexmap::IndexMap;

mod index_map_impl {
    use super::IndexMap;
    use core::hash::Hash;

    pub fn serialize_index_map<
        K: borsh::ser::BorshSerialize,
        V: borsh::ser::BorshSerialize,
        W: borsh::io::Write,
    >(
        obj: &IndexMap<K, V>,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::io::Error> {
        let key_value_tuples = obj.iter().collect::<Vec<_>>();
        borsh::BorshSerialize::serialize(&key_value_tuples, writer)?;
        Ok(())
    }
}

#[derive(BorshSerialize)]
struct B<K, V> {
    #[borsh(
        serialize_with = "index_map_impl::serialize_index_map",
    )]
    x: IndexMap<K, V>,
    y: String,
}
```

###### interaction with `#[borsh(skip)]`

`#[borsh(serialize_with = ...)]` is not allowed to be used simultaneously with `#[borsh(skip)]`.


*/
#[proc_macro_derive(BorshSerialize, attributes(borsh))]
pub fn borsh_serialize(input: TokenStream) -> TokenStream {
    let cratename = match check_attrs_get_cratename(&input) {
        Ok(cratename) => cratename,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        serialize::structs::process(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        serialize::enums::process(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemUnion>(input) {
        serialize::unions::process(&input, cratename)
    } else {
        // Derive macros can only be defined on structs, enums, and unions.
        unreachable!()
    };
    TokenStream::from(match res {
        Ok(res) => res,
        Err(err) => err.to_compile_error(),
    })
}

/**
# derive proc-macro for `borsh::de::BorshDeserialize` trait

## Bounds

Generally, `BorshDeserialize` adds `borsh::de::BorshDeserialize` bound to any type parameter
found in item's fields and `core::default::Default` bound to any type parameter found
in item's skipped fields.

```ignore
/// impl<U, V> borsh::de::BorshDeserialize for A<U, V>
/// where
///     U: borsh::de::BorshDeserialize,
///     V: borsh::de::BorshDeserialize,
#[derive(BorshDeserialize)]
struct A<U, V> {
    x: U,
    y: V,
}
```

```ignore
/// impl<U, V> borsh::de::BorshDeserialize for A<U, V>
/// where
///     U: borsh::de::BorshDeserialize,
///     V: core::default::Default,
#[derive(BorshDeserialize)]
struct A<U, V> {
    x: U,
    #[borsh(skip)]
    y: V,
}
```


## Attributes

### 1. `#[borsh(crate = "path::to::borsh")]` (item level attribute)

###### syntax

Attribute takes literal string value, which is the syn's [Path] to `borsh` crate used.

###### usage

Attribute is optional.

1. If the attribute is not provided, [crate_name](proc_macro_crate::crate_name) is used to find a version of `borsh`
in `[dependencies]` of the relevant `Cargo.toml`. If there is no match, a compilation error, similar to the following, is raised:

```bash
 1  error: proc-macro derive panicked
   --> path/to/file.rs:27:10
    |
 27 | #[derive(BorshDeserialize, BorshSerialize)]
    |          ^^^^^^^^^^^^^^^^
    |
    = help: message: called `Result::unwrap()` on an `Err` value: CrateNotFound { crate_name: "borsh", path: "/path/to/Cargo.toml" }
```

2. If the attribute is provided, the check for `borsh` in `[dependencies]` of the relevant `Cargo.toml` is skipped.

Examples of usage:

```ignore
use reexporter::borsh::BorshDeserialize;

// specifying the attribute removes need for a direct import of `borsh` into `[dependencies]`
#[derive(BorshDeserialize)]
#[borsh(crate = "reexporter::borsh")]
struct B {
    x: u64,
    y: i32,
    c: String,
}
```

```ignore
use reexporter::borsh::{self, BorshDeserialize};

// specifying the attribute removes need for a direct import of `borsh` into `[dependencies]`
#[derive(BorshDeserialize)]
#[borsh(crate = "borsh")]
struct B {
    x: u64,
    y: i32,
    c: String,
}
```

### 2. `#[borsh(init=...)]` (item level attribute)

###### syntax

Attribute's value is syn's [Path]-s, passed to borsh top level meta attribute as value of `init` argument.

###### usage

`#[borsh(init=...)]` allows to automatically run an initialization function right after deserialization.
This adds a lot of convenience for objects that are architectured to be used as strictly immutable.

```ignore
#[derive(BorshDeserialize)]
#[borsh(init=init)]
struct Message {
    message: String,
    timestamp: u64,
    public_key: CryptoKey,
    signature: CryptoSignature,
    hash: CryptoHash,
}

impl Message {
    pub fn init(&mut self) {
        self.hash = CryptoHash::new().write_string(self.message).write_u64(self.timestamp);
        self.signature.verify(self.hash, self.public_key);
    }
}
```

### 3. `borsh(use_discriminant=<bool>)` (item level attribute)

This attribute is only applicable to enums.
`use_discriminant` allows to override the default behavior of serialization of enums with explicit discriminant.
`use_discriminant` is `false` behaves like version of borsh of 0.10.3.
It's useful for backward compatibility and you can set this value to `false` to deserialise data serialised by older version of `borsh`.
You must specify `use_discriminant` for all enums with explicit discriminants in your project.

This is equivalent of borsh version 0.10.3 (explicit discriminant is ignored and this enum is equivalent to `A` without explicit discriminant):
```ignore
#[derive(BorshDeserialize)]
#[borsh(use_discriminant = false)]
enum A {
    A
    B = 10,
}
```

To have explicit discriminant value serialized as is, you must specify `borsh(use_discriminant=true)` for enum.
```ignore
#[derive(BorshDeserialize)]
#[borsh(use_discriminant = true)]
enum B {
    A
    B = 10,
}
```


###### borsh, expressions, evaluating to `isize`, as discriminant
This case is not supported:
```ignore
const fn discrim() -> isize {
    0x14
}

#[derive(BorshDeserialize)]
#[borsh(use_discriminant = true)]
enum X {
    A,
    B = discrim(), // expressions, evaluating to `isize`, which are allowed outside of `borsh` context
    C,
    D,
    E = 10,
    F,
}
```


###### borsh explicit discriminant does not support literal values outside of u8 range.
This is not supported:
```ignore
#[derive(BorshDeserialize)]
#[borsh(use_discriminant = true)]
enum X {
    A,
    B = 0x100, // literal values outside of `u8` range
    C,
    D,
    E = 10,
    F,
}
```


### 4. `#[borsh(skip)]` (field level attribute)

`#[borsh(skip)]` makes derive skip deserializing annotated field.

`#[borsh(skip)]` makes derive skip adding any type parameters, present in the field, to parameters bound by `borsh::de::BorshDeserialize`.

It adds `core::default::Default` bound to any
parameters encountered in annotated field.


```ignore
#[derive(BorshDeserialize)]
struct A {
    x: u64,
    #[borsh(skip)]
    y: f32,
}
```


### 5. `#[borsh(bound(deserialize = ...))]` (field level attribute)

###### syntax

Attribute takes literal string value, which is a comma-separated list of syn's [WherePredicate](syn::WherePredicate)-s, which may be empty.


###### usage

Attribute adds possibility to override bounds for `BorshDeserialize` in order to enable:

1. removal of bounds on type parameters from struct/enum definition itself and moving them to the trait's implementation block.
2. fixing complex cases, when derive hasn't figured out the right bounds on type parameters automatically.

```ignore
/// additional bounds `T: Ord + Hash + Eq` (required by `HashMap`) are injected into
/// derived trait implementation via attribute to avoid adding the bounds on the struct itself
#[derive(BorshDeserialize)]
struct A<T, U> {
    a: String,
    #[borsh(bound(
        deserialize =
        "T: Ord + Hash + Eq + borsh::de::BorshDeserialize,
         U: borsh::de::BorshDeserialize"
    ))]
    b: HashMap<T, U>,
}
```


```ignore
// derive here figures the bound erroneously as `T: borsh::de::BorshDeserialize,`
#[derive(BorshDeserialize)]
struct A<T, V>
where
    T: TraitName,
{
    #[borsh(bound(deserialize = "<T as TraitName>::Associated: borsh::de::BorshDeserialize"))]
    field: <T as TraitName>::Associated,
    another: V,
}
```

###### interaction with `#[borsh(skip)]`

`#[borsh(bound(deserialize = ...))]` replaces bounds, which are derived automatically,
irrelevant of whether `#[borsh(skip)]` attribute is present.

```ignore
/// implicit derived `core::default::Default` bounds on `K` and `V` type parameters are removed by
/// empty bound specified, as `HashMap` has its own `Default` implementation
#[derive(BorshDeserialize)]
struct A<K, V, U>(
    #[borsh(skip, bound(deserialize = ""))]
    HashMap<K, V>,
    U,
);
```

### 6. `#[borsh(deserialize_with = ...)]` (field level attribute)

###### syntax

Attribute takes literal string value, which is a syn's [ExprPath](syn::ExprPath).

###### usage

Attribute adds possibility to specify full path of function, optionally qualified with generics,
with which to deserialize the annotated field.

It may be used when `BorshDeserialize` cannot be implemented for field's type, if it's from foreign crate.

It may be used to override the implementation of deserialization for some other reason.

```ignore
use indexmap::IndexMap;

mod index_map_impl {
    use super::IndexMap;
    use core::hash::Hash;

    pub fn deserialize_index_map<
        R: borsh::io::Read,
        K: borsh::de::BorshDeserialize + Hash + Eq,
        V: borsh::de::BorshDeserialize,
    >(
        reader: &mut R,
    ) -> ::core::result::Result<IndexMap<K, V>, borsh::io::Error> {
        let vec: Vec<(K, V)> = borsh::BorshDeserialize::deserialize_reader(reader)?;
        let result: IndexMap<K, V> = vec.into_iter().collect();
        Ok(result)
    }
}

#[derive(BorshDeserialize)]
struct B<K: Hash + Eq, V> {
    #[borsh(
        deserialize_with = "index_map_impl::deserialize_index_map",
    )]
    x: IndexMap<K, V>,
    y: String,
}
```

###### interaction with `#[borsh(skip)]`

`#[borsh(deserialize_with = ...)]` is not allowed to be used simultaneously with `#[borsh(skip)]`.

*/
#[proc_macro_derive(BorshDeserialize, attributes(borsh))]
pub fn borsh_deserialize(input: TokenStream) -> TokenStream {
    let cratename = match check_attrs_get_cratename(&input) {
        Ok(cratename) => cratename,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        deserialize::structs::process(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        deserialize::enums::process(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemUnion>(input) {
        deserialize::unions::process(&input, cratename)
    } else {
        // Derive macros can only be defined on structs, enums, and unions.
        unreachable!()
    };
    TokenStream::from(match res {
        Ok(res) => res,
        Err(err) => err.to_compile_error(),
    })
}

/**
# derive proc-macro for `borsh::BorshSchema` trait

## Bounds

Generally, `BorshSchema` adds `borsh::BorshSchema` bound to any type parameter
found in item's fields.

```ignore
/// impl<U, V> borsh::BorshSchema for A<U, V>
/// where
///     U: borsh::BorshSchema,
///     V: borsh::BorshSchema,
#[derive(BorshSchema)]
struct A<U, V> {
    x: U,
    y: V,
}
```

```ignore
/// impl<U, V> borsh::BorshSchema for A<U, V>
/// where
///     U: borsh::BorshSchema,
#[derive(BorshSchema)]
struct A<U, V> {
    x: U,
    #[borsh(skip)]
    y: V,
}
```

## Attributes

### 1. `#[borsh(crate = "path::to::borsh")]` (item level attribute)

###### syntax

Attribute takes literal string value, which is the syn's [Path] to `borsh` crate used.

###### usage

Attribute is optional.

1. If the attribute is not provided, [crate_name](proc_macro_crate::crate_name) is used to find a version of `borsh`
in `[dependencies]` of the relevant `Cargo.toml`. If there is no match, a compilation error, similar to the following, is raised:

```bash
 1  error: proc-macro derive panicked
   --> path/to/file.rs:27:10
    |
 27 | #[derive(BorshSchema, BorshSerialize)]
    |          ^^^^^^^^^^^
    |
    = help: message: called `Result::unwrap()` on an `Err` value: CrateNotFound { crate_name: "borsh", path: "/path/to/Cargo.toml" }
```

2. If the attribute is provided, the check for `borsh` in `[dependencies]` of the relevant `Cargo.toml` is skipped.

Examples of usage:

```ignore
use reexporter::borsh::BorshSchema;

// specifying the attribute removes need for a direct import of `borsh` into `[dependencies]`
#[derive(BorshSchema)]
#[borsh(crate = "reexporter::borsh")]
struct B {
    x: u64,
    y: i32,
    c: String,
}
```

```ignore
use reexporter::borsh::{self, BorshSchema};

// specifying the attribute removes need for a direct import of `borsh` into `[dependencies]`
#[derive(BorshSchema)]
#[borsh(crate = "borsh")]
struct B {
    x: u64,
    y: i32,
    c: String,
}
```

### 2. `borsh(use_discriminant=<bool>)` (item level attribute)
This attribute is only applicable to enums.
`use_discriminant` allows to override the default behavior of serialization of enums with explicit discriminant.
`use_discriminant` is `false` behaves like version of borsh of 0.10.3.
You must specify `use_discriminant` for all enums with explicit discriminants in your project.

This is equivalent of borsh version 0.10.3 (explicit discriminant is ignored and this enum is equivalent to `A` without explicit discriminant):
```ignore
#[derive(BorshSchema)]
#[borsh(use_discriminant = false)]
enum A {
    A
    B = 10,
}
```

To have explicit discriminant value serialized as is, you must specify `borsh(use_discriminant=true)` for enum.
```ignore
#[derive(BorshSchema)]
#[borsh(use_discriminant = true)]
enum B {
    A
    B = 10,
}
```

###### borsh, expressions, evaluating to `isize`, as discriminant
This case is not supported:
```ignore
const fn discrim() -> isize {
    0x14
}

#[derive(BorshSchema)]
#[borsh(use_discriminant = true)]
enum X {
    A,
    B = discrim(), // expressions, evaluating to `isize`, which are allowed outside of `borsh` context
    C,
    D,
    E = 10,
    F,
}
```

###### borsh explicit discriminant does not support literal values outside of u8 range
This is not supported:
```ignore
#[derive(BorshSchema)]
#[borsh(use_discriminant = true)]
enum X {
    A,
    B = 0x100, // literal values outside of `u8` range
    C,
    D,
    E = 10,
    F,
}
```

### 3. `#[borsh(skip)]` (field level attribute)

`#[borsh(skip)]` makes derive skip including schema from annotated field into schema's implementation.

`#[borsh(skip)]` makes derive skip adding any type parameters, present in the field, to parameters bound by `borsh::BorshSchema`.

```ignore
#[derive(BorshSchema)]
struct A {
    x: u64,
    #[borsh(skip)]
    y: f32,
}
```

### 4. `#[borsh(schema(params = ...))]` (field level attribute)

###### syntax

Attribute takes literal string value, which is a comma-separated list of `ParameterOverride`-s, which may be empty.

###### usage
It may be used in order to:

1. fix complex cases, when derive hasn't figured out the right bounds on type parameters and
declaration parameters automatically.
2. remove parameters, which do not take part in serialization/deserialization, from bounded ones and from declaration parameters.

`ParameterOverride` describes an entry like `order_param => override_type`,

e.g. `K => <K as TraitName>::Associated`.

Such an entry instructs `BorshSchema` derive to:

1. add `override_type` to types, bounded by `borsh::BorshSchema` in implementation block.
2. add `<override_type>::declaration()` to parameters vector in `fn declaration()` method of `BorshSchema` trait that is being derived.
3. the `order_param` is required to establish the same order in parameters vector (2.) as that of type parameters in generics of type, that `BorshSchema` is derived for.
4. entries, specified for a field, together replace whatever would've been derived automatically for 1. and 2. .


```ignore
// derive here figures the bound erroneously as `T: borsh::BorshSchema` .
// attribute replaces it with <T as TraitName>::Associated: borsh::BorshSchema`
#[derive(BorshSchema)]
struct A<V, T>
where
    T: TraitName,
{
    #[borsh(schema(params = "T => <T as TraitName>::Associated"))]
    field: <T as TraitName>::Associated,
    another: V,
}
```

```ignore
// K in PrimaryMap isn't stored during serialization / read during deserialization.
// thus, it's not a parameter, relevant for `BorshSchema`
// ...
// impl<K: EntityRef, V> borsh::BorshSchema for A<K, V>
// where
//     V: borsh::BorshSchema,
#[derive(BorshSchema)]
struct A<K: EntityRef, V> {
    #[borsh(
        schema(
            params = "V => V"
        )
    )]
    x: PrimaryMap<K, V>,
    y: String,
}

#[derive(BorshSchema)]
pub struct PrimaryMap<K, V>
where
    K: EntityRef,
{
    elems: Vec<V>,
    unused: PhantomData<K>,
}
```

###### interaction with `#[borsh(skip)]`

`#[borsh(schema(params = ...))]` is not allowed to be used simultaneously with `#[borsh(skip)]`.

### 5. `#[borsh(schema(with_funcs(declaration = ..., definitions = ...)))]` (field level attribute)

###### syntax

Each of `declaration` and `definitions` nested sub-attributes takes literal string value, which is a syn's [ExprPath](syn::ExprPath).

Currently both `declaration` and `definitions` are required to be specified at the same time.

###### usage

Attribute adds possibility to specify full path of 2 functions, optionally qualified with generics,
with which to generate borsh schema for annotated field.

It may be used when `BorshSchema` cannot be implemented for field's type, if it's from foreign crate.

It may be used to override the implementation of schema for some other reason.

```ignore
use indexmap::IndexMap;

mod index_map_impl {
    pub mod schema {
        use std::collections::BTreeMap;

        use borsh::{
            schema::{Declaration, Definition},
            BorshSchema,
        };

        pub fn declaration<K: borsh::BorshSchema, V: borsh::BorshSchema>() -> Declaration {
            let params = vec![<K>::declaration(), <V>::declaration()];
            format!(r#"{}<{}>"#, "IndexMap", params.join(", "))
        }

        pub fn add_definitions_recursively<K: borsh::BorshSchema, V: borsh::BorshSchema>(
            definitions: &mut BTreeMap<Declaration, Definition>,
        ) {
            let definition = Definition::Sequence {
                elements: <(K, V)>::declaration(),
            };
            let no_recursion_flag = definitions.get(&declaration::<K, V>()).is_none();
            <() as BorshSchema>::add_definition(declaration::<K, V>(), definition, definitions);
            if no_recursion_flag {
                <(K, V)>::add_definitions_recursively(definitions);
            }
        }
    }
}

#[derive(BorshSchema)]
struct B<K, V> {
    #[borsh(
        schema(
            with_funcs(
                declaration = "index_map_impl::schema::declaration::<K, V>",
                definitions = "index_map_impl::schema::add_definitions_recursively::<K, V>"
            ),
        )
    )]
    x: IndexMap<K, V>,
    y: String,
}
```

###### interaction with `#[borsh(skip)]`

`#[borsh(schema(with_funcs(declaration = ..., definitions = ...)))]` is not allowed to be used simultaneously with `#[borsh(skip)]`.

*/
#[cfg(feature = "schema")]
#[proc_macro_derive(BorshSchema, attributes(borsh))]
pub fn borsh_schema(input: TokenStream) -> TokenStream {
    let cratename = match check_attrs_get_cratename(&input) {
        Ok(cratename) => cratename,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        schema::structs::process(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        schema::enums::process(&input, cratename)
    } else if syn::parse::<ItemUnion>(input).is_ok() {
        Err(syn::Error::new(
            Span::call_site(),
            "Borsh schema does not support unions yet.",
        ))
    } else {
        // Derive macros can only be defined on structs, enums, and unions.
        unreachable!()
    };
    TokenStream::from(match res {
        Ok(res) => res,
        Err(err) => err.to_compile_error(),
    })
}
