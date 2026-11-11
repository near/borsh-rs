//! The `extra(n)` charges for `decode_seq` (`n > 0`) and the `Rc`/`Arc` block. `None` means
//! an overflow or an invalid layout. Tuples mirror std's private node types: that matches
//! current rustc, but is not a language guarantee.

use core::alloc::Layout;
use core::mem::size_of;

fn allocations<T>(n: usize) -> Option<usize> {
    n.checked_mul(size_of::<T>())
}

/// For collections that allocate nothing besides the decoded vector.
#[inline]
pub(super) fn none(_n: usize) -> Option<usize> {
    Some(0)
}

/// Upper bound for a hashbrown table with `n` entries of type `E`.
/// A 16-byte control group covers all supported hashbrown versions, including std's.
#[cfg(hash_collections)]
pub(super) fn table<E>(n: usize) -> Option<usize> {
    let buckets = (n.checked_mul(8)? / 7).checked_next_power_of_two()?.max(16);
    let size = buckets
        .checked_mul(size_of::<E>().checked_add(1)?)?
        .checked_add(16)?;
    Layout::from_size_align(size, core::mem::align_of::<E>().max(16))
        .ok()
        .map(|layout| layout.size())
}

/// Stable-sort scratch for `BTreeMap`/`BTreeSet` entries. For `n ≤ 20`, only 16-bit and
/// size-optimized std builds allocate (`n/2` elements). Above, driftsort takes at most
/// `max(n, 48)` elements; the older merge sort `n/2` plus run buffers of at most 224 `usize`s.
fn sort_scratch<E>(n: usize) -> Option<usize> {
    if n < 2 {
        Some(0)
    } else if n <= 20 {
        Layout::array::<E>(n / 2).ok().map(|layout| layout.size())
    } else {
        let buffer = Layout::array::<E>(n.max(48)).ok()?.size();
        let runs = Layout::array::<usize>(224).ok()?.size();
        buffer.checked_add(runs)
    }
}

/// Bulk-built `BTreeMap<K, V>` nodes plus sort scratch; sets use `V = ()`.
pub(super) fn btree<K, V>(n: usize) -> Option<usize> {
    type Leaf<K, V> = (usize, u16, u16, [K; 11], [V; 11]);
    let leaf = Layout::new::<Leaf<K, V>>();
    let internal = leaf
        .extend(Layout::new::<[usize; 12]>())
        .ok()?
        .0
        .pad_to_align()
        .size();

    let leaves = n / 12 + 1;
    let mut internals = 0usize;
    let mut level = leaves;
    while level > 1 {
        level = level.div_ceil(12);
        internals += level;
    }

    let nodes = allocations::<Leaf<K, V>>(leaves)?.checked_add(internals.checked_mul(internal)?)?;
    sort_scratch::<(K, V)>(n)?.checked_add(nodes)
}

pub(super) fn linked_list<T>(n: usize) -> Option<usize> {
    allocations::<(usize, usize, T)>(n)
}

/// The block `Rc`/`Arc` allocate: two counts of type `C`, then the value.
#[cfg(feature = "rc")]
pub(super) fn rc_block<C>(value: Layout) -> Option<usize> {
    Some(
        Layout::new::<[C; 2]>()
            .extend(value)
            .ok()?
            .0
            .pad_to_align()
            .size(),
    )
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use super::*;

    #[test]
    #[cfg(hash_collections)]
    fn table_values() {
        assert_eq!(table::<(u64, u64)>(1), Some(16 * 17 + 16));
        assert_eq!(table::<(u64, u64)>(14), Some(16 * 17 + 16));
        assert_eq!(table::<(u64, u64)>(15), Some(32 * 17 + 16));
        assert_eq!(table::<(u64, u64)>(1000), Some(2048 * 17 + 16));
        assert_eq!(table::<u8>(1000), Some(2048 * 2 + 16));
        assert_eq!(table::<u8>(usize::MAX), None);
        // 2^31 buckets of 3·2^31 + 1 bytes fit in `usize` but exceed `isize::MAX`.
        assert_eq!(table::<[u8; 3 << 31]>(1 << 30), None);
    }

    #[test]
    fn sort_scratch_values() {
        assert_eq!(sort_scratch::<(u64, u64)>(0), Some(0));
        assert_eq!(sort_scratch::<(u64, u64)>(1), Some(0));
        assert_eq!(sort_scratch::<(u64, u64)>(2), Some(16));
        assert_eq!(sort_scratch::<(u64, u64)>(3), Some(16));
        assert_eq!(sort_scratch::<(u64, u64)>(20), Some(160));
        assert_eq!(sort_scratch::<(u64, u64)>(21), Some(48 * 16 + 224 * 8));
        assert_eq!(sort_scratch::<(u64, u64)>(1000), Some(1000 * 16 + 224 * 8));
        assert_eq!(sort_scratch::<u8>(usize::MAX), None);
    }

    #[test]
    fn btree_values() {
        // Leaf: 8 + 2 + 2 + 88 + 88 = 188, padded to 192. Internal: 192 + 96 = 288.
        let (leaf, internal) = (192, 288);
        assert_eq!(btree::<u64, u64>(1), Some(leaf));
        assert_eq!(btree::<u64, u64>(11), Some(5 * 16 + leaf));
        assert_eq!(btree::<u64, u64>(12), Some(6 * 16 + 2 * leaf + internal));
        assert_eq!(
            btree::<u64, u64>(24),
            Some(48 * 16 + 1792 + 3 * leaf + internal)
        );
        assert_eq!(
            btree::<u64, u64>(143),
            Some(143 * 16 + 1792 + 12 * leaf + internal)
        );
        assert_eq!(
            btree::<u64, u64>(144),
            Some(144 * 16 + 1792 + 13 * leaf + 3 * internal)
        );
        assert_eq!(
            btree::<u64, u64>(1000),
            Some(1000 * 16 + 1792 + 84 * leaf + 8 * internal)
        );
        // Leaf: 8 + 2 + 2 + 88 = 100, padded to 104.
        assert_eq!(btree::<u64, ()>(1), Some(104));
        assert_eq!(btree::<u64, ()>(12), Some(6 * 8 + 2 * 104 + 200));
        assert_eq!(btree::<u8, u8>(usize::MAX), None);
    }

    #[test]
    fn linked_list_values() {
        assert_eq!(linked_list::<u64>(100), Some(2400));
        assert_eq!(linked_list::<u8>(100), Some(2400));
        assert_eq!(linked_list::<u128>(1), Some(32));
        assert_eq!(linked_list::<u64>(usize::MAX), None);
    }

    #[test]
    #[cfg(feature = "rc")]
    fn rc_block_values() {
        use core::sync::atomic::AtomicUsize;
        assert_eq!(rc_block::<usize>(Layout::new::<u8>()), Some(24));
        assert_eq!(rc_block::<AtomicUsize>(Layout::new::<u8>()), Some(24));
        assert_eq!(rc_block::<usize>(Layout::new::<()>()), Some(16));
        assert_eq!(
            rc_block::<AtomicUsize>(Layout::array::<u8>(5001).unwrap()),
            Some(5024)
        );
        #[repr(align(64))]
        struct Aligned64(#[allow(dead_code)] u8);
        assert_eq!(rc_block::<usize>(Layout::new::<Aligned64>()), Some(128));
        assert_eq!(
            rc_block::<usize>(Layout::array::<Aligned64>(0).unwrap()),
            Some(64)
        );
    }
}
