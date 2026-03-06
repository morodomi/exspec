# Recommended Hooks

## Pre-commit (installed at .git/hooks/pre-commit)

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

## Pre-push (optional)

```bash
cargo test
cargo clippy -- -D warnings
```

## CI Integration

```bash
cargo test
cargo llvm-cov --lcov --output-path lcov.info
cargo clippy -- -D warnings
cargo fmt --check
```
