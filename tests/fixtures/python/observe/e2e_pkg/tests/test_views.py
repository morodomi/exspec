"""
Fixture E2E: test file for Layer 2 import tracing (PY-E2E-02)
Imports views.index from parent package -> Layer 2 match
"""
from ..views import index


def test_index_view():
    # Given: the index view function
    # When: called
    # Then: returns expected result
    result = index()
    assert result is None
