use std::str::FromStr;

use bigdecimal::BigDecimal;

use borsh::{BorshDeserialize, BorshSerialize};

macro_rules! test_big_decimal {
    ($test_name: ident, $num: expr) => {
        #[test]
        fn $test_name() {
            let buf = $num.try_to_vec().unwrap();
            let actual_num =
                <BigDecimal>::try_from_slice(&buf).expect("failed to deserialize BigDecimal");

            assert_eq!(actual_num, $num);
        }
    };
}

test_big_decimal!(test_zero, BigDecimal::from(0));
test_big_decimal!(test_666, BigDecimal::from(666));
test_big_decimal!(test_negative, BigDecimal::from(-42));
test_big_decimal!(test_big, BigDecimal::from_str(&"7".repeat(1024)).unwrap());
