import pytest


def test_invalid_input_raises_with_match():
    with pytest.raises(ValueError, match="empty name"):
        create_user("")
