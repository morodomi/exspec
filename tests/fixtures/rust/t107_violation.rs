#[test]
fn test_multiple_asserts_no_messages() {
    assert_eq!(1 + 1, 2);
    assert_eq!(2 + 2, 4);
    assert!(true);
}
