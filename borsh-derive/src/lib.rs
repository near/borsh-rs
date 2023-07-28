extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::crate_name;
use proc_macro_crate::FoundCrate;
use syn::{parse_macro_input, DeriveInput, Ident, ItemEnum, ItemStruct, ItemUnion};

use borsh_derive_internal::*;
#[cfg(feature = "schema")]
use borsh_schema_derive_internal::*;

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
    #[borsh_skip]
    y: V,
}
```

## Attributes

### `borsh_skip` (field level attribute)

`borsh_skip` makes derive skip serializing annotated field.

`borsh_skip` makes derive skip adding any type parameters, present in the field, to parameters bound by `borsh::ser::BorshSerialize`.

```ignore
#[derive(BorshSerialize)]
struct A {
    x: u64,
    #[borsh_skip]
    y: f32,
}
```

### `#[borsh(bound(serialize = ...))]` (field level attribute)

#### syntax

Attribute takes literal string value, which is a comma-separated list of syn's [WherePredicate](syn::WherePredicate)-s, which may be empty.

#### usage

Attribute adds possibility to override bounds for `BorshSerialize` in order to enable removal
of bounds on type parameters from struct/enum definition itself and fixing complex cases,
when derive hasn't figured out the right bounds on type parameters automatically.

```ignore
/// additional bound `T: PartialOrd` (required by `HashMap`) is injected into
/// derived trait implementation via attribute to avoid adding the bounds on the struct itself
#[derive(BorshSerialize)]
struct A<T, U> {
    a: String,
    #[borsh(bound(serialize =
        "T: borsh::ser::BorshSerialize + PartialOrd,
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

#### interaction with `#[borsh_skip]`

`#[borsh(bound(serialize = ...))]` replaces bounds, which are derived automatically,
irrelevant of whether `#[borsh_skip]` attribute is present.

#### interaction with `#[borsh(bound(deserialize = ...))]`

Both attributes may be used simultaneously, separated by a comma: `#[borsh(bound(serialize = ..., deserialize = ...))]`

### `borsh(use_discriminant=<bool>)` (item level attribute)
This attribute is only applicable to enums.
`use_discriminant` allows to override the default behavior of serialization of enums with explicit discriminant.
`use_discriminant` is `false` behaves like version of borsh of 0.10.3.
You must to specify `use_discriminant` for all enums explicit discriminants in your project.

