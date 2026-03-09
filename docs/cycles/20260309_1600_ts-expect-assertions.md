# Cycle: #39 T001 FP: TS expect.assertions / expect.unreachable / expectType

**Issue**: #39
**Date**: 2026-03-09
**Status**: DONE

## Goal

Detect `expect.assertions(N)`, `expect.hasAssertions()`, `expect.unreachable()`, and standalone `expectType<T>(value)` as assertion oracles in TypeScript, eliminating ~15% of vitest T001 false positives.

## Background

Dogfooding (#23) found ~15% of vitest T001 false positives come from 3 undetected assertion patterns. These are legitimate test oracles: `expect.assertions(N)` declares assertion count, `expect.unreachable()` is a control flow oracle (reaching it = failure), and `expectType<T>()` is a type assertion (consistent with existing `expectTypeOf` handling).

## Scope

### In Scope
- Pattern A: `expect.assertions(N)` / `expect.hasAssertions()` / `expect.unreachable()` (member-call on expect)
- Pattern B: `expectType<T>(value)` (standalone call, extending existing `expectTypeOf` pattern)
- Fixtures and unit tests for all patterns

### Out of Scope
- Chai method-call chains (#40)
- Python nested function assertion counting (#41)
- Rust code changes (scm-only fix)

## Design

### Approach
- **Pattern A**: New scm pattern matching `expect.<method>()` where method is `assertions|hasAssertions|unreachable`. Separate from existing `soft|element|poll` pattern because semantically distinct (standalone statements vs chainable expect variants).
- **Pattern B**: Extend existing `expectTypeOf` identifier match from `(#eq? "expectTypeOf")` to `(#match? "^expectType(Of)?$")`. Also add standalone `expectType(value)` pattern (without chained method call).

### Files Changed

| File | Change |
|------|--------|
| `crates/lang-typescript/queries/assertion.scm` | Add Pattern A, extend Pattern B, add standalone expectType |
| `tests/fixtures/typescript/t001_expect_assertions.test.ts` | New fixture (8 test cases) |
| `crates/lang-typescript/src/lib.rs` | Unit test with 8 assertions |

## Environment

- Rust 1.88.0, tree-sitter 0.24
- Layer: Backend (Rust + tree-sitter .scm)
- Risk: 10 (PASS)

## Test List

| # | Given | When | Then |
|---|-------|------|------|
| TC-01 | `expect.assertions(1)` + `expect(data).toBeDefined()` | T001 eval | assertion_count >= 1, PASS |
| TC-02 | `expect.assertions(0)` | T001 eval | assertion_count >= 1, PASS |
| TC-03 | `expect.hasAssertions()` + `expect(data).toBeTruthy()` | T001 eval | assertion_count >= 1, PASS |
| TC-04 | `expect.unreachable()` | T001 eval | assertion_count >= 1, PASS |
| TC-05 | `expectType<User>(user)` | T001 eval | assertion_count >= 1, PASS |
| TC-06 | `expect.assertions(2)` + 2x `expect().toBe()` | T001 eval | assertion_count >= 2 |
| TC-07 | `expectType<T>(v)` + `expectTypeOf(v).toX()` | T001 eval | assertion_count >= 2 |
| TC-08 | No assertions (regression guard) | T001 eval | assertion_count == 0, BLOCK |

## Dogfooding

| Metric | Before (#37 fix) | After |
|--------|-------------------|-------|
| vitest T001 BLOCK | 350 | 339 |
| Reduction | - | 11 tests (3.1%) |

## Progress Log

### Phase: KICKOFF - Completed
**Artifacts**: Cycle doc created
**Decisions**: scm-only fix, separate pattern for assertions/hasAssertions/unreachable
**Pre-Review**: Risk 10 (PASS)
**Next Phase Input**: Implementation already done (RED+GREEN completed before cycle doc)

### Phase: RED - Completed
**Artifacts**: `tests/fixtures/typescript/t001_expect_assertions.test.ts`, test in `crates/lang-typescript/src/lib.rs`
**Decisions**: 8 test cases covering all patterns + regression guard
**Next Phase Input**: Tests confirmed failing before scm changes

### Phase: GREEN - Completed
**Artifacts**: `crates/lang-typescript/queries/assertion.scm` (3 new/modified patterns)
**Decisions**: All 461 tests passing
**Next Phase Input**: Run /simplify for quality check

### Phase: REFACTOR - Completed
**Artifacts**: `crates/lang-typescript/queries/assertion.scm` (reverted Pattern B regex to #eq?)
**Decisions**: /simplify found 1 medium issue — expectType double-count latent bug. Fixed by reverting chained pattern to `#eq? "expectTypeOf"` (standalone `expectType` covered by separate pattern). 2 low findings skipped.
**Next Phase Input**: source files on disk, run review

### Phase: REVIEW - Completed
**Artifacts**: review results (mode: code)
**Decisions**: verdict=PASS, score=5 (max). security-reviewer: 0, correctness-reviewer: 5 (optional: TC-02/04/05 exact assertions adopted)
**Next Phase Input**: all tests passing, ready to commit

## DISCOVERED
