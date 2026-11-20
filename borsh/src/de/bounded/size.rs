//! What `decode_seq` (`n > 0`) and `Rc`/`Arc` charge for the memory the value keeps. `None`
//! means an overflow or an invalid layout. Tuples mirror std's private node types: that matches
//! current rustc, but is not a language guarantee.

use core::alloc::Layout;
use core::mem::size_of;

fn allocations<T>(n: usize) -> Option<usize> {
    n.checked_mul(size_of::<T>())
}

/// The buffer of a `Vec<T>` of `n` elements.
#[inline]
pub(super) fn vec<T>(n: usize) -> Option<usize> {
    Layout::array::<T>(n).ok().map(|layout| layout.size())
}

/// Upper bound for a hashbrown table with `n` entries of type `E`.
/// A 16-byte control group covers all supported hashbrown versions, including std's.
#[cfg(any(hash_collections, feature = "indexmap"))]
pub(super) fn table<E>(n: usize) -> Option<usize> {
    let buckets = (n.checked_mul(8)? / 7).checked_next_power_of_two()?.max(16);
    let size = buckets
        .checked_mul(size_of::<E>().checked_add(1)?)?
        .checked_add(16)?;
    Layout::from_size_align(size, core::mem::align_of::<E>().max(16))
        .ok()
        .map(|layout| layout.size())
}

/// The nodes of a bulk-built `BTreeMap<K, V>`; sets use `V = ()`.
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

    allocations::<Leaf<K, V>>(leaves)?.checked_add(internals.checked_mul(internal)?)
}

pub(super) fn linked_list<T>(n: usize) -> Option<usize> {
    allocations::<(usize, usize, T)>(n)
}

/// `IndexMap<K, V>` entries and indices; sets use `V = ()`.
#[cfg(feature = "indexmap")]
pub(super) fn index_map<K, V>(n: usize) -> Option<usize> {
    vec::<(usize, K, V)>(n)?.checked_add(table::<usize>(n)?)
}

/// What the `Rc`/`Arc` block adds to its value: two counts of type `C`, plus padding.
#[cfg(feature = "rc")]
pub(super) fn rc_overhead<C>(value: Layout) -> Option<usize> {
    let block = Layout::new::<[C; 2]>().extend(value).ok()?.0.pad_to_align();
    Some(block.size() - value.size())
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use super::*;

    #[test]
    fn vec_values() {
        assert_eq!(vec::<u64>(1000), Some(8000));
        assert_eq!(vec::<u8>(usize::MAX), None);
    }

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
    fn btree_values() {
        // Leaf: 8 + 2 + 2 + 88 + 88 = 188, padded to 192. Internal: 192 + 96 = 288.
        let (leaf, internal) = (192, 288);
        assert_eq!(btree::<u64, u64>(1), Some(leaf));
        assert_eq!(btree::<u64, u64>(11), Some(leaf));
        assert_eq!(btree::<u64, u64>(12), Some(2 * leaf + internal));
        assert_eq!(btree::<u64, u64>(24), Some(3 * leaf + internal));
        assert_eq!(btree::<u64, u64>(143), Some(12 * leaf + internal));
        assert_eq!(btree::<u64, u64>(144), Some(13 * leaf + 3 * internal));
        assert_eq!(btree::<u64, u64>(1000), Some(84 * leaf + 8 * internal));
        // Leaf: 8 + 2 + 2 + 88 = 100, padded to 104.
        assert_eq!(btree::<u64, ()>(1), Some(104));
        assert_eq!(btree::<u64, ()>(12), Some(2 * 104 + 200));
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
    #[cfg(feature = "indexmap")]
    fn index_map_values() {
        assert_eq!(index_map::<u64, u64>(1000), Some(24_000 + 2048 * 9 + 16));
        assert_eq!(index_map::<u64, ()>(1000), Some(16_000 + 2048 * 9 + 16));
        // Entries are flat: `(usize, u128, u8)` is 32 bytes, `(usize, (u128, u8))` 48.
        assert_eq!(index_map::<u128, u8>(1), Some(32 + 16 * 9 + 16));
        assert_eq!(index_map::<u64, u64>(usize::MAX), None);
    }

    #[test]
    #[cfg(feature = "rc")]
    fn rc_overhead_values() {
        use core::sync::atomic::AtomicUsize;
        // Blocks of 24, 24, 16, 5024, 128 and 64 bytes.
        assert_eq!(rc_overhead::<usize>(Layout::new::<u8>()), Some(23));
        assert_eq!(rc_overhead::<AtomicUsize>(Layout::new::<u8>()), Some(23));
        assert_eq!(rc_overhead::<usize>(Layout::new::<()>()), Some(16));
        assert_eq!(
            rc_overhead::<AtomicUsize>(Layout::array::<u8>(5001).unwrap()),
            Some(23)
        );
        #[repr(align(64))]
        struct Aligned64(#[allow(dead_code)] u8);
        assert_eq!(rc_overhead::<usize>(Layout::new::<Aligned64>()), Some(64));
        assert_eq!(
            rc_overhead::<usize>(Layout::array::<Aligned64>(0).unwrap()),
            Some(64)
        );
    }
}
