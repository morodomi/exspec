"""
Fixture E2E: conftest.py should be classified as helper (PY-E2E-03)
"""
import pytest


@pytest.fixture
def user():
    """Shared fixture - should be excluded as helper"""
    return {"id": 1, "name": "test_user"}
