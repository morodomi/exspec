import pytest


def test_skipped_without_assertion():
    pytest.skip("Not supported yet")
