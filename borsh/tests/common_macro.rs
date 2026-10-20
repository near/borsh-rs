#[allow(unused)]
macro_rules! set_insert_deser_assert_macro [

    [$set: ident, $data: ident, $($key: expr),*] => [
        $($set.insert($key));*
        ;

        let $data = borsh::to_vec(&$set).unwrap();
        #[cfg(feature = "std")]
        insta::assert_debug_snapshot!($data);
    ]
];

#[allow(unused)]
macro_rules! map_insert_deser_assert_macro [

    [$map: ident, $data: ident, $($key: expr => $value: expr),*] => [
        $($map.insert($key, $value));*
        ;

        let $data = borsh::to_vec(&$map).unwrap();
        #[cfg(feature = "std")]
        insta::assert_debug_snapshot!($data);
    ]
];

#[allow(unused)]
macro_rules! set_wrong_order_test [

    [$test_name: ident, $set_type: ty] => [

        #[test]
        fn $test_name() {
            let mut data = vec![];
            let arr_key = ["various".to_string(), "foo".to_string(), "many".to_string()];
            let len = arr_key.len() as u32;
            u32::serialize(&len, &mut data).expect("no error");

            for key in &arr_key {
                key.serialize(&mut data).expect("no error");
            }

            let result = from_slice::<$set_type>(&data);

            #[cfg(not(feature = "de_strict_order"))]
            {
                let result = result.unwrap();
                assert_eq!(result.len(), arr_key.len());
                for key in &arr_key {
                    assert!(result.contains(key));

                }

            }

            #[cfg(feature = "de_strict_order")]
            {
                assert!(result.is_err());

                assert_eq!(result.unwrap_err().to_string(), ERROR_WRONG_ORDER_OF_KEYS);
            }
        }
    ]
];

#[allow(unused)]
macro_rules! map_wrong_order_test [

    [$test_name: ident, $map_type: ty] => [

        #[test]
        fn $test_name() {
            let mut data = vec![];
            let arr_key = ["various".to_string(), "foo".to_string(), "many".to_string()];
            let arr_val = [
                "value".to_string(),
                "different".to_string(),
                "unexp".to_string(),
            ];
            let len = arr_key.len() as u32;
            u32::serialize(&len, &mut data).expect("no error");

            let entries = IntoIterator::into_iter(arr_key.clone())
                .zip(IntoIterator::into_iter(arr_val))
                .collect::<Vec<_>>();

            for (key, value) in entries.clone() {
                key.serialize(&mut data).expect("no error");
                value.serialize(&mut data).expect("no error");
            }

            let result = from_slice::<$map_type>(&data);

            #[cfg(not(feature = "de_strict_order"))]
            {
                let result = result.unwrap();
                assert_eq!(result.len(), arr_key.len());
                for (key, value) in entries {
                    assert_eq!(result.get(&key), Some(&value));
                }
            }

            #[cfg(feature = "de_strict_order")]
            {
                assert!(result.is_err());

                assert_eq!(result.unwrap_err().to_string(), ERROR_WRONG_ORDER_OF_KEYS);
            }
        }
    ]
];

#[allow(unused)]
macro_rules! schema_map(
    () => { BTreeMap::new() };
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = BTreeMap::new();
            $(
                m.insert($key.to_string(), $value);
            )+
            m
        }
     };
);

#[allow(unused)]
#[cfg(feature = "unstable__schema")]
pub mod schema_imports {
    extern crate alloc;
    pub use alloc::{
        boxed::Box,
        collections::BTreeMap,
        format,
        string::{String, ToString},
        vec,
        vec::Vec,
    };

    pub use borsh::schema::{
        add_definition, BorshSchemaContainer, Declaration, Definition, Fields,
        SchemaContainerValidateError, SchemaMaxSerializedSizeError,
    };
    pub use borsh::{schema_container_of, BorshSchema};
}

#[cfg(bounded)]
pub use bounded_helpers::*;

/// A no-op without `unstable__bounded`, so the fixtures can call it unconditionally.
#[cfg(not(bounded))]
#[allow(unused)]
pub fn assert_bounded_parity<T>(_buf: &[u8]) {}

#[cfg(bounded)]
mod bounded_helpers {
    // Cargo also builds this file as a test target of its own, without `tests.rs`.
    extern crate alloc;

    /// The limit for the parity checks: more than any honest fixture charges.
    #[allow(unused)]
    pub const PARITY_LIMIT: usize = 64 * 1024 * 1024;

    #[allow(unused)]
    pub fn is_limit_exceeded(err: &borsh::io::Error) -> bool {
        limit_exceeded(err).is_some()
    }

