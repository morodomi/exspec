# Cycle: #42 T001 FP: Chai intermediate vocabulary + returned terminal

**Issue**: #42
**Date**: 2026-03-09
**Status**: DONE

## Goal

Expand Chai method-call chain intermediate vocabulary and add `returned` property terminal to reduce vitest T001 false positives.

## Background

After #40 added Chai method-call chain patterns, dogfooding revealed that Chai flag modifiers (`deep`, `nested`, `own`, `ordered`, `any`, `all`, `itself`) were not recognized as intermediates. Also, Sinon-Chai's `expect(spy).to.have.returned` (property terminal, no parens) was not detected.

## Scope

### In Scope
- Expand intermediate chain vocabulary: `deep|nested|own|ordered|any|all|itself`
- Depth-2: without `not` (to avoid overlap with existing modifier-chain patterns)
- Depth-3+: with `not`
- Add `returned` to Chai property terminal allowlist (depths 1-5)
- TC-11 exact-count regression test, TC-12~18 intermediate tests, TC-P6 returned

### Out of Scope
- expect.soft modifier chains (#43)
- Python nested function (#41)

## Design

### Approach: Vocabulary expansion in existing patterns

No new scm patterns needed. Extended the regex alternation in existing Chai patterns:
- Depth-2 chain: `^(to|be|been|have)$` -> `^(to|be|been|have|deep|nested|own|ordered|any|all|itself)$`
- Depth-3~5 chain: `^(to|be|been|have|not)$` -> `^(to|be|been|have|not|deep|nested|own|ordered|any|all|itself)$`
- Property terminal (all depths): added `returned`

### Double-counting safety

`expect(x).deep.equal(y)` AST: the member_expression object is `expect(x).deep` not `expect(x)`. Base pattern (L1-6) only matches object == `expect(x)`, so no overlap.

## Test List

| ID | Description | Type |
|----|-------------|------|
| TC-11 | `expect(x).to.equal(y)` exact count 1 (regression) | exact |
| TC-12 | `expect(obj).to.have.deep.equal({a:1})` | >= 1 |
| TC-13 | `expect(obj).to.have.nested.property('a.b')` | >= 1 |
| TC-14 | `expect(obj).to.have.own.property('x')` | >= 1 |
| TC-15 | `expect(arr).to.have.ordered.members([1,2])` | >= 1 |
| TC-16 | `expect(obj).to.have.any.keys('x')` | >= 1 |
| TC-17 | `expect(obj).to.have.all.keys('x','y')` | >= 1 |
| TC-18 | `expect(obj).itself.to.respondTo('bar')` | >= 1 |
| TC-P6 | `expect(spy).to.have.returned` (property) | >= 1 |
| deep-no-double | `to.have.deep.equal` exactly 1 | exact |

## Result

- All tests pass. fixture funcs.len() updated: method_call 10->18, property 5->6.
- Commit: 7ce5c8e

## Files Changed

| File | Action |
|------|--------|
| `crates/lang-typescript/queries/assertion.scm` | Intermediate vocabulary expansion + returned |
| `tests/fixtures/typescript/t001_chai_method_call.test.ts` | TC-11~18 added |
| `tests/fixtures/typescript/t001_chai_property.test.ts` | TC-P6 added |
| `crates/lang-typescript/src/lib.rs` | Rust tests updated |
