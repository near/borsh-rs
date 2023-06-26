#[allow(unused)]
macro_rules! set_insert_deser_assert_macro [

    [$set: ident, $data: ident, $($key: expr),*] => [
        let mut count = 0;
        $($set.insert($key); count += 1);*
        ;
        assert_eq!($set.len(), count);

        let $data = $set.try_to_vec().unwrap();
        #[cfg(feature = "std")]
        insta::assert_debug_snapshot!($data);
    ]
];

#[allow(unused)]
macro_rules! map_insert_deser_assert_macro [

    [$map: ident, $data: ident, $($key: expr => $value: expr),*] => [
        let mut count = 0;
        $($map.insert($key, $value); count += 1);*
        ;
        assert_eq!($map.len(), count);
        let $data = $map.try_to_vec().unwrap();
        #[cfg(feature = "std")]
        insta::assert_debug_snapshot!($data);
    ]
];
