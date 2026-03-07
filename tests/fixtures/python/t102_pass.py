def test_few_fixtures(db, cache):
    """Test with 2 parameters (<= threshold 5) — T102 pass."""
    result = do_something(db, cache)
    assert result is not None