This is equivalent of borsh version 0.10.3 (explicit discriminant is ignored and this enum is equavielent to `A` without explicit discriminant):
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
#[derive(BorsSerialize)]
#[borsh(use_discriminant = true)]
enum B {
    A
    B = 10,
}
```

### borsh constant value as discriminant
This case is not supported:
```ignore
const fn discrim() -> isize {
    0x14
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
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

This is possible without `BorshSerialize`:
```ignore
enum X1 {
    A,
    B = discrim(),
    C,
    D,
    E = 10,
    F,
}
```
If you need to support this weird case for some strange reason, you can take the expansion of derive as template
and write the impl manually:
```ignore
impl borsh::ser::BorshSerialize for X {
    fn serialize<W: borsh::__private::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::__private::maybestd::io::Error> {
        let variant_idx: u8 = match self {
            X::A => 0,
            X::B => discrim()
                .try_into()
                .map_err(|_err| std::io::Error::new(std::io::ErrorKind::InvalidData, "err"))?,
            X::C => (discrim() + 1)
                .try_into()
                .map_err(|_err| std::io::Error::new(std::io::ErrorKind::InvalidData, "err"))?,
            X::D => (discrim() + 2)
                .try_into()
                .map_err(|_err| std::io::Error::new(std::io::ErrorKind::InvalidData, "err"))?,
            X::E => 10,
            X::F => 10 + 1,
        };
        writer.write_all(&variant_idx.to_le_bytes())?;
        match self {
            X::A => {}
            X::B => {}
            X::C => {}
            X::D => {}
            X::E => {}
            X::F => {}
        }
        Ok(())
    }
}
```
### borsh explicit discriminant does not support values greater than 256
This is not supported:
```ignore
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
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

*/
#[proc_macro_derive(BorshSerialize, attributes(borsh_skip, borsh))]
pub fn borsh_serialize(input: TokenStream) -> TokenStream {
    let name = &crate_name("borsh").unwrap();
    let name = match name {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    let cratename = Ident::new(name, Span::call_site());

    let for_derive_input = input.clone();
    let derive_input = parse_macro_input!(for_derive_input as DeriveInput);

    if let Err(err) = check_item_attributes(&derive_input) {
        return err.into();
    }

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        struct_ser(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        enum_ser(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemUnion>(input) {
        union_ser(&input, cratename)
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
    #[borsh_skip]
    y: V,
}
```


## Attributes

### `borsh_init` (item level attribute)

`borsh_init` allows to automatically run an initialization function right after deserialization.
This adds a lot of convenience for objects that are architectured to be used as strictly immutable.

```ignore
#[derive(BorshDeserialize)]
#[borsh_init(init)]
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

### `borsh_skip` (field level attribute)

`borsh_skip` makes derive skip deserializing annotated field.

`borsh_skip` makes derive skip adding any type parameters, present in the field, to parameters bound by `borsh::de::BorshDeserialize`.

It adds `core::default::Default` bound to any
parameters encountered in annotated field.


```ignore
#[derive(BorshDeserialize)]
struct A {
    x: u64,
    #[borsh_skip]
    y: f32,
}
```


### `#[borsh(bound(deserialize = ...))]` (field level attribute)

#### syntax

Attribute takes literal string value, which is a comma-separated list of syn's [WherePredicate](syn::WherePredicate)-s, which may be empty.


#### usage

Attribute adds possibility to override bounds for `BorshDeserialize` in order to enable removal
of bounds on type parameters from struct/enum definition itself and fixing complex cases,
when derive hasn't figured out the right bounds on type parameters automatically.

```ignore
/// additional bounds `T: PartialOrd + Hash + Eq` (required by `HashMap`) are injected into
/// derived trait implementation via attribute to avoid adding the bounds on the struct itself
#[derive(BorshDeserialize)]
struct A<T, U> {
    a: String,
    #[borsh(bound(
        deserialize =
        "T: PartialOrd + Hash + Eq + borsh::de::BorshDeserialize,
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

#### interaction with `#[borsh_skip]`

`#[borsh(bound(deserialize = ...))]` replaces bounds, which are derived automatically,
irrelevant of whether `#[borsh_skip]` attribute is present.

```ignore
/// implicit derived `core::default::Default` bounds on `K` and `V` type parameters are removed by
/// empty bound specified, as `HashMap` has its own `Default` implementation
#[derive(BorshDeserialize)]
struct A<K, V, U>(
    #[borsh_skip]
    #[borsh(bound(deserialize = ""))]
    HashMap<K, V>,
    U,
);
```

#### interaction with `#[borsh(bound(serialize = ...))]`

Both attributes may be used simultaneously, separated by a comma: `#[borsh(bound(serialize = ..., deserialize = ...))]`

This one will use proper version of serialization of enum with explicit discriminant.

### `borsh(use_discriminant=<bool>)` (item level attribute)
This attribute is only applicable to enums.
`use_discriminant` allows to override the default behavior of serialization of enums with explicit discriminant.
`use_discriminant` is `false` behaves like version of borsh of 0.10.3.
It's useful for backward compatibility and you can set this value to `false` to deserialise data serialised by older version of `borsh`.
You must to specify `use_discriminant` for all enums with explicit discriminants in your project.

This is equivalent of borsh version 0.10.3 (explicit discriminant is ignored and this enum is equavielent to `A` without explicit discriminant):
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


### borsh constant value as discriminant
This case is not supported:
```ignore
const fn discrim() -> isize {
    0x14
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
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

This is possible without `BorshSerialize`:
```ignore
enum X1 {
    A,
    B = discrim(),
    C,
    D,
    E = 10,
    F,
}
```
If you need to support this weird case for some strange reason, you can take the expansion of derive as template
and write the impl manually:
```ignore
impl borsh::ser::BorshSerialize for X {
    fn serialize<W: borsh::__private::maybestd::io::Write>(
        &self,
        writer: &mut W,
    ) -> ::core::result::Result<(), borsh::__private::maybestd::io::Error> {
        let variant_idx: u8 = match self {
            X::A => 0,
            X::B => discrim()
                .try_into()
                .map_err(|_err| std::io::Error::new(std::io::ErrorKind::InvalidData, "err"))?,
            X::C => (discrim() + 1)
                .try_into()
                .map_err(|_err| std::io::Error::new(std::io::ErrorKind::InvalidData, "err"))?,
            X::D => (discrim() + 2)
                .try_into()
                .map_err(|_err| std::io::Error::new(std::io::ErrorKind::InvalidData, "err"))?,
            X::E => 10,
            X::F => 10 + 1,
        };
        writer.write_all(&variant_idx.to_le_bytes())?;
        match self {
            X::A => {}
            X::B => {}
            X::C => {}
            X::D => {}
            X::E => {}
            X::F => {}
        }
        Ok(())
    }
}
```

### borsh explicit discriminant does not support values greater than 256
This is not supported:
```ignore
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Debug)]
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

