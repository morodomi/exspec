# Severity Review

Date: 2026-03-10

## Methodology

Reviewed all 16 rules against 5 dogfooding repos:
- Rust: exspec (self, 51 tests)
- Python: pydantic (~2500 tests), fastapi (2121 tests)
- TypeScript: vitest (3120 tests)
- PHP: Laravel (10790 tests)

Severity criteria:
- **BLOCK**: near-zero false positives expected (exit code 1)
- **WARN**: ~70-80% true positive rate expected (exit 0, unless --strict)
- **INFO**: ~50%+ acceptable, opinionated rules OK

## Decisions

| Rule | Name | Before | After | Rationale |
|------|------|--------|-------|-----------|
| T001 | assertion-free | BLOCK | BLOCK | TP rate improved via Phase 6 FP fixes. Residual FPs are helper delegation / benchmark patterns, addressable via .exspec.toml custom_patterns. |
| T002 | mock-overuse | WARN | WARN | Low fire rate (6 total). High precision. |
| T003 | giant-test | WARN | WARN | 391 total. Appropriate signal. |
| T004 | no-parameterized | INFO | INFO | High fire rate but INFO is appropriate. |
| T005 | pbt-missing | INFO | INFO | Same as T004. |
| T006 | low-assertion-density | WARN | WARN | Low fire rate (39 total). |
| T007 | test-source-ratio | INFO | INFO | Project-level, 3 total. |
| T008 | no-contract | INFO | INFO | High fire rate but INFO is appropriate. |
| T101 | how-not-what | WARN | INFO | Phase 8a-2 survey: 47% FP across 13 projects. Too noisy for WARN. |
| T102 | fixture-sprawl | WARN | INFO | Phase 8a-2 survey: 80% FP across 13 projects. Too noisy for WARN. |
| T103 | missing-error-test | INFO | INFO | Appropriate as INFO. |
| T105 | deterministic-no-metamorphic | INFO | INFO | Appropriate as INFO. |
| T106 | duplicate-literal-assertion | INFO | OFF | Phase 8a-2 survey: 93% FP. Default disabled; enable via `T106 = "info"` in .exspec.toml. |
| **T107** | **assertion-roulette** | **WARN** | **INFO** | **6625 total across all repos. 36-48% of tests trigger. TP rate is high (most tests genuinely lack assertion messages) but volume makes WARN impractical. Demoted to INFO to reduce noise.** |
| T108 | wait-and-see | WARN | INFO | Phase 8a-2 survey: 93% FP across 13 projects. Too noisy for WARN. |
| T109 | undescriptive-test-name | INFO | INFO | Appropriate as INFO. |

## Default-On Review

All rules remain default-on except T106 (disabled by default due to 93% FP rate). Opinionated rules (T105, T109) are INFO and non-blocking, so they serve as suggestions without CI friction. T106 can be re-enabled via `[rules.severity] T106 = "info"` in .exspec.toml.

## Fire Rate Summary (all repos combined)

| Severity | Rules | Total Fires |
|----------|-------|-------------|
| BLOCK | T001 | 656 |
| WARN | T002, T003, T006 | 436 |
| INFO | T004, T005, T007, T008, T101, T102, T103, T105, T107, T108, T109 | 17,732 |
| OFF | T106 | -- |

Post-change (Phase 8a-3), WARN-level fires drop from 2,318 to 436 (-81%). T101/T102/T108 moved to INFO; T106 default OFF.
