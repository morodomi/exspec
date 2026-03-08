import pytest


def test_invalid_input_raises():
    with pytest.raises(ValueError):
        create_user("")
