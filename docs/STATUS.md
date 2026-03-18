# Project Status

## Current Phase

v0.3.0 development. observe TypeScript: P=100%, R=91% (separate packages), route extraction for 4 frameworks. Lint: 17 active rules, 4 languages, 13 projects / ~45k tests dogfooded.

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
| 8a-1 - BLOCK FP fixes (#62/#63/#64) | **DONE** |
| 8a-2 - WARN/INFO dogfooding survey | **DONE** |
| 8a-3 - Severity adjustments (T101/T102/T108->INFO, T106->OFF) | **DONE** |
| 8a-4 - Helper delegation strategy | **DONE** |
| 8b - observe PoC (static test-to-code mapping) | **DONE** |
| 8c-1 - observe failure boundaries | **DONE** |
| 8c-2 - observe MVP ship (README, ship criteria) | **DONE** |
| 8c-3 - tsconfig path resolution | **DONE** |
| 8c-4 - context-aware enum/interface filter | **DONE** |
| 10 - Route extraction (NestJS, FastAPI, Next.js, Django) | **DONE** |
| 11 - TS observe re-dogfood + GT audit | **DONE** |
| 12 - Python observe dogfooding + GT | **DONE** |

### Phase 12 Python Observe Dogfooding Results (2026-03-19)

| Project | Precision | Recall | F1 | Status |
|---------|-----------|--------|----|--------|
| httpx (30 test files) | 66.7% | 6.2% | 11.4% | FAIL |
| Requests (9 test files) | N/A | ~0% | N/A | FAIL |

**Both below first-pass criteria (P>=90%, R>=80%).** Root causes: L1 `_` prefix mismatch, L2 barrel import unresolved, `src/` layout not detected. Improvement plan filed in dogfooding-results.md.

### Phase 11 Re-dogfood Results (2026-03-18)

NestJS ground truth re-validated after Phase 8c/10 changes. 12 FP reclassified as legitimate secondary targets.

| Scope | Precision | Recall | F1 | FP | FN |
|-------|-----------|--------|----|----|-----|
| Separate packages (common + core) | 100.0% | 91.0% | 95.2% | 0 | 15 |
| Root (full monorepo) | 94.1% | 95.8% | 94.9% | 10 | 7 |
| typeorm (50-pair spot-check) | 100% | -- | -- | 0 | -- |

Remaining FN (separate): B2 cross-package (8), B2+B4 cross-package enum/interface (5), B4 same-package barrel (2).
Root mode resolves most B2 FN but introduces FP from peripheral imports not yet in GT.

### Phase 8b Historical Results (for reference)

| Repository | Precision | Recall | F1 | FP | FN |
|------------|-----------|--------|----|----|-----|
| nestjs/nest (GT complete) | 99.4% | 93.4% | 96.3% | 1 | 11 |

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
| Tests | 946 passing | -- |
| Coverage | N/A | 90%+ (min 80%) |
| Clippy errors | 0 | 0 |

## Output Formats

| Format | Status |
|--------|--------|
| terminal | Supported |
| json | Supported |
| sarif | Supported (v2.1.0) |
| ai-prompt | Tier 3 (Phase 6) |
