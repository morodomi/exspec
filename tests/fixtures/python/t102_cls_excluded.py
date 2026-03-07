class TestUser:
    @classmethod
    def test_classmethod(cls, db, cache):
        """cls is excluded from count — fixture_count should be 2."""
        result = do_something(db, cache)
        assert result is not None
