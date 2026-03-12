---
feature: T001-chai-property-return
cycle: 20260312_1147_t001_return_chai_property
phase: REVIEW
complexity: trivial
test_count: 6
risk_level: low
created: 2026-03-12 11:47
updated: 2026-03-12 13:32
---

# #52 T001 FP: TS return-wrapped Chai property assertions

## Scope Definition

### In Scope
- [ ] Add `return_statement`-wrapped Chai property patterns (depth 1-5) to `assertion.scm`
- [ ] New fixture for return-wrapped Chai property assertions
- [ ] Integration test verifying assertion detection and no double-counting

### Out of Scope
- depth 6-7 patterns (Reason: no real-world evidence of return-wrapped chains that deep)
- Other wrapper types (Reason: only `expression_statement`, `arrow_function body:`, and `return_statement` are structurally valid)

### Files to Change (target: 10 or less)
- crates/lang-typescript/queries/assertion.scm (edit)
- tests/fixtures/typescript/t001_chai_property_return.test.ts (new)
- crates/lang-typescript/src/lib.rs (edit)

## Environment

### Scope
- Layer: Backend
- Plugin: Rust + tree-sitter
- Risk: 10 (PASS)

### Runtime
- Language: Rust (stable)

### Dependencies (key packages)
- tree-sitter: 0.24
- tree-sitter-typescript: existing

### Risk Interview (BLOCK only)
N/A - low risk

## Context & Dependencies

### Reference Documents
- [docs/dogfooding-results.md] - NestJS dogfooding showing 2 FPs from return-wrapped patterns
- [docs/SPEC.md] - T001 rule specification

### Dependent Features
- Chai property assertion patterns (depth 1-7) already in assertion.scm

### Related Issues/PRs
- Issue #52: T001 FP: TS return-wrapped Chai property assertions (P2)

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] TC-01: `return expect(x).to.be.rejected` (depth 3) -- detected as assertion
- [x] TC-02: `return expect(x).to.be.ok` (depth 2) -- detected as assertion
- [x] TC-03: `return expect(x).to.have.been.calledOnce` (depth 4) -- detected as assertion
- [x] TC-04: Regression -- existing `expression_statement` and `arrow_function body:` Chai property fixtures pass unchanged
- [x] TC-05: `assertion_count == 1` pinning test -- block-body `return expect(x).to.be.ok;` does not double-count
- [x] TC-06: Negative control -- `return someNonAssertionExpr;` is NOT counted as assertion (overmatching guard)

## Implementation Notes

### Goal
Eliminate 2 T001 false positives from NestJS dogfooding caused by `return`-wrapped Chai property assertions.

### Background
NestJS dogfooding found tests like:
```typescript
it('should be rejected', () => {
  return expect(target.transform(obj, metadata)).to.be.rejected;
});
```
tree-sitter parses this as `return_statement > member_expression`, but current Chai property patterns only match `expression_statement` and `arrow_function body:` wrappers.

### Design Approach
Add `return_statement` as a third wrapper type for Chai property patterns in assertion.scm. Mirror the existing `expression_statement` depth 1-5 patterns. The three wrapper types (`expression_statement`, `arrow_function body:`, `return_statement`) are structurally exclusive in tree-sitter AST -- no double-count risk.

Review notes:
- Add structural exclusivity comment before the `return_statement` block (matching style at line 163-165)
- Document why depth 6-7 are not added (no real-world evidence for return-wrapped deep chains)

## Progress Log

### 2026-03-12 11:47 - KICKOFF
- Cycle doc created
- 6 test cases defined from plan (5 original + TC-06 negative control from review)
- Scope: assertion.scm + fixture + integration test

### 2026-03-12 13:05 - RED
- Added `t001_chai_property_return.test.ts` plus Rust tests covering TC-01..TC-06
- Included regression checks for existing expression and arrow wrapper behavior
- Phase completed

### 2026-03-12 13:12 - GREEN
- Added `return_statement` Chai property patterns depth 1-5 to `assertion.scm`
- Verified return-wrapped property assertions are counted and non-assertion returns stay unmatched
- Phase completed

### 2026-03-12 13:18 - REFACTOR
- Kept the implementation aligned with the existing expression/arrow wrapper structure
- Added wrapper exclusivity comments to preserve no-double-count intent
- Phase completed

### 2026-03-12 13:25 - REVIEW (round 1)
- Code review PASS: no correctness or regression findings after targeted test verification
- Phase completed

### 2026-03-12 13:26 - REFACTOR (round 2)
- simplify: 3-agent parallel review (reuse/quality/efficiency)
- Consolidated targeted assertions to reduce overlap across the new return-wrapper checks
- Added name assertions to regression test for fixture index safety
- Strengthened TC-01..04 from >= 1 to == 1

### 2026-03-12 13:32 - REVIEW (round 2)
- security-reviewer: PASS (score 3). No issues.
- correctness-reviewer: WARN (score 15). 2 important findings:
  - Earlier depth labels were inconsistent with the actual member-expression nesting
  - Coverage needed stronger exact-count assertions for the new return-wrapper cases
- Fix: aligned depth labeling with the final fixture/test matrix and tightened exact-count coverage while preserving TC-01..TC-06
- Added block-body arrow exclusivity explanation to .scm comment
- Verification Gate: PASS (683 tests, clippy 0, fmt OK)
- Phase completed

---

## Next Steps

1. [Done] KICKOFF <- Current
2. [Done] RED
3. [Done] GREEN
4. [Done] REFACTOR
5. [Done] REVIEW
6. [ ] COMMIT

## Plan Review Notes

### 2026-03-12 - Review Findings

1. BLOCK: `Files to Change` points to `tests/integration/typescript_test.rs`, but the TypeScript fixture-based tests live in `crates/lang-typescript/src/lib.rs`. Update the planned edit target before RED so implementation does not stall on a nonexistent file.
2. WARN: `TC-04` is too abstract. Pin the regression check to concrete fixture coverage for both existing wrapper families: `expression_statement` and `arrow_function body:`.
3. WARN: The new fixture plan has no negative control. Add one assertion-free `return ...;` case in the same fixture to guard against overmatching after introducing `return_statement` patterns.

### 2026-03-12 - Re-review Findings

1. WARN: `test_count` is updated to 6, but the KICKOFF progress log still says `5 test cases defined`. Sync the log entry with the current plan to avoid phase-history drift.
2. PASS: Previous BLOCK/WARN items are resolved. `Files to Change` now references `crates/lang-typescript/src/lib.rs`, `TC-04` is specific enough for RED, and negative control `TC-06` has been added.
