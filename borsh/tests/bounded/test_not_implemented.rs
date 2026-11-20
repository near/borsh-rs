//! Compile-time checks: bounded impls and bounds mirror plain ones, with no fallback to plain.

use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::{BTreeMap, BTreeSet, BinaryHeap, LinkedList, VecDeque},
    rc::Rc,
    string::String,
    sync::Arc,
    vec::Vec,
};
use core::cell::{Cell, RefCell};
#[cfg(any(hash_collections, feature = "indexmap"))]
use core::hash::{BuildHasher, BuildHasherDefault, Hasher};
use core::marker::PhantomData;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use core::num::{
    NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize, NonZeroU128,
    NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize, Wrapping,
};
use core::ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};
use core::time::Duration;

#[cfg(feature = "hashbrown")]
use hashbrown::{HashMap, HashSet};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};

#[cfg(feature = "indexmap")]
use indexmap::{IndexMap, IndexSet};

use borsh::de::{EnumExt, EnumExtBounded};
use borsh::io::Read;
use borsh::{BorshDeserialize, BorshDeserializeBounded, Budget};

macro_rules! assert_impl {
    ($ty:ty: $tr:path) => {
        const _: fn() = || {
            fn check<T: ?Sized + $tr>() {}
            check::<$ty>();
        };
    };
}