*/
#[proc_macro_derive(BorshDeserialize, attributes(borsh_skip, borsh_init, borsh))]
pub fn borsh_deserialize(input: TokenStream) -> TokenStream {
    let name = &crate_name("borsh").unwrap();
    let name = match name {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    let cratename = Ident::new(name, Span::call_site());

    let for_derive_input = input.clone();
    let derive_input = parse_macro_input!(for_derive_input as DeriveInput);

    match check_item_attributes(&derive_input) {
        Ok(..) => {}
        Err(value) => return value.into(),
    };

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        struct_de(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        enum_de(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemUnion>(input) {
        union_de(&input, cratename)
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
    #[borsh_skip]
    y: V,
}
```

## Attributes

### `borsh_skip` (field level attribute)

`borsh_skip` makes derive skip including schema from annotated field into schema's implementation.

`borsh_skip` makes derive skip adding any type parameters, present in the field, to parameters bound by `borsh::BorshSchema`.

```ignore
#[derive(BorshSchema)]
struct A {
    x: u64,
    #[borsh_skip]
    y: f32,
}
```

### `#[borsh(schema(params = ...))]` (field level attribute)

#### syntax

Attribute takes literal string value, which is a comma-separated list of [SchemaParamsOverride]-s, which may be empty.

#### usage
It may be used to fix complex cases, when derive hasn't figured out the right bounds on type parameters and declaration
parameters automatically.

[SchemaParamsOverride] describes an entry like `order_param => override_type`,

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

#### interaction with `#[borsh_skip]`

`#[borsh(schema(params = ...))]` is ignored if field is also annotated with `#[borsh_skip]`.

*/
#[cfg(feature = "schema")]
#[proc_macro_derive(BorshSchema, attributes(borsh_skip, borsh))]
pub fn borsh_schema(input: TokenStream) -> TokenStream {
    let name = &crate_name("borsh").unwrap();
    let name = match name {
        FoundCrate::Itself => "borsh",
        FoundCrate::Name(name) => name.as_str(),
    };
    let cratename = Ident::new(name, Span::call_site());

    let res = if let Ok(input) = syn::parse::<ItemStruct>(input.clone()) {
        process_struct(&input, cratename)
    } else if let Ok(input) = syn::parse::<ItemEnum>(input.clone()) {
        process_enum(&input, cratename)
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
