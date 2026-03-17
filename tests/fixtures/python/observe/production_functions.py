"""
Fixture: production function extraction tests (PY-FUNC-01, PY-FUNC-02, PY-FUNC-03)
"""
import functools


def create_user():
    """PY-FUNC-01: top-level function -> name="create_user", class_name=None"""
    pass


def helper_function():
    """A helper also extracted as production function."""
    pass


class User:
    """PY-FUNC-02: class method -> name="save", class_name=Some("User")"""

    def save(self):
        pass

    def delete(self):
        pass


def my_decorator(func):
    @functools.wraps(func)
    def wrapper(*args, **kwargs):
        return func(*args, **kwargs)
    return wrapper


@my_decorator
def endpoint():
    """PY-FUNC-03: decorated function -> still extracted"""
    pass
