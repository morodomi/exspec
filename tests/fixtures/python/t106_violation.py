def test_duplicate_literals_in_assertions():
    assert calculate(1) == 42
    assert calculate(2) == 42
    assert calculate(3) == 42
    assert calculate(4) == 42
