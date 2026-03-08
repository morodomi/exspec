#[should_panic(expected = "division by zero")]
#[test]
fn test_divide_by_zero_reversed() {
    divide(1, 0);
}
