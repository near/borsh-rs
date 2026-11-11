//! Compares what bounded decoding charges with what it requests from the allocator.
//!
//! Its own test binary, as it installs a global allocator; it always links std.

extern crate alloc;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::mem::size_of;

use borsh::{
    from_slice, from_slice_bounded, to_vec, BorshDeserializeBounded, BorshSerialize, Budget,
};

#[path = "bounded/cases.rs"]
mod cases;
#[path = "common_macro.rs"]
mod common_macro;

use cases::{btree_nodes, cases, sort_scratch, Alloc};
use common_macro::{assert_same_outcome, limit_exceeded};

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static REQUESTED: Cell<usize> = const { Cell::new(0) };
    static REALLOCS: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// The hooks must not allocate or panic; `try_with` fails quietly during thread teardown.
fn counting() -> bool {
    COUNTING
        .try_with(|counting| counting.get())
        .unwrap_or(false)
}

fn record_bytes(bytes: usize) {
    if counting() {
        let _ =
            REQUESTED.try_with(|requested| requested.set(requested.get().saturating_add(bytes)));
    }
}

fn record_realloc() {
    if counting() {
        let _ = REALLOCS.try_with(|reallocs| reallocs.set(reallocs.get().saturating_add(1)));
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            record_bytes(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() {
            record_bytes(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() {
            record_realloc();
            record_bytes(new_size.saturating_sub(layout.size()));
        }
        new_ptr
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Allocations {
    /// Bytes of `alloc` and `alloc_zeroed`, plus the growth of every `realloc`.
    requested: usize,
    /// `realloc` calls, growing or shrinking.
    reallocs: usize,
}

const NO_ALLOCATIONS: Allocations = Allocations {
    requested: 0,
    reallocs: 0,
};

fn count_allocations<R>(f: impl FnOnce() -> R) -> (R, Allocations) {
    struct StopCounting;

    impl Drop for StopCounting {
        fn drop(&mut self) {
            COUNTING.set(false);
        }
    }

    assert!(!COUNTING.get(), "counting must not be nested");
    REQUESTED.set(0);
    REALLOCS.set(0);
    COUNTING.set(true);
    let stop_counting = StopCounting;
    let result = f();
    drop(stop_counting);
    let counted = Allocations {
        requested: REQUESTED.get(),
        reallocs: REALLOCS.get(),
    };
    (result, counted)
}

#[test]
fn test_counting_allocator() {
    let (buffer, counted) = count_allocations(|| {
        let mut buffer = Vec::<u8>::with_capacity(1000);
        buffer.reserve_exact(3000);
        buffer
    });
    assert_eq!(
        counted,
        Allocations {
            requested: 3000,
            reallocs: 1
        }
    );

    let (_, counted) = count_allocations(|| drop(buffer));
    assert_eq!(counted, NO_ALLOCATIONS);

    let (buffer, counted) = count_allocations(|| {
        let mut buffer = vec![0u8; 500];
        buffer.truncate(100);
        buffer.shrink_to_fit();
        buffer
    });
    assert_eq!(buffer.len(), 100);
    assert_eq!(
        counted,
        Allocations {
            requested: 500,
            reallocs: 1
        }
    );
}

#[test]
fn test_counting_allocator_ignores_other_threads() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let started = AtomicBool::new(false);
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while !started.load(Ordering::SeqCst) {
                std::hint::spin_loop();
            }
            drop(std::hint::black_box(vec![7u8; 1000]));
            done.store(true, Ordering::SeqCst);
        });
        // Atomics and spinning do not allocate, unlike most ways to wait.
        let ((), counted) = count_allocations(|| {
            started.store(true, Ordering::SeqCst);
            while !done.load(Ordering::SeqCst) {
                std::hint::spin_loop();
            }
        });
        assert_eq!(counted, NO_ALLOCATIONS);
    });
}

/// Whether hashbrown, also inside std, uses the 16-byte control groups `cases::table` assumes.
const SIXTEEN_BYTE_GROUPS: bool = cfg!(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(miri)
));

fn violation(
    alloc: Alloc,
    allocations: usize,
    counted: Allocations,
    plain: Allocations,
) -> Option<&'static str> {
    let equal = match alloc {
        Alloc::Exact => true,
        Alloc::HashTable { entries } => SIXTEEN_BYTE_GROUPS && entries >= 8,
        Alloc::UpperBound => false,
    };
    if counted.requested > allocations {
        Some("requested more than the allocations")
    } else if equal && counted.requested != allocations {
        Some("requested less than the allocations")
    } else if counted.reallocs > plain.reallocs {
        Some("reallocated more than plain decoding")
    } else {
        None
    }
}

