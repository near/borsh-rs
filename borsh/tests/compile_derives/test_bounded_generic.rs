use alloc::{borrow::Cow, string::String};

#[cfg(feature = "hashbrown")]
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;

use borsh::BorshDeserializeBounded;

/// An empty override removes `K: Default` and `V: Default`; `HashMap` needs neither.
#[allow(dead_code)]
#[cfg(hash_collections)]
#[derive(BorshDeserializeBounded)]
struct G3<K, V, U>(
    #[borsh(skip, bound(deserialize_bounded = ""))] HashMap<K, V>,
    U,
);

trait TraitName {
    type Associated;
    #[allow(dead_code)]
    fn method(&self);
}

/// The derived bound is `T::Associated: BorshDeserializeBounded`, not one on `T`.
#[allow(dead_code)]
#[derive(BorshDeserializeBounded)]
struct Parametrized<T, V>
where
    T: TraitName,
{
    field: T::Associated,
    another: V,
}

/// The qualified path would get a bound on `T` itself; the override replaces it.
#[allow(dead_code)]
#[derive(BorshDeserializeBounded)]
struct ParametrizedOverride<T, V>
where
    T: TraitName,
{
    #[borsh(bound(
        deserialize_bounded = "<T as TraitName>::Associated: borsh::de::BorshDeserializeBounded"
    ))]
    field: <T as TraitName>::Associated,
    another: V,
}

/// Defaults are dropped from the impl's parameters.
#[allow(dead_code)]
#[derive(BorshDeserializeBounded)]
struct WithDefault<T = u8, const N: usize = 4> {
    x: T,
    y: [u8; N],
}

/// `[T]` is only `ToOwned` for `T: Clone`.
#[allow(dead_code)]
#[derive(BorshDeserializeBounded)]
struct WithLifetime<'a, T: Clone> {
    text: Cow<'a, str>,
    items: Cow<'a, [T]>,
}

/// Verifies that derived bounds are no stricter than required.
#[allow(dead_code)]
fn instantiations() {
    struct Nothing;

    struct NotDecodableAssociated;

    impl TraitName for NotDecodableAssociated {
        type Associated = String;
        fn method(&self) {}
    }

    fn check<T: BorshDeserializeBounded>() {}

    check::<Parametrized<NotDecodableAssociated, u8>>();
    check::<ParametrizedOverride<NotDecodableAssociated, u8>>();
    check::<WithDefault<String, 0>>();
    check::<WithLifetime<'static, u8>>();

    #[cfg(hash_collections)]
    check::<G3<Nothing, Nothing, u8>>();
}
