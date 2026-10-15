//! A separate binary isolates the global allocator used to compare charges
//! with allocation requests.
#![cfg(feature = "de_limit")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::ErrorKind;

use borsh::from_slice_with_limit;

#[path = "memory_limit/cases.rs"]
mod cases;

use cases::{cases, LYING_LENGTH_PREFIX};

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static REQUESTED: Cell<usize> = const { Cell::new(0) };
}

/// Counts successful allocations and reallocation growth on this thread.
/// Frees and shrinking reallocations do not reduce the total.
struct CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// Hooks must not allocate or unwind; `try_with` tolerates TLS teardown.
fn record(bytes: usize) {
    let _ = COUNTING.try_with(|counting| {
        if counting.get() {
            let _ = REQUESTED
                .try_with(|requested| requested.set(requested.get().saturating_add(bytes)));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            record(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() {
            record(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() && new_size > layout.size() {
            record(new_size - layout.size());
        }
        new_ptr
    }
}

/// Counting stops on unwind as well as normal return.
fn requested_bytes<R>(f: impl FnOnce() -> R) -> (R, usize) {
    struct StopCounting;

    impl Drop for StopCounting {
        fn drop(&mut self) {
            COUNTING.set(false);
        }
    }

    assert!(!COUNTING.get(), "counting must not be nested");
    REQUESTED.set(0);
    COUNTING.set(true);
    let stop_counting = StopCounting;
    let result = f();
    drop(stop_counting);
    (result, REQUESTED.get())
}

#[test]
fn test_counting_allocator_counts_allocations_and_growth() {
    let (_, requested) = requested_bytes(|| {
        let mut buffer = Vec::<u8>::with_capacity(1000);
        buffer.reserve_exact(3000);
        buffer
    });
    assert_eq!(requested, 3000);
}

#[test]
fn test_limited_decode_requests_exactly_the_limit() {
    for case in cases() {
        let (result, requested) = requested_bytes(|| (case.decode)(&case.input, Some(case.limit)));
        result.unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        assert_eq!(requested, case.limit, "{}", case.name);
    }
}

#[test]
fn test_plain_decode_requests_the_same_bytes() {
    for case in cases() {
        let (result, requested) = requested_bytes(|| (case.decode)(&case.input, None));
        result.unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        assert_eq!(requested, case.limit, "{}", case.name);
    }
}

#[test]
fn test_exceeded_limit_fails_before_allocating_the_buffer() {
    let limit = 64 * 1024;
    let (result, requested) =
        requested_bytes(|| from_slice_with_limit::<Vec<u8>>(&LYING_LENGTH_PREFIX, limit));
    assert_eq!(result.unwrap_err().kind(), ErrorKind::OutOfMemory);
    // Far below the 1 MiB first buffer, so it was never allocated.
    assert!(requested < 1024, "requested {} bytes", requested);
}
