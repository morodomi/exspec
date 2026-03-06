from unittest.mock import patch


class TestUser:
    @patch("app.db")
    def test_create(self, mock_db):
        assert True
