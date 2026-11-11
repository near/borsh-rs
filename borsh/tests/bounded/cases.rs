//! Inputs with known charges, shared by the decoding tests and `tests/bounded_allocations.rs`.
//!
//! The formulas below are written out independently of the implementation, from the
//! charging table in `docs/rustdoc_include/borsh_bounded_charges.md`.

#![allow(dead_code)]

use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet, LinkedList, VecDeque},
    format,
    string::String,
    vec,
    vec::Vec,
};
use core::mem::size_of;

#[cfg(feature = "hashbrown")]
use hashbrown::{HashMap, HashSet};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};

use borsh::io::Result;
use borsh::{
    from_slice, from_slice_bounded, to_vec, BorshDeserialize, BorshDeserializeBounded,
    BorshSerialize, Budget,
};

pub const MIB: usize = 1024 * 1024;

/// How `bounded_allocations.rs` compares the requested bytes with `allocations`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alloc {
    /// Equal to `allocations`.
    Exact,
    /// `Exact` with 16-byte control groups and `entries >= 8`, otherwise `UpperBound`.
    HashTable { entries: usize },
    /// At most `allocations`.
    UpperBound,
}

pub struct Case {
    pub name: String,
    pub input: Vec<u8>,
    /// `size_of::<T>()` plus `allocations`.
    pub charge: usize,
    pub allocations: usize,
    /// `(requested, remaining)` of the limit error at limit `charge - 1`. `None` when the
    /// charge is 0.
    pub below: Option<(usize, usize)>,
    pub alloc: Alloc,
    /// Decodes `input` against `budget`, failing on trailing bytes.
    pub decode: fn(&[u8], &mut Budget) -> Result<()>,
    pub decode_plain: fn(&[u8]) -> Result<()>,
}

pub fn decode<T: BorshDeserializeBounded>(input: &[u8], budget: &mut Budget) -> Result<()> {
    from_slice_bounded::<T>(input, budget).map(drop)
}

fn decode_plain<T: BorshDeserialize>(input: &[u8]) -> Result<()> {
    from_slice::<T>(input).map(drop)
}

fn case<T: BorshSerialize + BorshDeserialize + BorshDeserializeBounded>(
    name: String,
    value: &T,
    allocations: usize,
    below: Option<(usize, usize)>,
    alloc: Alloc,
) -> Case {
    case_input::<T>(name, to_vec(value).unwrap(), allocations, below, alloc)
}

fn case_input<T: BorshDeserialize + BorshDeserializeBounded>(
    name: String,
    input: Vec<u8>,
    allocations: usize,
    below: Option<(usize, usize)>,
    alloc: Alloc,
) -> Case {
    Case {
        name,
        input,
        charge: size_of::<T>() + allocations,
        allocations,
        below,
        alloc,
        decode: decode::<T>,
        decode_plain: decode_plain::<T>,
    }
}

/// The failure of a case whose last charge, of `last` bytes, does not fit.
fn single(last: usize) -> Option<(usize, usize)> {
    if last == 0 {
        None
    } else {
        Some((last, last - 1))
    }
}

/// The failure of a collection of `n` elements whose last charge is `extra`.
fn seq_below<T>(n: usize, extra: usize) -> Option<(usize, usize)> {
    single(if n == 0 { size_of::<T>() } else { extra })
}

/// A hashbrown table of `n` entries of `entry` bytes.
pub fn table(entry: usize, n: usize) -> usize {
    let buckets = core::cmp::max(16, (n * 8 / 7).next_power_of_two());
    buckets * (entry + 1) + 16
}

/// Stable sort scratch for `n` elements of `entry` bytes.
pub fn sort_scratch(entry: usize, n: usize) -> usize {
    match n {
        0 | 1 => 0,
        2..=20 => n / 2 * entry,
        _ => core::cmp::max(n, 48) * entry + 224 * size_of::<usize>(),
    }
}

