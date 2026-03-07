import unittest

class TestDeprecation(unittest.TestCase):
    def test_old_api(self):
        result = old_api()
        self.assertEqual(result, 42)

    def test_old_api_warns(self):
        self.assertWarns(DeprecationWarning, old_api)
