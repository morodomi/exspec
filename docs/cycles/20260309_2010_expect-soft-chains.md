# Cycle: #43 T001 FP: expect.soft modifier-chain support

**Issue**: #43
**Date**: 2026-03-09
**Status**: DONE

## Goal

Add depth-3/4 patterns for `expect.soft/element/poll` with `not/resolves/rejects` modifier chains.

## Background

After #39 added depth-2 support for `expect.soft(x).toBe(y)`, deeper chains like `expect.soft(x).not.toBe(y)` and `expect.soft(x).resolves.not.toBe(y)` were still undetected.

## Scope

### In Scope
- Depth-3: `expect.soft(x).not.toBe(y)`, `expect.soft(x).resolves.toBe(y)`
- Depth-4: `expect.soft(x).resolves.not.toBe(y)`
- All three separators: `soft`, `element`, `poll`
- Terminal constraint: `^to[A-Z]` for depth-3+ (safety against custom helpers)
- Negative test: `expect.soft(x).resolves.customHelper()` must NOT match

### Out of Scope
- Chai vocabulary (#42, done separately)
- Python nested function (#41)

## Design

### Approach: Two new scm patterns

Added depth-3 and depth-4 patterns after existing depth-2 pattern. Structure:
- Root: `expect.soft/element/poll(...)` (call_expression with member_expression)
- Modifier: `not|resolves|rejects` (one for depth-3, two for depth-4)
- Terminal: `^to[A-Z]` (constrains to Jest/Vitest naming convention)

### Critical constraint (reviewer feedback)

Terminal is constrained to `^to[A-Z]` at depth-3+. Without this, `expect.soft(x).resolves.customHelper()` would false-positive. Depth-2 (existing) has no terminal constraint, matching base `expect(x).toX()` design intent.

## Test List

| ID | Description | Type |
|----|-------------|------|
| B1 | `expect.soft(x).toBe(y)` depth-2 (regression) | >= 1 |
| B2 | `expect.soft(x).not.toBe(y)` depth-3 | >= 1 |
| B3 | `expect.soft(x).resolves.toBe(y)` depth-3 | >= 1 |
| B4 | `expect.soft(x).rejects.toThrow()` depth-3 | >= 1 |
| B5 | `expect.soft(x).resolves.not.toBe(y)` depth-4 | >= 1 |
| B6 | `expect.soft(x).rejects.not.toThrow(TypeError)` depth-4 | >= 1 |
| B7 | `expect.soft(x).resolves.customHelper()` (negative) | == 0 |
| B8 | no assertions (negative) | == 0 |
| B9 | `expect.element(loc).not.toHaveText('x')` depth-3 | >= 1 |
| B10 | `expect.poll(fn).not.toBe(0)` depth-3 | >= 1 |

## Result

- All 10 test cases pass. New fixture file created.
- Commit: 5b9fe3c

## Dogfooding Impact (vitest)

| Metric | Before (#40) | After (#42+#43) | Delta |
|--------|-------------|-----------------|-------|
| T001 BLOCK | 350 | 326 | -24 |

**vitest T001 dogfooding declared COMPLETE.** Remaining 326 BLOCKs are predominantly project-local/custom assertion helpers, better handled via `.exspec.toml` escape hatches rather than further generic query expansion.

Cumulative Phase 6 improvement: 432 -> 326 (-106, -24.5%).

## Files Changed

| File | Action |
|------|--------|
| `crates/lang-typescript/queries/assertion.scm` | depth-3/4 patterns added |
| `tests/fixtures/typescript/t001_expect_soft_chain.test.ts` | New fixture (10 TC) |
| `crates/lang-typescript/src/lib.rs` | Rust test added |
