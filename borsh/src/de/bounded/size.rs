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

/// Upper bound for a hashbrown table with `n` entries of type `E`, after hashbrown's
/// `capacity_to_buckets` and `TableLayout::calculate_layout_for`. 1/8 of the buckets stay
/// empty, so a table has `n·8/7` buckets rounded up to a power of two; under 15 entries it has
/// 4, 8 or 16, and 16 bounds those, padding included. Each bucket holds an `E` and a control
/// byte, and `GROUP_WIDTH` more control bytes follow the last one.
#[cfg(hash_collections)]
pub(super) fn table<E>(n: usize) -> Option<usize> {
    // The widest control group (SSE2): covers all supported hashbrown versions, including std's.
    const GROUP_WIDTH: usize = 16;
    let buckets = (n.checked_mul(8)? / 7).checked_next_power_of_two()?.max(16);
    let size = buckets
        .checked_mul(size_of::<E>().checked_add(1)?)?
        .checked_add(GROUP_WIDTH)?;
    // `buckets` is a power of two of at least 16, so the `E`s end on the control bytes'
    // alignment: no padding comes between.
    Layout::from_size_align(size, core::mem::align_of::<E>().max(GROUP_WIDTH))
        .ok()
        .map(|layout| layout.size())
}

/// The nodes of a bulk-built `BTreeMap<K, V>`; sets use `V = ()`. `Leaf` mirrors std's
/// `LeafNode` (parent pointer, parent index, length, keys, values); an internal node is a leaf
/// plus `2·B` edges. `bulk_push` fills each leaf with `CAPACITY` entries and puts the next one
/// in a parent, so `n` entries take `n / (CAPACITY + 1) + 1` leaves; each level above has one
/// node per `2·B` nodes below, up to a single root.
pub(super) fn btree<K, V>(n: usize) -> Option<usize> {
    // As in std's `btree::node`.
    const B: usize = 6;
    const CAPACITY: usize = 2 * B - 1;
    type Leaf<K, V> = (usize, u16, u16, [K; CAPACITY], [V; CAPACITY]);
    let leaf = Layout::new::<Leaf<K, V>>();
    let internal = leaf
        .extend(Layout::new::<[usize; 2 * B]>())
        .ok()?
        .0
        .pad_to_align()
        .size();

    let leaves = n / (CAPACITY + 1) + 1;
    let mut internals = 0usize;
    let mut level = leaves;
    while level > 1 {
        level = level.div_ceil(2 * B);
        internals += level;
    }

    allocations::<Leaf<K, V>>(leaves)?.checked_add(internals.checked_mul(internal)?)
}

pub(super) fn linked_list<T>(n: usize) -> Option<usize> {
    allocations::<(usize, usize, T)>(n)
}

/// What the `Rc`/`Arc` block adds to its value: two counts of type `C`, plus padding.
#[cfg(feature = "rc")]
pub(super) fn rc_overhead<C>(value: Layout) -> Option<usize> {
    let block = Layout::new::<[C; 2]>().extend(value).ok()?.0.pad_to_align();
    block.size().checked_sub(value.size())
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