/// The nodes of a `BTreeMap<K, V>` bulk-built from `n` distinct entries.
pub fn btree_nodes<K, V>(n: usize) -> usize {
    let leaf = size_of::<(usize, u16, u16, [K; 11], [V; 11])>();
    let align = core::mem::align_of::<(usize, u16, u16, [K; 11], [V; 11])>()
        .max(core::mem::align_of::<usize>());
    let internal = (leaf + 12 * size_of::<usize>()).div_ceil(align) * align;
    let leaves = n / 12 + 1;
    let mut internals = 0;
    let mut level = leaves;
    while level > 1 {
        level = level.div_ceil(12);
        internals += level;
    }
    leaves * leaf + internals * internal
}

/// The `Rc`/`Arc` block: two counts of type `C`, then `size` bytes aligned to `align`.
pub fn rc_block<C>(size: usize, align: usize) -> usize {
    let counts = 2 * size_of::<C>();
    let align = align.max(core::mem::align_of::<C>());
    let offset = counts.div_ceil(align) * align;
    (offset + size).div_ceil(align) * align
}

/// Over-aligned, so `Rc` pads between its counts and the value.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(align(64))]
pub struct Aligned64(pub u8);

impl BorshSerialize for Aligned64 {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> Result<()> {
        self.0.serialize(writer)
    }
}

impl BorshDeserialize for Aligned64 {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> Result<Self> {
        u8::deserialize_reader(reader).map(Aligned64)
    }
}

impl BorshDeserializeBounded for Aligned64 {
    fn deserialize_bounded_alloc<R: borsh::io::Read>(
        reader: &mut R,
        budget: &mut Budget,
    ) -> Result<Self> {
        u8::deserialize_bounded_alloc(reader, budget).map(Aligned64)
    }
}

/// The collection sizes around the thresholds of the formulas.
pub const BOUNDARY_SIZES: [usize; 22] = [
    1, 2, 3, 4, 7, 8, 11, 12, 14, 15, 20, 21, 23, 24, 28, 29, 47, 48, 143, 144, 1000, 0,
];

