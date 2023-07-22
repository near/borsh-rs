# Borsh in Rust &emsp; [![Latest Version]][crates.io] [![borsh: rustc 1.65+]][Rust 1.65] [![License Apache-2.0 badge]][License Apache-2.0] [![License MIT badge]][License MIT]

[Borsh]: https://borsh.io
[Latest Version]: https://img.shields.io/crates/v/borsh.svg
[crates.io]: https://crates.io/crates/borsh
[borsh: rustc 1.65+]: https://img.shields.io/badge/rustc-1.65+-lightgray.svg
[Rust 1.65]: https://blog.rust-lang.org/2022/11/03/Rust-1.65.0.html
[License Apache-2.0 badge]: https://img.shields.io/badge/license-Apache2.0-blue.svg
[License Apache-2.0]: https://opensource.org/licenses/Apache-2.0
[License MIT badge]: https://img.shields.io/badge/license-MIT-blue.svg
[License MIT]: https://opensource.org/licenses/MIT

**borsh-rs** is Rust implementation of the [Borsh] binary serialization format.

Borsh stands for _Binary Object Representation Serializer for Hashing_. It is meant to be used in
security-critical projects as it prioritizes [consistency, safety, speed][Borsh], and comes with a
strict [specification](https://github.com/near/borsh#specification).

## Example

```rust
use borsh::{BorshSerialize, BorshDeserialize, from_slice};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
struct A {
    x: u64,
    y: String,
}

#[test]
fn test_simple_struct() {
    let a = A {
        x: 3301,
        y: "liber primus".to_string(),
    };
    let encoded_a = a.try_to_vec().unwrap();
    let decoded_a = from_slice::<A>(&encoded_a).unwrap();
    assert_eq!(a, decoded_a);
}
```

## Features

Opting out from Serde allows borsh to have some features that currently are not available for serde-compatible serializers.
Currently we support two features: `borsh_init` and `borsh_skip` (the former one not available in Serde).

`borsh_init` allows to automatically run an initialization function right after deserialization. This adds a lot of convenience for objects that are architectured to be used as strictly immutable. Usage example:

```rust
#[derive(BorshSerialize, BorshDeserialize)]
#[borsh_init(init)]
struct Message {
    message: String,
    timestamp: u64,
    public_key: CryptoKey,
    signature: CryptoSignature
    hash: CryptoHash
}

impl Message {
    pub fn init(&mut self) {
        self.hash = CryptoHash::new().write_string(self.message).write_u64(self.timestamp);
        self.signature.verify(self.hash, self.public_key);
    }
}
```

`borsh_skip` allows to skip serializing/deserializing fields, assuming they implement `Default` trait, similary to `#[serde(skip)]`.

```rust
#[derive(BorshSerialize, BorshDeserialize)]
struct A {
    x: u64,
    #[borsh_skip]
    y: f32,
}
```

### Enum with explicit discriminant

`#[borsh(use_discriminant=false|true])` is required if you have an enum with explicit discriminant. This settings affects `BorshSerialize` and `BorshDeserialize` behaviour at the same time.

If you don't specify `use_discriminant` option for enum with explicit discriminant, you will get an error:

````bash
error: You have to specify `#[borsh(use_discriminant=true)]` or `#[borsh(use_discriminant=false)]` for all structs that have enum with explicit discriminant
```

```rust
#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(use_discriminant=false)]
enum A {
    X,
    Y = 10,
}
````

Will keep old behaviour of borsh deserialization and will not use discriminant. This option is left to have backward compatability with previous versions of borsh and to have ability to deserialise data from previous versions of borsh.

```rust
#[derive(BorshDeserialize, BorshSerialize)]
#[borsh(use_discriminant=true)]
enum A {
    X,
    Y = 10,
}
```

This one will use proper version of serialization of enum with explicit discriminant.

## Releasing

The versions of all public crates in this repository are collectively managed by a single version in the [workspace manifest](https://github.com/near/borsh-rs/blob/master/Cargo.toml).

So, to publish a new version of all the crates, you can do so by simply bumping that to the next "patch" version and submit a PR.

We have CI Infrastructure put in place to automate the process of publishing all crates once a version change has merged into master.

However, before you release, make sure the [CHANGELOG](CHANGELOG.md) is up to date and that the `[Unreleased]` section is present but empty.

## License

This repository is distributed under the terms of both the MIT license and the Apache License (Version 2.0).
See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
