import pytest
from pytest import fixture
from unittest.mock import patch


# TC-01: @pytest.fixture decorated test_data -> should NOT be extracted as test
@pytest.fixture
def test_data():
    return {"key": "value"}


# TC-02: @pytest.fixture() with parens -> should NOT be extracted as test
@pytest.fixture()
def test_config():
    return {"host": "localhost", "port": 8080}


# TC-03: @fixture (from pytest import fixture) -> should NOT be extracted as test
@fixture
def test_input():
    return [1, 2, 3]


# TC-04: @patch decorated real test -> SHOULD be extracted as test
@patch("os.path.exists")
def test_something(mock_exists):
    mock_exists.return_value = True
    assert True


# TC-05: Real test function with assertion
def test_real_function():
    assert 1 + 1 == 2


# TC-05: Real test function that uses fixture (assertion-free -> T001 violation)
def test_uses_fixture(test_data):
    result = test_data["key"]
