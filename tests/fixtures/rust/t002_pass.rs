#[test]
fn test_with_one_mock() {
    let mock_repo = MockRepo::new();
    assert_eq!(mock_repo.count(), 0);
}
