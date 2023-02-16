use borsh::{BorshDeserialize, BorshSerialize};
use core::time::Duration;

macro_rules! test_duration {
    ($test_name: ident, $dur: expr) => {
        #[test]
        fn $test_name() {
            let buf = $dur.try_to_vec().unwrap();
            let actual_dur =
                <Duration>::try_from_slice(&buf).expect("failed to deserialize a duration");
            assert_eq!(actual_dur, $dur);
        }
    };
}

test_duration!(test_zero_duration, Duration::ZERO);
test_duration!(test_max_duration, Duration::MAX);
test_duration!(test_1_sec, Duration::from_secs(1));
test_duration!(test_1_nanos, Duration::from_nanos(1));
test_duration!(test_1_milis, Duration::from_millis(1));
test_duration!(test_1_day, Duration::from_secs(86400));
test_duration!(
    test_1_day_and_2_hour_and_3_min_and_3_nanos,
    Duration::new(93780, 3)
);