    /// The `(limit, requested, remaining)` of a limit error. Without `std`, the numbers are
    /// parsed back from the message.
    #[allow(unused)]
    pub fn limit_exceeded(err: &borsh::io::Error) -> Option<(usize, usize, usize)> {
        if err.kind() != borsh::io::ErrorKind::OutOfMemory {
            return None;
        }
        #[cfg(feature = "std")]
        {
            let exceeded = err
                .get_ref()?
                .downcast_ref::<borsh::error::LimitExceeded>()?;
            Some((exceeded.limit, exceeded.requested, exceeded.remaining))
        }
        #[cfg(not(feature = "std"))]
        {
            use alloc::string::ToString;
            let message = err.to_string();
            let rest = message.strip_prefix("Memory limit exceeded: ")?;
            let (requested, rest) = rest.split_once(" bytes requested, ")?;
            let (remaining, rest) = rest.split_once(" of ")?;
            let limit = rest.strip_suffix(" bytes remaining")?;
            Some((
                limit.parse().ok()?,
                requested.parse().ok()?,
                remaining.parse().ok()?,
            ))
        }
    }

    #[allow(unused)]
    pub struct OneByteReader<'a>(pub &'a [u8]);

    impl borsh::io::Read for OneByteReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> borsh::io::Result<usize> {
            match (buf.first_mut(), self.0.split_first()) {
                (Some(out), Some((byte, rest))) => {
                    *out = *byte;
                    self.0 = rest;
                    Ok(1)
                }
                _ => Ok(0),
            }
        }
    }

    #[allow(unused)]
    #[track_caller]
    pub fn assert_same_outcome<T: PartialEq + core::fmt::Debug>(
        plain: &borsh::io::Result<T>,
        bounded: &borsh::io::Result<T>,
        context: impl core::fmt::Display,
    ) {
        use alloc::string::ToString;
        match (plain, bounded) {
            (Ok(plain), Ok(bounded)) => assert_eq!(plain, bounded, "{}", context),
            (Err(plain), Err(bounded)) => assert_eq!(
                (plain.kind(), plain.to_string()),
                (bounded.kind(), bounded.to_string()),
                "{}",
                context
            ),
            (plain, bounded) => panic!("{}: plain {:?}, bounded {:?}", context, plain, bounded),
        }
    }

    /// Checks parity under `PARITY_LIMIT` through both entry points, two readers, and mutations.
    /// Errors must match by kind and message, except that malformed inputs may exceed the
    /// limit because a false length prefix can claim more than `PARITY_LIMIT` bytes.
    #[allow(unused)]
    pub fn assert_bounded_parity<T>(buf: &[u8])
    where
        T: borsh::BorshDeserialize + borsh::BorshDeserializeBounded + PartialEq + core::fmt::Debug,
    {
        use alloc::vec::Vec;
        use borsh::de::Budget;

        fn check<T: PartialEq + core::fmt::Debug>(
            buf: &[u8],
            how: &str,
            plain: borsh::io::Result<T>,
            bounded: borsh::io::Result<T>,
            limit_error_allowed: bool,
        ) {
            match &bounded {
                Err(err) if limit_error_allowed && is_limit_exceeded(err) => {}
                _ => assert_same_outcome(&plain, &bounded, format_args!("{} on {:?}", how, buf)),
            }
        }

        let plain = borsh::from_slice::<T>(buf);
        let limit_error_allowed = plain.is_err();
        let bounded = borsh::from_slice_bounded::<T>(buf, &mut Budget::new(PARITY_LIMIT));
        check(buf, "from_slice", plain, bounded, limit_error_allowed);

        let plain = borsh::from_reader::<_, T>(&mut &buf[..]);
        let bounded =
            borsh::from_reader_bounded::<_, T>(&mut &buf[..], &mut Budget::new(PARITY_LIMIT));
        check(buf, "from_reader", plain, bounded, limit_error_allowed);

        let plain = borsh::from_reader::<_, T>(&mut OneByteReader(buf));
        let bounded = borsh::from_reader_bounded::<_, T>(
            &mut OneByteReader(buf),
            &mut Budget::new(PARITY_LIMIT),
        );
        check(
            buf,
            "one byte per read",
            plain,
            bounded,
            limit_error_allowed,
        );

        let mut mutations: Vec<Vec<u8>> = Vec::new();
        if !buf.is_empty() {
            mutations.push(buf[..buf.len() - 1].to_vec());
            mutations.push(buf[..buf.len() / 2].to_vec());
            for offset in [0, buf.len() / 2, buf.len() - 1] {
                let mut mutated = buf.to_vec();
                mutated[offset] = !mutated[offset];
                mutations.push(mutated);
            }
        }
        let mut appended = buf.to_vec();
        appended.push(0);
        mutations.push(appended);
        for mutated in &mutations {
            let plain = borsh::from_slice::<T>(mutated);
            let bounded = borsh::from_slice_bounded::<T>(mutated, &mut Budget::new(PARITY_LIMIT));
            check(mutated, "mutation", plain, bounded, true);
        }
    }
}
