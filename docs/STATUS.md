# Project Status

## Current Phase

Phase 8a (Lint Reliability) + Phase 8b (observe PoC) in progress. v0.1.2 published to crates.io. 13 projects / 4 languages / ~45k tests dogfooded. 17 active rules, 4 languages, 741 tests.

## Progress

| Phase | Status |
|-------|--------|
| 0 - SPEC.md + naming | DONE |
| 1 - Rust + tree-sitter scaffolding | DONE |
| 2 - Python + Tier 1 (T001-T003) | DONE |
| 3A - TypeScript + inline suppression + output polish | DONE |
| 3B - T004-T008 + .exspec.toml parsing | DONE |
| 3B cleanup - Discovered items | DONE |
| 3C - SARIF output + metrics | DONE |
| 3 cleanup - NaN guard, TestCase false positive, dead code | DONE |
| 4 - PHP support (PHPUnit/Pest) + dev-crew integration | DONE |
| 4.1 - PHP FQCN attribute + Pest arrow function | DONE |
| 4.2 - Nested class, docblock dedup, FQCN pattern | DONE |
| 5A - Rust language support (cargo test) | DONE |
| 5B - Tier 2 rules (T101-T105) Python + TypeScript | DONE |
| 5C - Tier 2 PHP/Rust expansion (T101-T105) | DONE |
| 5.5 - Gap rules (T106-T109) + T104 removal | DONE |
| 6 - Release Hardening (FP fixes, dogfooding) | DONE |
| 7 - OSS release (crates.io v0.1.2) | DONE |
| 8a - Lint Reliability (BLOCK/WARN/INFO FP fixes) | IN PROGRESS |
| 8b - observe PoC (static test-to-code mapping) | IN PROGRESS |

### Phase 8b Task Progress

| # | Task | Status |
|---|------|--------|
| 0 | Ground truth (nestjs/nest manual mapping) | TODO |
| 1 | Production function extractor (TypeScript) | DONE |
| 2 | NestJS route/decorator extractor | DONE |
| 3a | Test-to-code mapper: file name convention (Layer 1) | TODO |
| 3b | Test-to-code mapper: import tracing (Layer 2) | TODO |
| 4a | Test status code assertion extractor | TODO |
| 4b | Error-path gap analyzer | TODO |
| 5 | `exspec --observe` CLI + Markdown output | TODO |
| 6 | NestJS precision verification (ground truth comparison) | TODO |

## Supported Languages

| Language | Extraction | Assertions | Mocks | Suppression |
|----------|-----------|------------|-------|-------------|
| Python (pytest) | Yes | Yes | Yes | Yes |
| TypeScript (Jest/Vitest) | Yes | Yes | Yes | Yes |
| PHP (PHPUnit/Pest) | Yes | Yes | Yes | Yes |
| Rust (cargo test) | Yes | Yes | Yes | Yes |

## Active Rules

| ID | Rule | Level | Python | TypeScript | PHP | Rust |
|----|------|-------|--------|-----------|-----|------|
| T001 | assertion-free | BLOCK | Yes | Yes | Yes | Yes |
| T002 | mock-overuse | WARN | Yes | Yes | Yes | Yes |
| T003 | giant-test | WARN | Yes | Yes | Yes | Yes |
| T004 | no-parameterized | INFO | Yes | Yes | Yes | Yes |
| T005 | pbt-missing | INFO | Yes | Yes | N/A | Yes |
| T006 | low-assertion-density | WARN | Yes | Yes | Yes | Yes |
| T007 | test-source-ratio | INFO | -- | -- | -- | -- |
| T008 | no-contract | INFO | Yes | Yes | Yes | N/A |
| T101 | how-not-what | INFO | Yes | Yes | Yes | Yes* |
| T102 | fixture-sprawl | INFO | Yes | Yes | Yes* | Yes* |
| T103 | missing-error-test | INFO | Yes | Yes | Yes | Yes* |
| T105 | deterministic-no-metamorphic | INFO | Yes | Yes | Yes | Yes* |
| T106 | duplicate-literal-assertion | OFF | Yes | Yes | Yes | Yes |
| T107 | assertion-roulette | INFO | Yes | -- | Yes | Yes |
| T108 | wait-and-see | INFO | Yes | Yes | Yes | Yes |
| T109 | undescriptive-test-name | INFO | Yes | Yes | Yes | Yes |
| T110 | skip-only-test | INFO | Yes | -- | Yes | -- |

\* Notes:
- Rust T101: token_tree limitation -- private field access in macros not detectable.
- Rust T105: token_tree limitation -- relational operators in `assert!()` not detectable.
- PHP T102: `#[DataProvider]` params excluded from fixture count (#20).
- Rust T102: Smart fixture detection -- constructor/struct/macro counted, method calls on locals excluded (#21).
- Rust T103: `.is_err()` removed as weak proxy -- only `#[should_panic]` and `.unwrap_err()` (#22).
- T107: TypeScript skipped -- Jest/Vitest expect() has no message argument.
- T104: Deprecated and removed in Phase 5.5 (replaced by T106).

## Quality Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Tests | 741 passing | -- |
| Coverage | N/A | 90%+ (min 80%) |
| Clippy errors | 0 | 0 |

## Output Formats

| Format | Status |
|--------|--------|
| terminal | Supported |
| json | Supported |
| sarif | Supported (v2.1.0) |
| ai-prompt | Tier 3 (Phase 6) |
