def test_too_many_fixtures(db, cache, mailer, queue, logger, auth, config):
    """Test with 7 parameters (> threshold 5) — T102 violation."""
    result = do_something(db, cache, mailer, queue, logger, auth, config)
    assert result is not None
