#!/usr/bin/env bash

# NOTE: `cargo test [TESTNAME]` is used to filter only the submodule with tests to test a 
# specific feature or features' combination 
# e.g. `cargo test --features rc,unstable__schema 'schema::test_rc'` is used to test `BorshSchema`
# implementation of `std::rc::Rc` and `std::sync::Arc`, 
# where `[TESTNAME]` argument is set to `schema::test_rc`, which includes tests from `schema::test_rc`
# submodule of `borsh/tests/tests.rs` to be run. 
set -e
set -x
export INSTA_UPDATE=no
pushd borsh
############################ borsh `default-features = true` group #########################
########## general group
cargo test --no-run
cargo test
cargo test --features derive
cargo test --features derive,unstable__tokio
cargo test --features derive,unstable__async-std
cargo test --features unstable__schema
########## features = ["ascii"] group
cargo test --features ascii 'roundtrip::test_ascii_strings'
cargo test --features ascii 'deserialization_errors::test_ascii_strings'
cargo test --features ascii,unstable__schema 'schema::test_ascii_strings'
########## features = ["rc"] group
cargo test --features rc 'roundtrip::test_rc'
cargo test --features rc,unstable__schema 'schema::test_rc'
########## features = ["de_strict_order"] group
cargo test --features de_strict_order 'roundtrip::test_hash_map'
cargo test --features de_strict_order 'roundtrip::test_btree_map'
########## features = ["bson"] group
cargo test --features bson,derive 'roundtrip::requires_derive_category::test_bson_object_ids'
########## features = ["bytes"] group
cargo test --features bytes,derive 'roundtrip::requires_derive_category::test_ultimate_many_features_combined'


############################ borsh `default-features = false` group #########################
########## general group
cargo test --no-default-features
cargo test --no-default-features --features derive
cargo test --no-default-features --features unstable__schema
########## features = ["ascii"] group
cargo test --no-default-features --features ascii 'roundtrip::test_ascii_strings'
cargo test --no-default-features --features ascii 'deserialization_errors::test_ascii_strings'
cargo test --no-default-features --features ascii,unstable__schema 'schema::test_ascii_strings'
########## features = ["rc"] group
cargo test --no-default-features --features rc 'roundtrip::test_rc'
cargo test --no-default-features --features rc,unstable__schema 'schema::test_rc'
########## features = ["hashbrown"] group
cargo test --no-default-features --features hashbrown
cargo test --no-default-features --features hashbrown,derive
cargo test --no-default-features --features hashbrown,unstable__schema
popd
pushd borsh-derive
############################ borsh-derive group #########################
cargo test --features schema
