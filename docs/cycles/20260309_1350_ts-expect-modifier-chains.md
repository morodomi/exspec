# Cycle: #37 T001 FP: TS expect modifier chains (.not/.resolves)

**Issue**: #37
**Date**: 2026-03-09
**Status**: DONE

## Goal

Generalize the existing TypeScript assertion query for modifier chains so that `expect(...).not.toX()`, `expect(...).resolves.toX()`, and `expect(...).rejects.toX()` are all recognized. Add a second query for two-modifier chains such as `expect(...).resolves.not.toX()` and `expect(...).rejects.not.toX()`.

## Background

Dogfooding (#23) revealed ~85 vitest T001 false positives caused by undetected modifier chains. The assertion.scm only matches `expect(x).rejects.toX()` but not `.not` or `.resolves` modifiers. These chains share the same relevant member-expression shape for assertion extraction; only modifier property names differ.

## Scope

### In Scope
- Depth-2: generalize `rejects`-only to `not|resolves|rejects`
- Depth-3: combined modifier chains (e.g., `resolves.not.toX()`) with `^to[A-Z]` terminal matcher constraint
- Fixtures and integration tests for new patterns

### Out of Scope
- Chai method-call chains (#40)
- `expect.assertions(N)` / `expect.unreachable()` (#39)
- PHP/Python mock assertion patterns (#38)

## Design

### Approach
For simplicity, the extractor accepts any two-modifier combination from `not|resolves|rejects` before a `toX` matcher, even if some combinations (e.g., `not.resolves`) are uncommon in practice. The depth-3 query adds an explicit terminal matcher constraint (`^to[A-Z]`) following the Jest/Vitest matcher naming convention, to avoid counting arbitrary chained methods as assertions.

The existing depth-2 structure already constrains the outer `call_expression` to the called member chain `expect(x).modifier.toX(...)`, so no additional terminal matcher predicate is needed there.

### Files to Change

| File | Change |
|------|--------|
| `crates/lang-typescript/queries/assertion.scm` | Generalize depth-2, add depth-3 |
| `tests/fixtures/typescript/t001_not_modifier.test.ts` | New fixture |
| `tests/fixtures/typescript/t001_resolves_rejects_chain.test.ts` | New fixture |
| `crates/lang-typescript/src/lib.rs` | Integration tests |

## Environment

- Rust 1.88.0, tree-sitter 0.24
- Layer: Backend (Rust + tree-sitter .scm)
- Risk: 15 (PASS)

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] TC-01: `expect(x).not.toBe(y)` → assertion_count >= 1
- [x] TC-02: `expect(x).not.toEqual(y)` → assertion_count >= 1
- [x] TC-03: `expect(x).not.toContain(y)` → assertion_count >= 1
- [x] TC-04: `expect(p).resolves.toBe(y)` → assertion_count >= 1
- [x] TC-05: `expect(p).resolves.not.toThrow()` (depth-3) → assertion_count >= 1
- [x] TC-06: `expect(p).rejects.not.toThrow()` (depth-3, symmetry) → assertion_count >= 1
- [x] TC-07: Existing `rejects.toThrow` fixture → no regression (557 tests pass)
- [x] TC-08: Existing violation fixture → assertion_count == 0 (557 tests pass)
