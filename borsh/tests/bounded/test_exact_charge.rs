use borsh::de::Budget;

use super::cases::cases;
use crate::common_macro::limit_exceeded;

#[test]
fn test_exact_charge_passes_and_one_byte_less_fails() {
    for case in cases() {
        let mut budget = Budget::new(case.charge);
        (case.decode)(&case.input, &mut budget)
            .unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        assert_eq!(budget.remaining(), 0, "{}", case.name);

        let mut budget = Budget::new(case.charge + 1000);
        (case.decode)(&case.input, &mut budget)
            .unwrap_or_else(|err| panic!("{}: {}", case.name, err));
        assert_eq!(budget.remaining(), 1000, "{}", case.name);

        match case.below {
            None => assert_eq!(case.charge, 0, "{}", case.name),
            Some((requested, remaining)) => {
                let limit = case.charge - 1;
                let mut budget = Budget::new(limit);
                let err = (case.decode)(&case.input, &mut budget).unwrap_err();
                assert_eq!(
                    limit_exceeded(&err),
                    Some((limit, requested, remaining)),
                    "{}: {}",
                    case.name,
                    err
                );
                assert_eq!(budget.remaining(), remaining, "{}", case.name);
            }
        }
    }
}
