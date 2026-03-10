---
feature: T001-FP-pytest-fixture
cycle: 20260310_1739_pytest-fixture-filter
phase: DONE
complexity: trivial
test_count: 5
risk_level: low
created: 2026-03-10 17:39
updated: 2026-03-10 18:01
---

# #56 T001 FP — pytest fixtures with `test_` prefix

## Scope Definition

### In Scope
- [ ] Python extractor: skip `@pytest.fixture` / `@fixture` decorated `test_*` functions
- [ ] Test fixture file for the pattern
- [ ] Integration test verifying exclusion

### Out of Scope
- Other decorator-based filtering (Reason: not needed for this issue)
- Query-level (.scm) changes (Reason: code-level filtering chosen)

### Files to Change (target: 10 or less)
- crates/lang-python/src/lib.rs (edit)
- tests/fixtures/python/test_fixture_false_positive.py (new)
- tests/integration/python_fixture_filter.rs or existing python integration test (edit/new)

## Environment

### Scope
- Layer: Backend
- Plugin: rust (exspec core)
- Risk: 10 (PASS)

### Runtime
- Language: Rust (stable)

### Dependencies (key packages)
- tree-sitter: 0.24
- tree-sitter-python: existing

### Risk Interview (BLOCK only)
N/A — low risk

## Context & Dependencies

### Reference Documents
- crates/lang-python/src/lib.rs — Python extractor with `extract_functions_from_tree()`
- PHP/Rust extractors — reference pattern for attribute-based filtering

### Dependent Features
- T001 assertion-free rule — this fix reduces FPs

### Related Issues/PRs
- Issue #56: T001 FP — pytest fixtures with `test_` prefix

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] TC-01: `@pytest.fixture` decorated `test_data` → NOT included in test functions
- [x] TC-02: `@pytest.fixture()` (with parens) decorated `test_data` → NOT included
- [x] TC-03: `@fixture` (from pytest import fixture) decorated `test_input` → NOT included
- [x] TC-04: `@patch("x")` decorated `test_something` (real test) → IS included (no regression)
- [x] TC-05: Mixed file with fixture + real tests → fixture excluded, real tests evaluated normally

## Implementation Notes

### Goal
Eliminate T001 false positives caused by `@pytest.fixture` decorated functions with `test_` prefix being misidentified as test functions.

### Background
Python `test_function.scm` matches any `test_*` function regardless of decorator. The Rust extractor does not inspect decorator names. Dogfooding against Keiba found 2 FPs from this pattern.

### Design Approach
Code-level filtering in `extract_functions_from_tree()` in `crates/lang-python/src/lib.rs`. When processing a `decorated_definition` match, walk decorator children and check for `pytest.fixture` or `fixture`. If found, skip the match. This follows the same pattern as PHP/Rust extractors which filter by attribute name.

Decorator patterns to exclude:
- `@pytest.fixture` / `@pytest.fixture()` (import pytest)
- `@fixture` / `@fixture()` (from pytest import fixture)

## Progress Log

### 2026-03-10 17:39 - KICKOFF
- Cycle doc created
- 5 test cases defined from plan

### 2026-03-10 17:40 - RED
- 5 tests created in crates/lang-python/src/lib.rs (TC-01~TC-05)
- Fixture file: tests/fixtures/python/test_fixture_false_positive.py
- 4 tests failing (TC-01,02,03,05), 1 passing (TC-04 regression guard)
- exspec self-dogfooding: BLOCK 0 (3 in fixtures = expected)
- Phase completed

### 2026-03-10 17:50 - GREEN
- Added `is_pytest_fixture_decorator()` helper in lang-python/src/lib.rs
- Fixture check inserted before `test_matches.push()`, after `decorated_fn_ids.insert()`
- T001 message updated to include func.name
- 613 tests pass, clippy clean, fmt clean
- Phase completed

### 2026-03-10 17:55 - REFACTOR
- Verification Gate: all checks pass (test/clippy/fmt/exspec)
- No additional refactoring needed (minimal implementation)
- Phase completed

### 2026-03-10 17:59 - REVIEW
- review(code) score:15 verdict:PASS
- Security: PASS (8), Correctness: PASS (22)
- Phase completed

### 2026-03-10 18:01 - COMMIT
- Committed: c3888e6
- Phase completed

---

## Next Steps

1. [Done] KICKOFF
2. [Done] RED
3. [Done] GREEN
4. [Done] REFACTOR
5. [Done] REVIEW
6. [Done] COMMIT <- Complete
3. [ ] GREEN
4. [ ] REFACTOR
5. [ ] REVIEW
6. [ ] COMMIT
