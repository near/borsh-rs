#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "schema")]

#[cfg(feature = "std")]
use std::collections::BTreeMap;

#[cfg(not(feature = "std"))]
use alloc::{
    borrow,
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use borsh::BorshSchema;

struct ThirdParty<K, V>(BTreeMap<K, V>);
mod third_party_impl {

    #[cfg(feature = "std")]
    use std::collections::BTreeMap;

    #[cfg(not(feature = "std"))]
    use alloc::{
        borrow,
        boxed::Box,
        collections::BTreeMap,
        string::{String, ToString},
        vec,
        vec::Vec,
    };
    use borsh::BorshSchema;

    pub(super) fn declaration<K: borsh::BorshSchema, V: borsh::BorshSchema>(
    ) -> borsh::schema::Declaration {
        let params = vec![<K>::declaration(), <V>::declaration()];
        format!(r#"{}<{}>"#, "ThirdParty", params.join(", "))
    }

    pub(super) fn add_definitions_recursively<K: borsh::BorshSchema, V: borsh::BorshSchema>(
        definitions: &mut BTreeMap<borsh::schema::Declaration, borsh::schema::Definition>,
    ) {
        let fields = borsh::schema::Fields::UnnamedFields(vec![
            <BTreeMap<K, V> as borsh::BorshSchema>::declaration(),
        ]);
        let definition = borsh::schema::Definition::Struct { fields };
        let no_recursion_flag = definitions.get(&declaration::<K, V>()).is_none();
        <() as BorshSchema>::add_definition(declaration::<K, V>(), definition, definitions);
        if no_recursion_flag {
            <BTreeMap<K, V> as borsh::BorshSchema>::add_definitions_recursively(definitions);
        }
    }
}

#[derive(BorshSchema)]
struct A<K, V> {
    #[borsh(schema(with(
        declaration = "third_party_impl::declaration::<K, V>",
        definitions = "third_party_impl::add_definitions_recursively::<K, V>"
    )))]
    x: ThirdParty<K, V>,
    y: u64,
}
