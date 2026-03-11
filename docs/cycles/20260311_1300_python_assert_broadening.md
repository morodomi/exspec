# Cycle: #62 Python `^assert_` → `^assert` broadening

## Phase: DONE

## Context
pytest dogfooding (2,380 tests) shows ~100% FP rate for T001 BLOCK. Root cause: `reprec.assertoutcome()` and similar `assert`-prefixed methods (without underscore) are not recognized as assertions.

## Scope
- `crates/lang-python/queries/assertion.scm` lines 21-25
- New fixture: `tests/fixtures/python/t001_pass_assert_no_underscore.py`
- New tests in `crates/lang-python/src/lib.rs`

## Design
Replace single pattern with 2-pattern approach:
1. `obj.assert*()` (broadened, non-self) — catches `reprec.assertoutcome()`, `response.assertStatus()`
2. `expr.attr.assert_*()` (chained, attribute object) — catches `mock.return_value.assert_called_once()`

## Test List
- [x] `obj.assertoutcome()` → assertion_count >= 1
- [x] `obj.assertStatus()` → assertion_count >= 1
- [x] `self.assertEqual()` → assertion_count == 1 (no double-count, existing test)
- [x] `mock.return_value.assert_called_once()` → assertion_count >= 1 (existing test)

## Progress Log

### 2026-03-11 - RED/GREEN
- Created fixture t001_pass_assert_no_underscore.py
- Added 2 new tests, updated 1 existing test expectation
- Modified assertion.scm: 2-pattern design with self-exclusion guard
- Phase completed

### 2026-03-11 - REFACTOR
- Fixed comment header format consistency
- /simplify review: no significant issues found
- Phase completed

### 2026-03-11 - REVIEW
- Risk: LOW (score 0). Security: PASS (3/100). Correctness: PASS (22/100).
- All tests pass (658), clippy clean, fmt clean, self-dogfooding BLOCK 3 (fixtures only)
- Phase completed
