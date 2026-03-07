import unittest

class TestUser(unittest.TestCase):
    def test_create_user(self):
        user = create_user("alice")
        self.assertEqual(user.name, "alice")

    def test_create_user_invalid(self):
        self.assertRaisesRegex(ValueError, "empty name", create_user, "")
