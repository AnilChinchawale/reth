//! Benchmarks for XDPoS consensus
//!
//! Run with: `cargo bench -p reth-consensus-xdpos`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use reth_consensus_xdpos::{
    extra_data::V1ExtraData,
    reward::{RewardCalculator, RewardLog, BLOCK_REWARD},
    state_root_cache::XdcStateRootCache,
    tests::helpers::mock_qc,
    v2::verification::verify_qc,
    XDPoSConfig,
};
use alloy_primitives::{Address, B256, U256};
use std::collections::HashMap;
use std::time::Duration;

/// Benchmark QC signature verification with varying signature counts
fn bench_qc_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc_verification");

    // Create validators
    let validators: Vec<Address> = (0..18).map(|i| Address::with_last_byte(i as u8 + 1)).collect();

    // Benchmark with different signature counts
    for sig_count in [5, 10, 12, 18] {
        let qc = mock_qc(B256::random(), 100, 1000, sig_count);

        group.bench_with_input(
            BenchmarkId::from_parameter(sig_count),
            &sig_count,
            |b, _| {
                b.iter(|| {
                    let result = verify_qc(black_box(&qc), black_box(&validators), black_box(None));
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark reward calculation with varying signer counts
fn bench_reward_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("reward_calculation");

    let config = XDPoSConfig {
        reward: BLOCK_REWARD,
        reward_checkpoint: 900,
        ..Default::default()
    };
    let calculator = RewardCalculator::new(config);

    // Benchmark with different signer counts
    for signer_count in [5, 10, 18, 50, 100] {
        // Create signer logs
        let mut signer_logs = HashMap::new();
        for i in 0..signer_count {
            signer_logs.insert(
                Address::with_last_byte((i % 256) as u8),
                RewardLog {
                    sign_count: (i as u64 + 1) * 5, // Varying sign counts
                    reward: U256::ZERO,
                },
            );
        }
        let total_signs = signer_logs.values().map(|l| l.sign_count).sum();

        group.bench_with_input(
            BenchmarkId::from_parameter(signer_count),
            &signer_count,
            |b, _| {
                let mut logs = signer_logs.clone();
                b.iter(|| {
                    let result = calculator
                        .calculate_rewards_per_signer(black_box(&mut logs), black_box(total_signs));
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark state root cache lookup with varying cache sizes
fn bench_cache_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_lookup");

    // Benchmark with different cache sizes
    for cache_size in [100, 1000, 10000] {
        let cache = XdcStateRootCache::new(
            cache_size * 2,
            Duration::from_secs(3600),
            None, // No persistence for benchmark
        );

        // Populate cache
        let mut keys = Vec::new();
        for i in 0..cache_size {
            let block_hash = B256::random();
            let state_root = B256::random();
            cache.insert((i + 1) * 900, block_hash, state_root);
            keys.push(block_hash);
        }

        // Benchmark lookups (50% hit rate)
        group.throughput(Throughput::Elements(cache_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}", cache_size)),
            &cache_size,
            |b, _| {
                let mut i = 0;
                b.iter(|| {
                    // Look up every other key to get ~50% hit rate
                    let key = keys[i % keys.len()];
                    let result = cache.get(black_box(&key));
                    black_box(result);
                    i += 1;
                });
            },
        );
    }

    group.finish();
}

/// Benchmark extra data parsing
fn bench_extra_data_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("extra_data_parsing");

    // Create extra data with various validator counts
    for validator_count in [0, 5, 18, 50] {
        let mut data = vec![0u8; 32]; // Vanity

        // Add validators for checkpoint
        for i in 0..validator_count {
            data.extend_from_slice(Address::with_last_byte(i as u8).as_slice());
        }

        data.extend_from_slice(&[0u8; 65]); // Seal

        let data_ref: &[u8] = &data;
        group.bench_with_input(
            BenchmarkId::from_parameter(validator_count),
            &validator_count,
            |b, _| {
                b.iter(|| {
                    let result = V1ExtraData::parse(black_box(data_ref), black_box(validator_count > 0));
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cache insertion
fn bench_cache_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_insertion");

    let cache = XdcStateRootCache::new(
        10000,
        Duration::from_secs(3600),
        None,
    );

    group.bench_function("insert", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            let block_hash = B256::random();
            let state_root = B256::random();
            cache.insert(black_box(i * 900), black_box(block_hash), black_box(state_root));
        });
    });

    group.finish();
}

/// Benchmark hash without seal computation
fn bench_hash_without_seal(c: &mut Criterion) {
    use reth_consensus_xdpos::extra_data::hash_without_seal;
    use alloy_consensus::Header;

    let mut group = c.benchmark_group("hash_without_seal");

    let header = Header {
        number: 1000,
        timestamp: 1234567890,
        gas_limit: 8_000_000,
        extra_data: vec![0u8; 32 + 65].into(), // Vanity + Seal
        ..Default::default()
    };

    group.bench_function("compute", |b| {
        b.iter(|| {
            let result = hash_without_seal(black_box(&header));
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark proposer selection
fn bench_proposer_selection(c: &mut Criterion) {
    use reth_consensus_xdpos::v2::proposer::select_proposer;

    let mut group = c.benchmark_group("proposer_selection");

    // Test with different validator counts
    for validator_count in [10, 18, 50, 100] {
        let validators: Vec<Address> =
            (0..validator_count).map(|i| Address::with_last_byte((i % 256) as u8)).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(validator_count),
            &validator_count,
            |b, _| {
                let mut round = 0u64;
                b.iter(|| {
                    round += 1;
                    let result = select_proposer(black_box(round), black_box(&validators));
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark reward holder calculation
fn bench_holder_reward_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("holder_reward_calculation");

    let config = XDPoSConfig {
        foundation_wallet: Address::with_last_byte(0xFF),
        reward: BLOCK_REWARD,
        ..Default::default()
    };
    let calculator = RewardCalculator::new(config);

    let owner = Address::with_last_byte(1);
    let signer_reward = U256::from(BLOCK_REWARD);

    group.bench_function("calculate", |b| {
        b.iter(|| {
            let result = calculator.calculate_holder_rewards(black_box(owner), black_box(signer_reward));
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark signature deduplication
fn bench_signature_deduplication(c: &mut Criterion) {
    use reth_consensus_xdpos::v2::verification::unique_signatures;

    let mut group = c.benchmark_group("signature_deduplication");

    // Test with different signature counts and duplication ratios
    for total_sigs in [10, 50, 100] {
        let dup_ratio = 0.3; // 30% duplicates
        let unique_count = (total_sigs as f64 * (1.0 - dup_ratio)) as usize;

        let mut signatures: Vec<Vec<u8>> = (0..unique_count)
            .map(|i| {
                let mut sig = vec![0u8; 65];
                sig[0] = i as u8;
                sig
            })
            .collect();

        // Add duplicates
        for i in 0..(total_sigs - unique_count) {
            signatures.push(signatures[i % unique_count].clone());
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_sigs", total_sigs)),
            &total_sigs,
            |b, _| {
                b.iter(|| {
                    let (unique, dups) = unique_signatures(black_box(&signatures));
                    black_box((unique, dups));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_qc_verification,
    bench_reward_calculation,
    bench_cache_lookup,
    bench_extra_data_parsing,
    bench_cache_insertion,
    bench_hash_without_seal,
    bench_proposer_selection,
    bench_holder_reward_calculation,
    bench_signature_deduplication
);

criterion_main!(benches);
