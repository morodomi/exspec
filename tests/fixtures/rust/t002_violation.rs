#[test]
fn test_with_too_many_mocks() {
    let mock_service_a = MockServiceA::new();
    let mock_service_b = MockServiceB::new();
    let mock_service_c = MockServiceC::new();
    let mock_service_d = MockServiceD::new();
    let mock_service_e = MockServiceE::new();
    let mock_service_f = MockServiceF::new();
    assert_eq!(mock_service_a.call(), 1);
}
