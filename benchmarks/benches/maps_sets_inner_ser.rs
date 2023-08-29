use std::{
    any::type_name,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    iter::FromIterator,
};

use borsh::{to_vec, BorshSerialize};

use benchmarks::Generate;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::SeedableRng;

fn ser_obj<T, U>(num_samples: usize, c: &mut Criterion)
where
    T: Generate + BorshSerialize + 'static,
    U: FromIterator<T> + BorshSerialize,
{
    let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);

    let group_name = format!("{}_{}", type_name::<U>(), num_samples);
    let mut group = c.benchmark_group(group_name);

    let collection: U = (0..num_samples).map(|_| T::generate(&mut rng)).collect();

    group.bench_with_input(BenchmarkId::new("borsh_ser", ""), &collection, |b, d| {
        b.iter(|| to_vec(d).unwrap());
    });
}

fn ser_string_hashmap_10(c: &mut Criterion) {
    ser_obj::<(String, String), HashMap<String, String>>(10, c);
}
fn ser_string_hashmap_1000(c: &mut Criterion) {
    ser_obj::<(String, String), HashMap<String, String>>(1000, c);
}

fn ser_string_hashmap_10000(c: &mut Criterion) {
    ser_obj::<(String, String), HashMap<String, String>>(10_000, c);
}

fn ser_string_btreemap_10(c: &mut Criterion) {
    ser_obj::<(String, String), BTreeMap<String, String>>(10, c);
}
fn ser_string_btreemap_1000(c: &mut Criterion) {
    ser_obj::<(String, String), BTreeMap<String, String>>(1000, c);
}

fn ser_string_btreemap_10000(c: &mut Criterion) {
    ser_obj::<(String, String), BTreeMap<String, String>>(10_000, c);
}

fn ser_string_hashset_10(c: &mut Criterion) {
    ser_obj::<String, HashSet<String>>(10, c);
}
fn ser_string_hashset_1000(c: &mut Criterion) {
    ser_obj::<String, HashSet<String>>(1000, c);
}

fn ser_string_hashset_10000(c: &mut Criterion) {
    ser_obj::<String, HashSet<String>>(10_000, c);
}

fn ser_string_btreeset_10(c: &mut Criterion) {
    ser_obj::<String, BTreeSet<String>>(10, c);
}
fn ser_string_btreeset_1000(c: &mut Criterion) {
    ser_obj::<String, BTreeSet<String>>(1000, c);
}

fn ser_string_btreeset_10000(c: &mut Criterion) {
    ser_obj::<String, BTreeSet<String>>(10_000, c);
}

criterion_group!(
    ser_string_map,
    ser_string_hashmap_10,
    ser_string_hashmap_1000,
    ser_string_hashmap_10000,
    ser_string_btreemap_10,
    ser_string_btreemap_1000,
    ser_string_btreemap_10000,
);

criterion_group!(
    ser_string_set,
    ser_string_hashset_10,
    ser_string_hashset_1000,
    ser_string_hashset_10000,
    ser_string_btreeset_10,
    ser_string_btreeset_1000,
    ser_string_btreeset_10000,
);

criterion_main!(ser_string_map, ser_string_set);
