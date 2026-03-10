# Language Support

## Overview

| Language | Test Frameworks | Since | Notes |
|----------|----------------|-------|-------|
| Python | pytest | v0.1.0 | Best coverage |
| TypeScript | Jest, Vitest | v0.1.0 | Chai / Sinon assertion chains supported |
| PHP | PHPUnit, Pest | v0.1.0 | Mockery assertions supported |
| Rust | cargo test | v0.1.0 | Macro-generated tests not detected |
| Dart | flutter_test | Planned | -- |

## Python

- **Test detection**: Functions starting with `test_` or decorated with `@pytest.mark.*`
- **Assertions**: `assert`, `pytest.raises`, `mock.assert_called*`, `mock.assert_any_call`, etc.
- **Known gap**: Nested test functions (functions defined inside test functions) -- assertions in inner functions are not counted toward the outer test

## TypeScript

- **Test detection**: `test()`, `it()` calls
- **Assertions**: `expect()` chains, Chai property/method chains (`.to.be.true`, `.to.have.been.called`, etc.), Sinon `.verify()`
- **Chai chain depth**: Supported up to depth 7 (e.g. `.rejected.and.be.an.instanceof()`)
- **T107 (assertion-roulette)**: Always set to `assertion_count` rather than independently counting message arguments. This avoids false positives but means T107 never fires for TypeScript
- **Inline suppression scope**: `// exspec-ignore` applies to the **next** `test()`/`it()` call only. It does **not** propagate through a `describe()` block

## PHP

- **Test detection**: Methods starting with `test` or annotated with `@test`, Pest `test()` / `it()` calls
- **Assertions**: `$this->assert*()`, `self::assert*()`, `Assert::assert*()`, `$this->expect*()`, Mockery `shouldReceive` / `shouldHaveReceived` / `shouldNotHaveReceived` / `expects`, Facade `ClassName::assert*()`
- **Known gap**: Helper delegation patterns like `$this->fails()`, `$assert->has()` are not recognized. Use `[assertions] custom_patterns`

## Rust

- **Test detection**: Functions with `#[test]` or `#[tokio::test]` attributes
- **`#[should_panic]`**: Detected and counted as an assertion
- **Macro limitation**: tree-sitter parses macro bodies as opaque `token_tree` nodes. This means:
  - Test functions generated inside macros (e.g. `rgtest!`, custom test harnesses) are **not detected**
  - Custom assertion macros (e.g. `assert_pending!`, `assert_ready!`, `assert_data_eq!`) are **invisible**
  - Use `[assertions] custom_patterns` to recognize custom assertion macros