pub fn cases() -> Vec<Case> {
    let p = size_of::<usize>();
    let mut cases = vec![
        case("()".into(), &(), 0, None, Alloc::Exact),
        case(
            "u64".into(),
            &7u64,
            0,
            single(size_of::<u64>()),
            Alloc::Exact,
        ),
        case(
            "(u8, bool, i128)".into(),
            &(7u8, true, -1i128),
            0,
            single(size_of::<(u8, bool, i128)>()),
            Alloc::Exact,
        ),
        case(
            "Option<u64>".into(),
            &Some(7u64),
            0,
            single(size_of::<Option<u64>>()),
            Alloc::Exact,
        ),
        case(
            "[u64; 4]".into(),
            &[7u64; 4],
            0,
            single(size_of::<[u64; 4]>()),
            Alloc::Exact,
        ),
        case(
            "[u8; 32]".into(),
            &[7u8; 32],
            0,
            single(size_of::<[u8; 32]>()),
            Alloc::Exact,
        ),
        case(
            "empty Vec<u64>".into(),
            &Vec::<u64>::new(),
            0,
            single(size_of::<Vec<u64>>()),
            Alloc::Exact,
        ),
        case(
            "Box<()>".into(),
            &Box::new(()),
            0,
            single(size_of::<Box<()>>()),
            Alloc::Exact,
        ),
        case(
            "Box<u64>".into(),
            &Box::new(7u64),
            8,
            single(8),
            Alloc::Exact,
        ),
        case(
            "Vec<u64> of 1000".into(),
            &vec![7u64; 1000],
            8000,
            single(8000),
            Alloc::Exact,
        ),
        case(
            "Vec<u8> of 1".into(),
            &vec![7u8],
            1,
            single(1),
            Alloc::Exact,
        ),
        case(
            "Vec<u8> of 1 MiB".into(),
            &vec![7u8; MIB],
            MIB,
            single(MIB),
            Alloc::Exact,
        ),
        case(
            "Vec<u8> of 1 MiB + 1".into(),
            &vec![7u8; MIB + 1],
            MIB + 1,
            single(MIB + 1),
            Alloc::Exact,
        ),
        case(
            "Vec<u8> of 2 MiB + 1".into(),
            &vec![7u8; 2 * MIB + 1],
            2 * MIB + 1,
            single(2 * MIB + 1),
            Alloc::Exact,
        ),
        case(
            "String of 5000 bytes".into(),
            &"borsh".repeat(1000),
            5000,
            single(5000),
            Alloc::Exact,
        ),
        case(
            "Vec<Vec<u8>> of 3 x 10 bytes".into(),
            &vec![vec![7u8; 10]; 3],
            3 * size_of::<Vec<u8>>() + 30,
            // The outer vector and two inner ones fit; the third does not.
            Some((10, 9)),
            Alloc::Exact,
        ),
        case(
            "VecDeque<u64> of 100".into(),
            &(0..100u64).collect::<VecDeque<_>>(),
            800,
            single(800),
            Alloc::Exact,
        ),
        case(
            "Box<[u64; 4]>".into(),
            &Box::new([7u64; 4]),
            32,
            single(32),
            Alloc::Exact,
        ),
        case(
            "Vec<Box<[u64; 4]>> of 100".into(),
            &vec![Box::new([7u64; 4]); 100],
            100 * p + 100 * 32,
            // The vector and 99 boxes fit; the last box does not.
            Some((32, 31)),
            Alloc::Exact,
        ),
        // A box of an unsized value reuses the buffer that was already charged.
        case(
            "Box<[u8]> of 5000".into(),
            &vec![7u8; 5000].into_boxed_slice(),
            5000,
            single(5000),
            Alloc::Exact,
        ),
        case(
            "Box<Vec<u8>> of 10".into(),
            &Box::new(vec![7u8; 10]),
            10 + size_of::<Vec<u8>>(),
            single(10),
            Alloc::Exact,
        ),
        case(
            "empty Box<Vec<u8>>".into(),
            &Box::new(Vec::<u8>::new()),
            size_of::<Vec<u8>>(),
            single(size_of::<Vec<u8>>()),
            Alloc::Exact,
        ),
        case(
            "Cow<[u8]> of 10".into(),
            &alloc::borrow::Cow::<[u8]>::Owned(vec![7; 10]),
            10,
            single(10),
            Alloc::Exact,
        ),
        case(
            "LinkedList<u8> of 3".into(),
            &(0..3u8).collect::<LinkedList<_>>(),
            3 + 3 * size_of::<(usize, usize, u8)>(),
            single(3 * size_of::<(usize, usize, u8)>()),
            Alloc::Exact,
        ),
    ];

    for n in BOUNDARY_SIZES {
        let list = (0..n as u64).collect::<LinkedList<_>>();
        let extra = n * size_of::<(usize, usize, u64)>();
        cases.push(case(
            format!("LinkedList<u64> of {}", n),
            &list,
            n * 8 + extra,
            seq_below::<LinkedList<u64>>(n, extra),
            Alloc::Exact,
        ));

        let map = (0..n as u64)
            .map(|k| (k, k * 7))
            .collect::<BTreeMap<_, _>>();
        let extra = if n == 0 {
            0
        } else {
            sort_scratch(16, n) + btree_nodes::<u64, u64>(n)
        };
        cases.push(case(
            format!("BTreeMap<u64, u64> of {}", n),
            &map,
            n * 16 + extra,
            seq_below::<BTreeMap<u64, u64>>(n, extra),
            Alloc::UpperBound,
        ));

        let set = (0..n as u32).collect::<BTreeSet<_>>();
        let extra = if n == 0 {
            0
        } else {
            sort_scratch(4, n) + btree_nodes::<u32, ()>(n)
        };
        cases.push(case(
            format!("BTreeSet<u32> of {}", n),
            &set,
            n * 4 + extra,
            seq_below::<BTreeSet<u32>>(n, extra),
            Alloc::UpperBound,
        ));

        #[cfg(any(feature = "std", feature = "hashbrown"))]
        {
            let map = (0..n as u64).map(|k| (k, k * 7)).collect::<HashMap<_, _>>();
            let extra = if n == 0 { 0 } else { table(16, n) };
            cases.push(case(
                format!("HashMap<u64, u64> of {}", n),
                &map,
                n * 16 + extra,
                seq_below::<HashMap<u64, u64>>(n, extra),
                Alloc::HashTable { entries: n },
            ));

            let set = (0..n as u32).collect::<HashSet<_>>();
            let extra = if n == 0 { 0 } else { table(4, n) };
            cases.push(case(
                format!("HashSet<u32> of {}", n),
                &set,
                n * 4 + extra,
                seq_below::<HashSet<u32>>(n, extra),
                Alloc::HashTable { entries: n },
            ));
        }
    }

    // Hash tables reserve capacity for duplicate entries.
    #[cfg(all(
        not(feature = "de_strict_order"),
        any(feature = "std", feature = "hashbrown")
    ))]
    {
        let entries = [
            (1u64, 10u64),
            (1, 11),
            (2, 12),
            (3, 13),
            (4, 14),
            (5, 15),
            (6, 16),
            (7, 17),
            (8, 18),
        ];
        let n = entries.len();
        let extra = table(16, n);
        cases.push(case_input::<HashMap<u64, u64>>(
            "HashMap<u64, u64> with a duplicate key".into(),
            to_vec(&entries.to_vec()).unwrap(),
            n * 16 + extra,
            single(extra),
            Alloc::HashTable { entries: n },
        ));
    }

    #[cfg(feature = "rc")]
    {
        use alloc::{rc::Rc, sync::Arc};
        use core::sync::atomic::AtomicUsize;
        let rc = |size, align| rc_block::<usize>(size, align);
        let arc = |size, align| rc_block::<AtomicUsize>(size, align);
        cases.extend([
            case(
                "Rc<u8>".into(),
                &Rc::new(7u8),
                1 + rc(1, 1),
                // The box fits; the block does not.
                Some((rc(1, 1), rc(1, 1) - 1)),
                Alloc::Exact,
            ),
            case(
                "Rc<()>".into(),
                &Rc::new(()),
                rc(0, 1),
                single(rc(0, 1)),
                Alloc::Exact,
            ),
            case(
                "Rc<Aligned64>".into(),
                &Rc::new(Aligned64(7)),
                64 + rc(64, 64),
                Some((rc(64, 64), rc(64, 64) - 1)),
                Alloc::Exact,
            ),
            case(
                "empty Rc<[Aligned64]>".into(),
                &Rc::<[Aligned64]>::from(Vec::new()),
                rc(0, 64),
                single(rc(0, 64)),
                Alloc::Exact,
            ),
            case(
                "Arc<str> of 5001 bytes".into(),
                &Arc::<str>::from("a".repeat(5001)),
                5001 + arc(5001, 1),
                Some((arc(5001, 1), arc(5001, 1) - 1)),
                Alloc::Exact,
            ),
            case(
                "empty Arc<[u8]>".into(),
                &Arc::<[u8]>::from(Vec::new()),
                arc(0, 1),
                single(arc(0, 1)),
                Alloc::Exact,
            ),
            case(
                "Rc<[u64]> of 10".into(),
                &Rc::<[u64]>::from(vec![7u64; 10]),
                80 + rc(80, 8),
                Some((rc(80, 8), rc(80, 8) - 1)),
                Alloc::Exact,
            ),
            case(
                "Vec<Arc<u64>> of 10".into(),
                &(0..10).map(|_| Arc::new(7u64)).collect::<Vec<_>>(),
                10 * p + 10 * (8 + arc(8, 8)),
                Some((arc(8, 8), arc(8, 8) - 1)),
                Alloc::Exact,
            ),
        ]);
    }

    cases
}
