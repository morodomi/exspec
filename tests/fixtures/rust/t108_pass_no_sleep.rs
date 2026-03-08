#[test]
fn test_no_waiting() {
    let result = compute(42);
    assert_eq!(result, 84);
}
