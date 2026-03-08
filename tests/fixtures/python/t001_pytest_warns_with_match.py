import pytest


def test_user_warning_with_match():
    with pytest.warns(UserWarning, match="deprecated"):
        trigger_warning()
