def test_outer():
    def test_inner():
        assert False

    assert True


def test_multi_outer():
    def test_multi_mid():
        def test_multi_inner():
            assert True

    assert True


def test_with_helper():
    def helper():
        assert True

    assert True


def test_sibling_a():
    assert True


def test_sibling_b():
    assert True


def test_async_outer():
    async def test_async_helper():
        assert True

    assert True