#[test]
fn test_decoding_requests_what_it_charges() {
    let mut table = String::new();
    writeln!(
        table,
        "{:<40} {:<28} {:>11} {:>10} {:>8} {:>14}",
        "case", "alloc", "allocations", "requested", "reallocs", "plain reallocs"
    )
    .unwrap();
    let mut failures = Vec::new();

    for case in cases() {
        let decode = |limit| (case.decode)(&case.input, &mut Budget::from_bytes(limit));
        // A first, unmeasured run: building a hasher may allocate once, lazily.
        decode(case.charge).unwrap_or_else(|err| panic!("{}: {}", case.name, err));

        let (result, counted) = count_allocations(|| decode(case.charge));
        result.unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        let (result, unlimited) = count_allocations(|| decode(usize::MAX));
        result.unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        let (result, plain) = count_allocations(|| (case.decode_plain)(&case.input));
        result.unwrap_or_else(|err| panic!("{}: {}", case.name, err));

        writeln!(
            table,
            "{:<40} {:<28} {:>11} {:>10} {:>8} {:>14}",
            case.name,
            format!("{:?}", case.alloc),
            case.allocations,
            counted.requested,
            counted.reallocs,
            plain.reallocs
        )
        .unwrap();
        if let Some(violation) = violation(case.alloc, case.allocations, counted, plain) {
            failures.push(format!("{}: {}", case.name, violation));
        }
        if unlimited != counted {
            failures.push(format!(
                "{}: requested {:?} without a limit",
                case.name, unlimited
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "16-byte control groups: {}\n{}\n\n{}",
            SIXTEEN_BYTE_GROUPS,
            failures.join("\n"),
            table
        );
    }
}

/// What a limit error allocates; without `std` that depends on its numbers.
fn error_allocations(limit: usize, requested: usize, remaining: usize) -> Allocations {
    let mut budget = Budget::from_bytes(limit);
    budget.charge_bytes(limit - remaining).unwrap();
    let (result, counted) = count_allocations(|| {
        if requested == usize::MAX {
            // Unlike `charge_bytes(usize::MAX)`, fails even when `remaining` is `usize::MAX`.
            budget.charge_array::<u8>(usize::MAX)
        } else {
            budget.charge_bytes(requested)
        }
    });
    let err = result.unwrap_err();
    assert_eq!(limit_exceeded(&err), Some((limit, requested, remaining)));
    counted
}

/// Returns allocations excluding the error value, plus the error's `(requested, remaining)`.
#[track_caller]
fn decode_over_limit<T: BorshDeserializeBounded>(
    input: &[u8],
    limit: usize,
) -> (Allocations, (usize, usize)) {
    // Warm up lazy hasher allocations before measuring.
    let _ = from_slice_bounded::<T>(input, &mut Budget::from_bytes(limit));
    let (result, decode) = count_allocations(|| {
        from_slice_bounded::<T>(input, &mut Budget::from_bytes(limit)).map(|_value| ())
    });
    let err = match result {
        Ok(()) => panic!("decoded within a limit of {}", limit),
        Err(err) => err,
    };
    let (error_limit, requested, remaining) =
        limit_exceeded(&err).unwrap_or_else(|| panic!("not a limit error: {}", err));
    assert_eq!(error_limit, limit);

    let error = error_allocations(limit, requested, remaining);
    assert!(
        decode.requested >= error.requested && decode.reallocs >= error.reallocs,
        "the decode requested {:?}, less than its error alone, {:?}",
        decode,
        error
    );
    let before_error = Allocations {
        requested: decode.requested - error.requested,
        reallocs: decode.reallocs - error.reallocs,
    };
    (before_error, (requested, remaining))
}

/// `charge` is what the honest `value` charges beyond `size_of::<T>()`.
#[track_caller]
fn assert_collection_over_limit_allocates_nothing<T>(value: &T, charge: usize)
where
    T: BorshSerialize + BorshDeserializeBounded,
{
    let mut lying = to_vec(value).unwrap();
    lying[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    let limit = size_of::<T>() + charge;
    let (allocations, (requested, remaining)) = decode_over_limit::<T>(&lying, limit);
    assert_eq!((allocations, remaining), (NO_ALLOCATIONS, charge));
    assert!(requested > charge, "requested {}", requested);
}

#[test]
fn test_collection_over_limit_allocates_nothing() {
    let n = 1000;
    assert_collection_over_limit_allocates_nothing(&vec![7u8; n], n);
    assert_collection_over_limit_allocates_nothing(&vec![7u64; n], n * 8);
    assert_collection_over_limit_allocates_nothing(
        &(0..n as u64).map(|k| (k, k)).collect::<BTreeMap<_, _>>(),
        n * 16 + sort_scratch(16, n) + btree_nodes::<u64, u64>(n),
    );
}

#[cfg(hash_collections)]
#[test]
fn test_hash_collection_over_limit_allocates_nothing() {
    #[cfg(feature = "hashbrown")]
    use hashbrown::HashMap;
    #[cfg(feature = "std")]
    use std::collections::HashMap;

    use cases::table;

    let n = 1000;
    assert_collection_over_limit_allocates_nothing(
        &(0..n as u64).map(|k| (k, k)).collect::<HashMap<_, _>>(),
        n * 16 + table(16, n),
    );
}

fn replace_prefix(input: &mut [u8], offset: usize, expected: u32, prefix: u32) {
    assert_eq!(input[offset..offset + 4], expected.to_le_bytes());
    input[offset..offset + 4].copy_from_slice(&prefix.to_le_bytes());
}

#[test]
fn test_map_value_over_limit_allocates_only_the_entries_before_it() {
    let limit = 4096;
    // Only the entries vector and the first value exist when the second value fails.
    let entry = size_of::<(u64, Vec<u8>)>();
    let entries = 2 * entry;
    let charged = size_of::<BTreeMap<u64, Vec<u8>>>()
        + entries
        + sort_scratch(entry, 2)
        + btree_nodes::<u64, Vec<u8>>(2)
        + 3;
    let before = Allocations {
        requested: entries + 3,
        reallocs: 0,
    };

    let map = BTreeMap::from([(1u64, vec![7u8; 3]), (2, vec![8; 5])]);
    let mut input = to_vec(&map).unwrap();
    replace_prefix(&mut input, 4 + 8 + 4 + 3 + 8, 5, u32::MAX);
    let (allocations, (requested, remaining)) =
        decode_over_limit::<BTreeMap<u64, Vec<u8>>>(&input, limit);
    assert_eq!((allocations, remaining), (before, limit - charged));
    assert!(requested > remaining, "requested {}", requested);
}

#[test]
fn test_input_ending_at_a_growth_boundary_allocates_like_plain() {
    // Both decoders start with room for 512 `u64`s.
    let mut input = to_vec(&vec![7u64; 512]).unwrap();
    replace_prefix(&mut input, 0, 512, 1000);
    let (plain, plain_allocations) = count_allocations(|| from_slice::<Vec<u64>>(&input));
    let (bounded, allocations) = count_allocations(|| {
        from_slice_bounded::<Vec<u64>>(&input, &mut Budget::from_bytes(usize::MAX))
    });
    assert!(plain.is_err());
    assert_same_outcome(&plain, &bounded, "512 of 1000 u64s");
    assert_eq!(allocations, plain_allocations);
    assert_eq!(allocations.reallocs, 0);
}

#[test]
fn test_box_over_limit_is_never_allocated() {
    let input = to_vec(&[7u64; 512]).unwrap();
    assert_eq!(
        decode_over_limit::<Box<[u64; 512]>>(&input, 4095 + size_of::<Box<[u64; 512]>>()),
        (NO_ALLOCATIONS, (4096, 4095))
    );
}

#[cfg(feature = "rc")]
#[test]
fn test_rc_block_over_limit_is_never_allocated() {
    use std::sync::Arc;

    let input = to_vec(&[7u64; 512]).unwrap();
    // The 4096-byte box fits; the block it would be copied into does not.
    let boxed = Allocations {
        requested: 4096,
        reallocs: 0,
    };
    let block = cases::rc_block::<core::sync::atomic::AtomicUsize>(4096, 8);
    assert_eq!(
        decode_over_limit::<Arc<[u64; 512]>>(&input, 4096 + 100 + size_of::<Arc<[u64; 512]>>()),
        (boxed, (block, 100))
    );
}
