import pytest


def test_deprecation_warning():
    with pytest.warns(DeprecationWarning):
        deprecated_function()
