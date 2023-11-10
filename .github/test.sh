#!/usr/bin/env bash
set -e
set -x
export INSTA_UPDATE=no
pushd borsh
cargo test --no-run
cargo test
cargo test --features ascii
cargo test --features derive
cargo test --features unstable__schema
cargo test --test test_rc --features rc
cargo test --test test_hash_map --test test_btree_map --features de_strict_order

cargo test --no-default-features
cargo test --no-default-features --features ascii,unstable__schema
cargo test --no-default-features --features derive
cargo test --no-default-features --features unstable__schema
cargo test --no-default-features --test test_rc --features rc
cargo test --no-default-features --features hashbrown
popd
pushd borsh-derive
cargo test --features schema
