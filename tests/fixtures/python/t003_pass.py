def test_short_function():
    user = create_user("Bob")
    assert user.name == "Bob", "name should match"
    assert user.active is True, "user should be active"