/// Fails to compile if `$ty` implements `$tr`: the path to `some_item` then matches two
/// impls and the compiler cannot pick one (E0283).
macro_rules! assert_not_impl {
    ($ty:ty: $tr:path) => {
        const _: fn() = || {
            trait AmbiguousIfImpl<A> {
                fn some_item() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            #[allow(dead_code)]
            struct Invalid;
            impl<T: ?Sized + $tr> AmbiguousIfImpl<Invalid> for T {}
            let _ = <$ty as AmbiguousIfImpl<_>>::some_item;
        };
    };
}

macro_rules! assert_both {
    ($($ty:ty),+ $(,)?) => {
        $(
            assert_impl!($ty: BorshDeserialize);
            assert_impl!($ty: BorshDeserializeBounded);
        )+
    };
}

macro_rules! assert_plain_only {
    ($($ty:ty),+ $(,)?) => {
        $(
            assert_impl!($ty: BorshDeserialize);
            assert_not_impl!($ty: BorshDeserializeBounded);
        )+
    };
}

macro_rules! assert_bounded_only {
    ($($ty:ty),+ $(,)?) => {
        $(
            assert_not_impl!($ty: BorshDeserialize);
            assert_impl!($ty: BorshDeserializeBounded);
        )+
    };
}

macro_rules! assert_neither {
    ($($ty:ty),+ $(,)?) => {
        $(
            assert_not_impl!($ty: BorshDeserialize);
            assert_not_impl!($ty: BorshDeserializeBounded);
        )+
    };
}

/// Derives every trait the containers need, so only the bounded impl is missing.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PlainOnly(u8);

impl BorshDeserialize for PlainOnly {
    fn deserialize_reader<R: Read>(reader: &mut R) -> borsh::io::Result<Self> {
        u8::deserialize_reader(reader).map(PlainOnly)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct BoundedOnly(u8);

impl BorshDeserializeBounded for BoundedOnly {
    fn deserialize_bounded_alloc<R: Read>(
        reader: &mut R,
        budget: &mut Budget,
    ) -> borsh::io::Result<Self> {
        u8::deserialize_bounded_alloc(reader, budget).map(BoundedOnly)
    }
}

/// Omits `Clone`, `Copy` and `Ord` to test container bounds.
#[derive(PartialEq, Eq, Hash)]
struct Bare(u8);

impl BorshDeserialize for Bare {
    fn deserialize_reader<R: Read>(reader: &mut R) -> borsh::io::Result<Self> {
        u8::deserialize_reader(reader).map(Bare)
    }
}

impl BorshDeserializeBounded for Bare {
    fn deserialize_bounded_alloc<R: Read>(
        reader: &mut R,
        budget: &mut Budget,
    ) -> borsh::io::Result<Self> {
        u8::deserialize_bounded_alloc(reader, budget).map(Bare)
    }
}

#[cfg(any(hash_collections, feature = "indexmap"))]
#[derive(Default)]
struct ZeroHasher;

#[cfg(any(hash_collections, feature = "indexmap"))]
impl Hasher for ZeroHasher {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, _: &[u8]) {}
}

/// A `BuildHasher` without `Default`, which both decoders require.
#[cfg(any(hash_collections, feature = "indexmap"))]
#[allow(dead_code)]
struct NoDefaultState;

#[cfg(any(hash_collections, feature = "indexmap"))]
impl BuildHasher for NoDefaultState {
    type Hasher = ZeroHasher;

    fn build_hasher(&self) -> ZeroHasher {
        ZeroHasher
    }
}

assert_both!(u8, i8, i16, i32, i64, i128, u16, u32, u64, u128);
assert_both!(
    NonZeroI8,
    NonZeroI16,
    NonZeroI32,
    NonZeroI64,
    NonZeroI128,
    NonZeroU8,
    NonZeroU16,
    NonZeroU32,
    NonZeroU64,
    NonZeroU128,
    NonZeroUsize,
);
assert_both!(isize, usize, f32, f64, bool);

assert_both!(Option<u8>, Result<u8, String>);
assert_both!(String);
#[cfg(feature = "ascii")]
assert_both!(ascii::AsciiString, ascii::AsciiChar);
// Zero-sized elements and keys are rejected at run time by both decoders, not by bounds.
assert_both!(Vec<u8>, Vec<()>);
#[cfg(feature = "bytes")]
assert_both!(bytes::Bytes, bytes::BytesMut);
#[cfg(feature = "bson")]
assert_both!(bson::oid::ObjectId);
#[cfg(feature = "indexmap")]
assert_both!(
    IndexSet<u8>,
    IndexSet<u8, BuildHasherDefault<ZeroHasher>>,
    IndexMap<u8, u8>,
    IndexMap<u8, u8, BuildHasherDefault<ZeroHasher>>,
    IndexMap<(), u8>,
);
#[cfg(feature = "uuid")]
assert_both!(uuid::Uuid);
assert_both!(Cow<'static, [u8]>, Cow<'static, u8>);
assert_both!(VecDeque<u8>, LinkedList<u8>);

#[cfg(hash_collections)]
assert_both!(
    HashSet<u8>,
    HashSet<u8, BuildHasherDefault<ZeroHasher>>,
    HashMap<u8, u8>,
    HashMap<u8, u8, BuildHasherDefault<ZeroHasher>>,
    HashMap<(), u8>,
);

assert_both!(BTreeSet<u8>, BTreeMap<u8, u8>, BTreeMap<(), u8>);
assert_both!(
    SocketAddr,
    IpAddr,
    SocketAddrV4,
    SocketAddrV6,
    Ipv4Addr,
    Ipv6Addr
);

assert_both!(Box<u8>, Box<[u8]>);
assert_both!([u8; 0], [u8; 32]);

// Tuples are supported through arity 20.
assert_both!((), RangeFull, (u8,), (u8, String));
assert_both!((u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, String));
assert_both!(
    Range<u32>,
    RangeInclusive<u32>,
    RangeFrom<u32>,
    RangeTo<u32>,
    RangeToInclusive<u32>,
);

#[cfg(feature = "rc")]
assert_both!(Rc<u8>, Rc<[u8]>, Arc<u8>, Arc<[u8]>);

// `PhantomData` decodes nothing, so `T` needs neither impl.
assert_both!(PhantomData<[u8]>, PhantomData<BinaryHeap<u8>>);
assert_both!(Cell<u32>, RefCell<String>);

assert_neither!(BinaryHeap<u8>, NonZeroIsize, char, Wrapping<u8>, Duration);
assert_neither!((
    u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8
));

#[cfg(not(feature = "rc"))]
assert_neither!(Rc<u8>, Arc<u8>);

// `Declaration` is an alias of `String`.
#[cfg(feature = "unstable__schema")]
assert_plain_only!(
    borsh::schema::BorshSchemaContainer,
    borsh::schema::Definition,
    borsh::schema::Fields,
);

assert_plain_only!(
    PlainOnly,
    Option<PlainOnly>,
    Result<u8, PlainOnly>,
    Result<PlainOnly, u8>,
    Vec<PlainOnly>,
    Cow<'static, PlainOnly>,
    VecDeque<PlainOnly>,
    LinkedList<PlainOnly>,
    BTreeSet<PlainOnly>,
    BTreeMap<u8, PlainOnly>,
    BTreeMap<PlainOnly, u8>,
    Box<PlainOnly>,
    [PlainOnly; 0],
    (u8, PlainOnly),
    Range<PlainOnly>,
    Cell<PlainOnly>,
    RefCell<PlainOnly>,
);

#[cfg(hash_collections)]
assert_plain_only!(
    HashSet<PlainOnly>,
    HashMap<u8, PlainOnly>,
    HashMap<PlainOnly, u8>,
);

#[cfg(feature = "indexmap")]
assert_plain_only!(
    IndexSet<PlainOnly>,
    IndexMap<u8, PlainOnly>,
    IndexMap<PlainOnly, u8>,
);

#[cfg(feature = "rc")]
assert_plain_only!(Rc<PlainOnly>, Arc<PlainOnly>);

assert_bounded_only!(
    BoundedOnly,
    Option<BoundedOnly>,
    Result<u8, BoundedOnly>,
    Result<BoundedOnly, u8>,
    Vec<BoundedOnly>,
    Cow<'static, BoundedOnly>,
    VecDeque<BoundedOnly>,
    LinkedList<BoundedOnly>,
    BTreeSet<BoundedOnly>,
    BTreeMap<u8, BoundedOnly>,
    BTreeMap<BoundedOnly, u8>,
    Box<BoundedOnly>,
    [BoundedOnly; 0],
    (u8, BoundedOnly),
    Range<BoundedOnly>,
    Cell<BoundedOnly>,
    RefCell<BoundedOnly>,
);

#[cfg(hash_collections)]
assert_bounded_only!(
    HashSet<BoundedOnly>,
    HashMap<u8, BoundedOnly>,
    HashMap<BoundedOnly, u8>,
);

#[cfg(feature = "indexmap")]
assert_bounded_only!(
    IndexSet<BoundedOnly>,
    IndexMap<u8, BoundedOnly>,
    IndexMap<BoundedOnly, u8>,
);

#[cfg(feature = "rc")]
assert_bounded_only!(Rc<BoundedOnly>, Arc<BoundedOnly>);

assert_both!(
    Bare,
    Option<Bare>,
    Vec<Bare>,
    VecDeque<Bare>,
    LinkedList<Bare>,
    BTreeMap<u8, Bare>,
    [Bare; 2],
    (u8, Bare),
    Range<Bare>,
    RefCell<Bare>,
);
// `Box<T>` needs `T: Clone` through `ToOwned` (`Cow<'_, Bare>` is not even a valid
// type), `Cell` needs `Copy`, and set elements and map keys need `Ord`.
assert_neither!(
    Box<Bare>,
    Cell<Bare>,
    BTreeSet<Bare>,
    BTreeMap<Bare, u8>,
);

#[cfg(hash_collections)]
assert_both!(HashMap<u8, Bare>);
#[cfg(hash_collections)]
assert_neither!(HashSet<Bare>, HashMap<Bare, u8>);
#[cfg(hash_collections)]
assert_neither!(HashSet<u8, NoDefaultState>, HashMap<u8, u8, NoDefaultState>);

// Indexmap requires `Eq + Hash`, not `Ord`: `Bare` qualifies, but `f32` does not.
#[cfg(feature = "indexmap")]
assert_both!(IndexMap<u8, Bare>, IndexMap<Bare, u8>, IndexSet<Bare>);
#[cfg(feature = "indexmap")]
assert_neither!(
    IndexMap<f32, u8>,
    IndexSet<f32>,
    IndexMap<u8, u8, NoDefaultState>,
    IndexSet<u8, NoDefaultState>,
);

#[cfg(feature = "rc")]
assert_neither!(Rc<Bare>, Arc<Bare>);

assert_not_impl!(u8: EnumExt);
assert_not_impl!(u8: EnumExtBounded);
assert_not_impl!(Option<u8>: EnumExt);
assert_not_impl!(Option<u8>: EnumExtBounded);
assert_not_impl!(Result<u8, u8>: EnumExt);
assert_not_impl!(Result<u8, u8>: EnumExtBounded);

#[cfg(feature = "derive")]
mod derived {
    use core::marker::PhantomData;

    use borsh::de::{EnumExt, EnumExtBounded};
    use borsh::{BorshDeserialize, BorshDeserializeBounded};

    use super::{Bare, BoundedOnly, PlainOnly};

    #[allow(dead_code)]
    #[derive(Default)]
    struct DefaultOnly;

    /// A decoded type parameter gets a `BorshDeserializeBounded` bound, and nothing else.
    #[allow(dead_code)]
    #[derive(BorshDeserializeBounded)]
    struct Wrap<T>(T);

    assert_impl!(Wrap<Bare>: BorshDeserializeBounded);
    assert_bounded_only!(Wrap<BoundedOnly>);
    assert_neither!(Wrap<PlainOnly>);

    /// A skipped type parameter gets a `Default` bound instead.
    #[allow(dead_code)]
    #[derive(BorshDeserializeBounded)]
    struct Skipped<T>(#[borsh(skip)] T, u8);

    assert_impl!(Skipped<DefaultOnly>: BorshDeserializeBounded);
    assert_not_impl!(Skipped<BoundedOnly>: BorshDeserializeBounded);

    /// A type parameter only in `PhantomData` gets no bound.
    #[allow(dead_code)]
    #[derive(BorshDeserializeBounded)]
    struct Phantom<T>(PhantomData<T>, u8);

    assert_impl!(Phantom<PlainOnly>: BorshDeserializeBounded);

    /// `bound(deserialize_bounded = ...)` replaces the derived bound.
    #[allow(dead_code)]
    #[derive(BorshDeserializeBounded)]
    struct Overridden<T>(
        #[borsh(bound(deserialize_bounded = "T: BorshDeserializeBounded + Clone"))] T,
    );

    assert_impl!(Overridden<BoundedOnly>: BorshDeserializeBounded);
    assert_not_impl!(Overridden<Bare>: BorshDeserializeBounded);

    /// Each derive reads only its own `bound(...)` key.
    #[allow(dead_code)]
    #[derive(BorshDeserialize, BorshDeserializeBounded)]
    struct PlainKey<T>(#[borsh(bound(deserialize = "T: BorshDeserialize + Clone"))] T);

    assert_impl!(PlainKey<Bare>: BorshDeserializeBounded);
    assert_not_impl!(PlainKey<Bare>: BorshDeserialize);

    #[allow(dead_code)]
    #[derive(BorshDeserialize, BorshDeserializeBounded)]
    struct BoundedKey<T>(
        #[borsh(bound(deserialize_bounded = "T: BorshDeserializeBounded + Clone"))] T,
    );

    assert_impl!(BoundedKey<Bare>: BorshDeserialize);
    assert_not_impl!(BoundedKey<Bare>: BorshDeserializeBounded);

    /// Each derive implements its own trait only.
    #[allow(dead_code)]
    #[derive(BorshDeserialize)]
    struct PlainDerived(u8);

    #[allow(dead_code)]
    #[derive(BorshDeserializeBounded)]
    struct BoundedDerived(u8);

    assert_plain_only!(PlainDerived);
    assert_bounded_only!(BoundedDerived);

    /// Enums get `EnumExt` from the plain derive and `EnumExtBounded` from the bounded one.
    #[allow(dead_code)]
    #[derive(BorshDeserialize)]
    enum PlainEnum {
        A,
        B(u8),
    }

    #[allow(dead_code)]
    #[derive(BorshDeserializeBounded)]
    enum BoundedEnum {
        A,
        B(u8),
    }

    #[allow(dead_code)]
    #[derive(BorshDeserializeBounded)]
    enum WrapEnum<T> {
        A(T),
        B,
    }

    assert_plain_only!(PlainEnum);
    assert_impl!(PlainEnum: EnumExt);
    assert_not_impl!(PlainEnum: EnumExtBounded);

    assert_bounded_only!(BoundedEnum);
    assert_impl!(BoundedEnum: EnumExtBounded);
    assert_not_impl!(BoundedEnum: EnumExt);

    assert_impl!(WrapEnum<u8>: EnumExtBounded);
    assert_not_impl!(WrapEnum<PlainOnly>: EnumExtBounded);
}
