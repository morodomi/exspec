import pytest


def test_explicit_failure_oracle():
    # pytest.fail() is an explicit oracle — unconditionally fails the test
    pytest.fail("This condition should never be reached")


def test_no_assertions():
    x = 1 + 1
