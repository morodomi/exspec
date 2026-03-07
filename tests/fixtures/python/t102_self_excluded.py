class TestUser:
    def test_method(self, db, cache):
        """self is excluded from count — fixture_count should be 2."""
        result = do_something(db, cache)
        assert result is not None
