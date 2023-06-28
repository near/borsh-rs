use std::{
    any::type_name,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    iter::FromIterator,
};

use benchmarks::{Generate, PublicKey};
use borsh::{from_slice, BorshDeserialize, BorshSerialize};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::SeedableRng;

fn de_obj<T, U>(num_samples: usize, c: &mut Criterion)
where
    T: Generate + BorshDeserialize + BorshSerialize + 'static,
    U: FromIterator<T> + BorshDeserialize + BorshSerialize,
{
    let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);

    let group_name = format!("{}_{}", type_name::<U>(), num_samples);
    let mut group = c.benchmark_group(group_name);

    let collection: U = (0..num_samples).map(|_| T::generate(&mut rng)).collect();

    let serialized: Vec<u8> = collection.try_to_vec().unwrap();

    group.bench_with_input(BenchmarkId::new("borsh_de", ""), &serialized, |b, d| {
        b.iter(|| from_slice::<U>(d).unwrap());
    });
}
fn de_string_hashmap_10(c: &mut Criterion) {
    de_obj::<(String, String), HashMap<String, String>>(10, c);
}
fn de_string_hashmap_1000(c: &mut Criterion) {
    de_obj::<(String, String), HashMap<String, String>>(1000, c);
}

fn de_string_hashmap_10000(c: &mut Criterion) {
    de_obj::<(String, String), HashMap<String, String>>(10_000, c);
}

fn de_string_btreemap_10(c: &mut Criterion) {
    de_obj::<(String, String), BTreeMap<String, String>>(10, c);
}
fn de_string_btreemap_1000(c: &mut Criterion) {
    de_obj::<(String, String), BTreeMap<String, String>>(1000, c);
}

fn de_string_btreemap_10000(c: &mut Criterion) {
    de_obj::<(String, String), BTreeMap<String, String>>(10_000, c);
}

fn de_string_hashset_10(c: &mut Criterion) {
    de_obj::<String, HashSet<String>>(10, c);
}
fn de_string_hashset_1000(c: &mut Criterion) {
    de_obj::<String, HashSet<String>>(1000, c);
}

fn de_string_hashset_10000(c: &mut Criterion) {
    de_obj::<String, HashSet<String>>(10_000, c);
}

fn de_string_btreeset_10(c: &mut Criterion) {
    de_obj::<String, BTreeSet<String>>(10, c);
}
fn de_string_btreeset_1000(c: &mut Criterion) {
    de_obj::<String, BTreeSet<String>>(1000, c);
}

fn de_string_btreeset_10000(c: &mut Criterion) {
    de_obj::<String, BTreeSet<String>>(10_000, c);
}

criterion_group!(
    de_string_map,
    de_string_hashmap_10,
    de_string_hashmap_1000,
    de_string_hashmap_10000,
    de_string_btreemap_10,
    de_string_btreemap_1000,
    de_string_btreemap_10000,
);

criterion_group!(
    de_string_set,
    de_string_hashset_10,
    de_string_hashset_1000,
    de_string_hashset_10000,
    de_string_btreeset_10,
    de_string_btreeset_1000,
    de_string_btreeset_10000,
);
fn de_pubkey_hashmap_10(c: &mut Criterion) {
    de_obj::<(PublicKey, PublicKey), HashMap<PublicKey, PublicKey>>(10, c);
}
fn de_pubkey_hashmap_1000(c: &mut Criterion) {
    de_obj::<(PublicKey, PublicKey), HashMap<PublicKey, PublicKey>>(1000, c);
}

fn de_pubkey_hashmap_10000(c: &mut Criterion) {
    de_obj::<(PublicKey, PublicKey), HashMap<PublicKey, PublicKey>>(10_000, c);
}

fn de_pubkey_btreemap_10(c: &mut Criterion) {
    de_obj::<(PublicKey, PublicKey), BTreeMap<PublicKey, PublicKey>>(10, c);
}
fn de_pubkey_btreemap_1000(c: &mut Criterion) {
    de_obj::<(PublicKey, PublicKey), BTreeMap<PublicKey, PublicKey>>(1000, c);
}

fn de_pubkey_btreemap_10000(c: &mut Criterion) {
    de_obj::<(PublicKey, PublicKey), BTreeMap<PublicKey, PublicKey>>(10_000, c);
}

fn de_pubkey_hashset_10(c: &mut Criterion) {
    de_obj::<PublicKey, HashSet<PublicKey>>(10, c);
}
fn de_pubkey_hashset_1000(c: &mut Criterion) {
    de_obj::<PublicKey, HashSet<PublicKey>>(1000, c);
}

fn de_pubkey_hashset_10000(c: &mut Criterion) {
    de_obj::<PublicKey, HashSet<PublicKey>>(10_000, c);
}

fn de_pubkey_btreeset_10(c: &mut Criterion) {
    de_obj::<PublicKey, BTreeSet<PublicKey>>(10, c);
}
fn de_pubkey_btreeset_1000(c: &mut Criterion) {
    de_obj::<PublicKey, BTreeSet<PublicKey>>(1000, c);
}

fn de_pubkey_btreeset_10000(c: &mut Criterion) {
    de_obj::<PublicKey, BTreeSet<PublicKey>>(10_000, c);
}

criterion_group!(
    de_pubkey_map,
    de_pubkey_hashmap_10,
    de_pubkey_hashmap_1000,
    de_pubkey_hashmap_10000,
    de_pubkey_btreemap_10,
    de_pubkey_btreemap_1000,
    de_pubkey_btreemap_10000,
);

criterion_group!(
    de_pubkey_set,
    de_pubkey_hashset_10,
    de_pubkey_hashset_1000,
    de_pubkey_hashset_10000,
    de_pubkey_btreeset_10,
    de_pubkey_btreeset_1000,
    de_pubkey_btreeset_10000,
);

criterion_main!(de_string_map, de_string_set, de_pubkey_map, de_pubkey_set);
