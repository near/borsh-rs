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

#[cfg(feature = "unstable__bounded")]
pub use bounded_helpers::*;

/// A no-op without `unstable__bounded`, so the fixtures can call it unconditionally.
#[cfg(not(feature = "unstable__bounded"))]
#[allow(unused)]
pub fn assert_bounded_parity<T>(_buf: &[u8]) {}

#[cfg(feature = "unstable__bounded")]
#[allow(unused)]
mod bounded_helpers {
    // Cargo also builds this file as a test target of its own, without `tests.rs`.
    extern crate alloc;

    use alloc::string::ToString;
    use alloc::{vec, vec::Vec};
    use core::fmt::{Debug, Display};

    use borsh::de::Budget;
    use borsh::io::{Error, ErrorKind, Read, Result};
    use borsh::{BorshDeserialize, BorshDeserializeBounded};

    /// The limit for the parity checks: more than any honest fixture charges.
    pub const PARITY_LIMIT: usize = 64 * 1024 * 1024;

    pub fn is_limit_exceeded(err: &Error) -> bool {
        limit_exceeded(err).is_some()
    }

    /// The `(limit, requested, remaining)` of a limit error. Without `std`, the numbers are
    /// parsed back from the message.
    pub fn limit_exceeded(err: &Error) -> Option<(usize, usize, usize)> {
        if err.kind() != ErrorKind::OutOfMemory {
            return None;
        }
        #[cfg(feature = "std")]
        {
            let exceeded = borsh::error::LimitExceeded::from_io_error(err)?;
            Some((exceeded.limit, exceeded.requested, exceeded.remaining))
        }
        #[cfg(not(feature = "std"))]
        {
            // No inner error to downcast, and the `#[non_exhaustive]` struct cannot be built here.
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

    pub struct OneByteReader<'a>(pub &'a [u8]);

    impl Read for OneByteReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            let len = buf.len().min(1);
            self.0.read(&mut buf[..len])
        }
    }

    #[track_caller]
    pub fn assert_same_outcome<T: PartialEq + Debug>(
        plain: &Result<T>,
        bounded: &Result<T>,
        context: impl Display,
    ) {
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

    /// Checks that bounded decoding under `PARITY_LIMIT` matches plain decoding on `buf` and on
    /// mutations of it, through both entry points and a reader that returns one byte per read.
    /// Errors must match by kind and message. Only where plain decoding fails may bounded
    /// decoding fail with a limit error instead, since a false length prefix can claim more
    /// than `PARITY_LIMIT` bytes.
    pub fn assert_bounded_parity<T>(buf: &[u8])
    where
        T: BorshDeserialize + BorshDeserializeBounded + PartialEq + Debug,
    {
        fn check<T: PartialEq + Debug>(
            input: &[u8],
            how: &str,
            plain: Result<T>,
            bounded: Result<T>,
        ) {
            match &bounded {
                Err(err) if plain.is_err() && is_limit_exceeded(err) => {}
                _ => assert_same_outcome(&plain, &bounded, format_args!("{} on {:?}", how, input)),
            }
        }

        let mut inputs = vec![buf.to_vec()];
        if !buf.is_empty() {
            inputs.push(buf[..buf.len() - 1].to_vec());
            inputs.push(buf[..buf.len() / 2].to_vec());
            for offset in [0, buf.len() / 2, buf.len() - 1] {
                let mut mutated = buf.to_vec();
                mutated[offset] = !mutated[offset];
                inputs.push(mutated);
            }
        }
        let mut appended = buf.to_vec();
        appended.push(0);
        inputs.push(appended);

        for input in &inputs {
            let plain = borsh::from_slice::<T>(input);
            let bounded = borsh::from_slice_bounded::<T>(input, &mut Budget::new(PARITY_LIMIT));
            check(input, "from_slice", plain, bounded);

            // No limit: a false length must not make the decoder allocate more than plain.
            let plain = borsh::from_slice::<T>(input);
            let bounded = borsh::from_slice_bounded::<T>(input, &mut Budget::new(usize::MAX));
            check(input, "from_slice without a limit", plain, bounded);

            let plain = borsh::from_reader::<_, T>(&mut &input[..]);
            let bounded =
                borsh::from_reader_bounded::<_, T>(&mut &input[..], &mut Budget::new(PARITY_LIMIT));
            check(input, "from_reader", plain, bounded);

            let plain = borsh::from_reader::<_, T>(&mut OneByteReader(input));
            let bounded = borsh::from_reader_bounded::<_, T>(
                &mut OneByteReader(input),
                &mut Budget::new(PARITY_LIMIT),
            );
            check(input, "one byte per read", plain, bounded);
        }
    }
}
