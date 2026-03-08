def test_trivial_literals_not_counted():
    assert calculate(1) == 0
    assert calculate(2) == 0
    assert calculate(3) == 0
    assert calculate(4) == 0
