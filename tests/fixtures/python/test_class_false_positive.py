# Non-test class with test_ prefixed methods (should be excluded)
class UserService:
    def test_connection(self):
        return self.db.ping()

    def test_health(self):
        return True


# Test class (should be included)
class TestUser:
    def test_create(self):
        assert True

    def test_delete(self):
        assert True


# Module-level test function (should be included)
def test_standalone():
    assert True
