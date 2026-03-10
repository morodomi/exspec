# Known Constraints

exspec uses tree-sitter for static AST analysis. This is fast and language-agnostic, but has inherent limitations.

## Rust macro-generated tests

tree-sitter parses macro bodies as opaque `token_tree` nodes. This affects Rust in two ways:

1. **Test functions inside macros are not detected.** If your test harness generates tests via macros (e.g. `rgtest!` in ripgrep), exspec will not see them at all.
2. **Custom assertion macros are invisible.** Macros like `assert_pending!`, `assert_ready!`, `assert_data_eq!` are not recognized as assertions.

**Workaround**: Use `[assertions] custom_patterns` for assertion macros:

```toml
[assertions]
custom_patterns = ["assert_pending!", "assert_ready!", "assert_data_eq!"]
```

This does not help with macro-generated test functions -- those are fundamentally invisible to tree-sitter.

**Dogfooding data**: In tokio (~1,582 tests), 131 of 388 BLOCK violations (33.8%) were false positives caused by custom assertion macros. In clap (~1,455 tests), 115 of 528 BLOCK violations came from `assert_data_eq!`.

## TypeScript T107 (assertion-roulette)

T107 detects tests with multiple assertions but no descriptive messages. For TypeScript, the assertion message count is always set equal to the assertion count, which means T107 never fires.

This is intentional. TypeScript assertion libraries (Jest, Vitest, Chai) have inconsistent message parameter positions, and false-positive T107 was noisier than useful (36-48% false positive rate in dogfooding).

## Helper delegation

Test functions that delegate assertions to project-local helpers are not recognized as having assertions:

```python
def test_validation(self):
    self.assertValid(data)  # exspec sees no standard assertion
```

```php
public function test_structure(): void {
    $this->assertJsonStructure($response, $expected);  // not recognized
}
```

**Workaround**: Add helper patterns to config:

```toml
[assertions]
custom_patterns = ["assertValid", "assertJsonStructure", "self.assertValid"]
```

**Dogfooding data**: Helper delegation was the primary remaining false positive source across all languages after query-level fixes. In Laravel, 222 remaining BLOCK violations were all helper delegation patterns.

## Benchmark / compile-fail / model-check tests

Some test functions are intentionally assertion-free:

- **Benchmarks**: `benchmark()` functions in pytest-benchmark
- **Compile-fail tests**: Tests that verify compilation failure
- **Model-check tests**: Property-based model checking

These will trigger T001 (assertion-free). Use inline suppression:

```python
# exspec-ignore: T001
def test_benchmark_performance(benchmark):
    benchmark(my_function)
```

## Callback / wrapper patterns

Tests that pass assertions through callbacks (e.g. `done()` in Mocha-style async tests) or return assertion wrappers may not be recognized:

```typescript
it('async test', (done) => {
    fetchData().then(data => {
        expect(data).toBe(true);
        done();
    });
});
```

The `expect()` inside the callback **is** counted. But if the test only calls `done()` without any assertion, T001 will fire -- which is typically a true positive.
