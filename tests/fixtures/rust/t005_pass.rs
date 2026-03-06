use proptest::prelude::*;

proptest! {
    #[test]
    fn test_string_reverse_twice(s in ".*") {
        let reversed: String = s.chars().rev().collect();
        let double_reversed: String = reversed.chars().rev().collect();
        prop_assert_eq!(s, double_reversed);
    }
}
