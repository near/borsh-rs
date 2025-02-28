use benchmarks::{Generate, ValidatorStake};
use borsh::BorshSerialize;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::SeedableRng;

fn ser_obj_length<T>(group_name: &str, num_samples: usize, c: &mut Criterion)
where
    for<'a> T: Generate + BorshSerialize + 'static,
{
    let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);
    let mut group = c.benchmark_group(group_name);

    let objects: Vec<_> = (0..num_samples).map(|_| T::generate(&mut rng)).collect();
    let borsh_datas: Vec<Vec<u8>> = objects.iter().map(|t| borsh::to_vec(t).unwrap()).collect();
    let borsh_sizes: Vec<_> = borsh_datas.iter().map(|d| d.len()).collect();

    for i in 0..objects.len() {
        let size = borsh_sizes[i];
        let obj = &objects[i];
        assert_eq!(
            borsh::to_vec(obj).unwrap().len(),
            borsh::object_length(obj).unwrap()
        );

        let benchmark_param_display = format!("idx={}; size={}", i, size);

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new(
                "borsh::to_vec(obj).unwrap().len()",
                benchmark_param_display.clone(),
            ),
            obj,
            |b, d| {
                b.iter(|| borsh::to_vec(d).unwrap().len());
            },
        );
        group.bench_with_input(
            BenchmarkId::new(
                "borsh::object_length(obj).unwrap()",
                benchmark_param_display.clone(),
            ),
            obj,
            |b, d| {
                b.iter(|| borsh::object_length(d).unwrap());
            },
        );
    }
    group.finish();
}
fn ser_length_validator_stake(c: &mut Criterion) {
    ser_obj_length::<ValidatorStake>("ser_account", 3, c);
}
criterion_group!(ser_length, ser_length_validator_stake,);
criterion_main!(ser_length);
