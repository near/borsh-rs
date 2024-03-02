#!/usr/bin/env bash
set -e
set -x
export INSTA_UPDATE=no
pushd borsh
cargo test --no-run
cargo test
cargo test --features ascii 'roundtrip::test_ascii_strings'
cargo test --features ascii 'deserialization_errors::test_ascii_strings'
cargo test --features ascii,unstable__schema 'schema::test_ascii_strings'
cargo test --features derive
cargo test --features unstable__schema
cargo test --features rc 'roundtrip::test_rc'
cargo test --features rc,unstable__schema 'schema::test_rc'
cargo test --features de_strict_order 'roundtrip::test_hash_map'
cargo test --features de_strict_order 'roundtrip::test_btree_map'
cargo test --features derive,bson 'roundtrip::requires_derive_category::test_bson_object_ids'
cargo test --features derive,bytes 'roundtrip::requires_derive_category::test_ultimate_many_features_combined'

cargo test --no-default-features
cargo test --no-default-features --features ascii 'roundtrip::test_ascii_strings'
cargo test --no-default-features --features ascii 'deserialization_errors::test_ascii_strings'
cargo test --no-default-features --features ascii,unstable__schema 'schema::test_ascii_strings'
cargo test --no-default-features --features derive
cargo test --no-default-features --features unstable__schema
cargo test --no-default-features --features rc 'roundtrip::test_rc'
cargo test --no-default-features --features rc,unstable__schema 'schema::test_rc'
cargo test --no-default-features --features hashbrown
popd
pushd borsh-derive
cargo test --features schema
