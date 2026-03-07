def test_create_user():
    user = create_user("alice")
    assert user.name == "alice"

def test_delete_user():
    result = delete_user(1)
    assert result is True
