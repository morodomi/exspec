---
feature: lang-python
cycle: pytest-fail-oracle
phase: DONE
complexity: trivial
test_count: 2
risk_level: low
created: 2026-03-11 11:26
updated: 2026-03-11 11:48
---

# T001 FP fix: pytest.fail() as test oracle (#57)

## Scope Definition

### In Scope
- [x] Add pytest.fail() pattern to Python assertion.scm
- [ ] Create fixture t001_pytest_fail.py
- [ ] Add integration test in lang-python/src/lib.rs
- [x] Update docs/SPEC.md with pytest.fail() in supported patterns

### Out of Scope
- pytest.skip() (not an oracle — skips execution)
- pytest.xfail() (not an oracle — marks expected failure)

### Files to Change (target: 10 or less)
- crates/lang-python/queries/assertion.scm (edit)
- tests/fixtures/python/t001_pytest_fail.py (new)
- crates/lang-python/src/lib.rs (edit)
- docs/SPEC.md (edit)

## Environment

### Scope
- Layer: Backend
- Plugin: N/A (Rust)
- Risk: 10 (PASS)

### Runtime
- Language: Rust (stable)

### Dependencies (key packages)
- tree-sitter: 0.24
- tree-sitter-python: existing

### Risk Interview (BLOCK only)
N/A (PASS)

## Context & Dependencies

### Reference Documents
- [docs/SPEC.md] - T001 rule specification
- [docs/languages/python.md] - Python-specific behaviors

### Dependent Features
- Python assertion.scm: existing pytest.raises/warns patterns (pattern 3, 5)

### Related Issues/PRs
- Issue #57: T001 FP: pytest.fail() not recognized as test oracle

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] TC-01: Given a test function containing only `pytest.fail(msg)`, When T001 is evaluated, Then assertion_count >= 1 and no T001 violation
- [x] TC-02: Given a test function with no assertions and no pytest.fail(), When T001 is evaluated, Then T001 BLOCK (control case)

## Implementation Notes

### Goal
Recognize pytest.fail() as a test oracle in Python assertion.scm to eliminate T001 false positives.

### Background
pytest.fail() is an explicit failure oracle — it unconditionally fails the test with a message. This is functionally equivalent to `assert False, msg` and should count as an assertion/oracle for T001 purposes. Found during Keiba project dogfooding.

### Design Approach
Follow existing pytest.raises (pattern 3) / pytest.warns (pattern 5) convention in assertion.scm. Add a new pattern matching `pytest.fail()` as an attribute call with `(#eq? @obj "pytest") (#eq? @attr "fail")`.

Side effects (by design):
- T107: pytest.fail() increments assertion_count. Correct behavior.
- T106: pytest.fail(msg) message strings counted as assertion literals. Consistent.

## Progress Log

### 2026-03-11 - REVIEW
- review(code) score:12 verdict:PASS
- Security: PASS (score 2), Correctness: PASS (score 22)
- No blocking or warning issues
- Phase completed

### 2026-03-11 - REFACTOR
- /simplify: 3-agent parallel review (reuse, quality, efficiency)
- No actionable issues found — all findings are existing codebase patterns
- Verification Gate: PASS (619 tests, clippy clean, fmt clean)
- Phase completed

### 2026-03-11 11:48 - GREEN
- Added pytest.fail() pattern to crates/lang-python/queries/assertion.scm (pattern 6)
- Updated docs/SPEC.md T001 Detection section with Python assertion patterns list
- cargo test: all 619 tests pass (TC-01 t001_pytest_fail_counts_as_assertion: ok)
- cargo clippy -- -D warnings: 0 errors
- cargo fmt --check: no diff

### 2026-03-11 11:26 - KICKOFF
- Cycle doc created
- Scope definition ready
- Test list transferred from plan

### 2026-03-11 - RED
- Created tests/fixtures/python/t001_pytest_fail.py (pass case + control case)
- Added t001_pytest_fail_counts_as_assertion (TC-01) to crates/lang-python/src/lib.rs
- Added t001_no_assertions_still_fires (TC-02) to crates/lang-python/src/lib.rs
- RED state verified: TC-01 FAILED (assertion_count=0, pytest.fail() not yet in assertion.scm)
- TC-02 PASSED (control case: 0 assertions confirmed, no regression)
- cargo fmt + cargo clippy: clean

---

## Next Steps

1. [Done] KICKOFF
2. [Done] RED
3. [Done] GREEN
4. [Done] REFACTOR
5. [Done] REVIEW
6. [Done] COMMIT <- Complete (0d1cc78)
