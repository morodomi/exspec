#[test]
fn test_create_user() {
    let user = User::new("alice");
    assert_eq!(user.name(), "alice");
}

#[test]
fn test_weak_error_check() {
    // .is_err() without assertion — not a real error test
    let _ = User::new("").is_err();
}
