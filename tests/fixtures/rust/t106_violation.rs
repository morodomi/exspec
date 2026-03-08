#[test]
fn test_duplicate_literals_in_assertions() {
    assert_eq!(calculate(1), 42);
    assert_eq!(calculate(2), 42);
    assert_eq!(calculate(3), 42);
}
