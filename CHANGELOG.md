# Changelog

## v0.1.1 (2026-03-11)

Bug fixes and two new configuration features since the initial public beta.

### Features

- **`--min-severity` display filter** (#59): Filter terminal/JSON output by severity level. `exspec --min-severity warn .` hides INFO diagnostics. Does not affect exit code (BLOCK violations still fail regardless of filter).
- **Per-rule severity override** (#60): `[rules.severity]` in `.exspec.toml` lets you change a rule's evaluation severity or disable it entirely. `T107 = "off"` disables the rule; `T101 = "info"` downgrades from WARN to INFO. This is orthogonal to `--min-severity`: severity overrides change *evaluation*, while `--min-severity` controls *display*.

### Bug Fixes

- **`.tsx` files**: TypeScript assertion detection now uses the TSX parser, fixing false positives on `.tsx` test files (#53)
- **`[paths] ignore` config**: The `ignore` patterns in `.exspec.toml` were not applied to file discovery. Fixed (#54)
- **T109 CJK test names**: Single-word heuristic falsely flagged Japanese/Chinese test names as undescriptive. CJK character sequences are now excluded (#55)
- **`@pytest.fixture` false positives**: Functions decorated with `@pytest.fixture` that happen to start with `test_` are no longer analyzed as test functions (#56)
- **`pytest.fail()` as test oracle**: `pytest.fail()` is now recognized as a valid assertion for T001 (#57)
- **PHP `Facade::shouldReceive()`**: Static Mockery calls on Laravel Facades (`Event::shouldReceive()`, etc.) are now recognized as assertions for T001 (#58)

### Internal

- T109 suffix check uses `chars().count()` instead of `len()` for correct Unicode handling (#61)
- `KNOWN_RULE_IDS` extracted as single source of truth for rule ID validation (#60)

## v0.1.0 (2026-03-10) -- Public Beta

First public release. Dogfooded across 9 projects, 4 languages, ~23,000 tests.

### What this release includes

- **16 check rules** (Tier 1 + Tier 2) for test design quality
- **4 languages**: Python (pytest), TypeScript (Jest/Vitest), PHP (PHPUnit/Pest), Rust (cargo test)
- **Output formats**: Terminal, JSON, SARIF (GitHub Code Scanning)
- **Inline suppression**: `# exspec-ignore: T001` per function
- **Custom assertion helpers**: `[assertions] custom_patterns` in `.exspec.toml`
- **Gradual adoption**: disable Tier 2 rules, enable one at a time

### What this release does NOT promise

- **Not production-ready**: This is a public beta for trial and gradual adoption
- **Not on crates.io**: Install via `cargo install --git`. crates.io publish is intentionally deferred
- **No stability guarantee**: Rule IDs, severity levels, and config format may change in minor versions
- **Known false positives**: Helper delegation patterns require `custom_patterns` config. See [Known Constraints](README.md#known-constraints) in README

### Install

```bash
cargo install --git https://github.com/morodomi/exspec.git
```
