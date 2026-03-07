import pytest

def test_create_user():
    user = create_user("alice")
    assert user.name == "alice"

def test_create_user_invalid():
    with pytest.raises(ValueError):
        create_user("")
