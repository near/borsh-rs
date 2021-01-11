# Borsh in Rust &emsp; [![Build Status]][travis-ci] [![Latest Version]][crates.io] [![borsh: rustc 1.40+]][Rust 1.40] [![License Apache-2.0 badge]][License Apache-2.0] [![License MIT badge]][License MIT]

[Borsh]: https://borsh.io
[Build Status]: https://travis-ci.com/near/borsh-rs.svg?branch=master
[travis-ci]: https://travis-ci.com/near/borsh-rs
[Latest Version]: https://img.shields.io/crates/v/borsh.svg
[crates.io]: https://crates.io/crates/borsh
[borsh: rustc 1.40+]: https://img.shields.io/badge/rustc-1.40+-lightgray.svg
[Rust 1.40]: https://blog.rust-lang.org/2019/12/19/Rust-1.40.0.html
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
use borsh::{BorshSerialize, BorshDeserialize};

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
    let decoded_a = A::try_from_slice(&encoded_a).unwrap();
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

## Releasing

After you merged your change into the master branch and bumped the versions of all three crates it is time to officially release the new version.

Make sure `borsh`, `borsh-derive`, `borsh-derive-internal`, and `borsh-schema-derive-internal` all have the new crate versions. Then run the `publish.sh` script:

```bash
sh publish.sh
```

Make sure you are on the master branch, then tag the code and push the tag:

```bash
git tag -a v9.9.9 -m "My superawesome change."
git push origin v9.9.9
```

## License

This repository is distributed under the terms of both the MIT license and the Apache License (Version 2.0).
See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
