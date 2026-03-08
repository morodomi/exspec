#[test]
fn test_multiple_asserts_with_messages() {
    assert_eq!(1 + 1, 2, "addition of ones");
    assert_eq!(2 + 2, 4, "addition of twos");
    assert!(true, "always true");
}
