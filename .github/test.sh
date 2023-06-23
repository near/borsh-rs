#!/usr/bin/env bash
set -e
set -x
pushd borsh
cargo test --no-run
cargo test
cargo test --no-default-features
cargo test --no-default-features --features hashbrown,rc
cargo test --features rc
cargo test --test test_hash_map --test test_btree_map --features de_strict_order
popd
cargo test --workspace
