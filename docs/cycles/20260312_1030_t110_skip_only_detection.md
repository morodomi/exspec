# Cycle: T110 skip-only-test detection (INFO)

**Issue**: #65
**Created**: 2026-03-12
**Status**: DONE

## Goal

skip呼び出しを含みassertionのないテスト関数をINFOとして検出する新ルールT110を追加する。
#64でT001から除外されたskip系テストを補完し、技術的負債として可視化する。

## Design

T108 (wait-and-see) パターンを踏襲するper-functionルール。

**条件**: `has_skip_call == true && assertion_count == 0`
- T001が`!has_skip_call`で除外済み → T110がそれを拾う
- 新フィールド不要（`has_skip_call`は#64で追加済み）

**意図的なスコープ**: この条件は以下の両方を検出する:
1. skip呼び出し**のみ**のテスト（例: `pytest.skip("reason")`だけ）
2. skip呼び出し + ロジック + assertionなし（例: 条件分岐後にskip）

いずれも「assertionがなくskipを含む」点で技術的負債として等価。#64 Test List:5 の `skip + logic (no assert)` ケースも `has_skip_call=true` であり、T110の対象に含む。

**デフォルト severity**: INFO

**対象言語と検出範囲**:
- Python: `pytest.skip()`, `self.skipTest()`（関数body内の呼び出しのみ）
  - **非対応**: `@pytest.mark.skip` / `@pytest.mark.skipIf` デコレータ（skip_test.scm:3に明記済み）
- PHP: `$this->markTestSkipped()`, `$this->markTestIncomplete()`
- Rust/TypeScript: `has_skip_call: false`固定 → T110は発火しない

## Critical Files

| File | Change |
|------|--------|
| `crates/core/src/rules.rs` | KNOWN_RULE_IDS追加、evaluate_rules()にT110ロジック、ユニットテスト |
| `crates/core/src/output.rs` | RULE_REGISTRYにT110のRuleMeta追加（SARIF rule metadata） |
| `docs/SPEC.md` | T110仕様追加（検出範囲の制約を含む） |
| `tests/fixtures/python/t110_violation.py` | Python違反fixture |
| `tests/fixtures/php/t110_violation.php` | PHP違反fixture |
| `tests/integration/` | 統合テスト |
| `README.md` | ルール数 16→17、Tier 2 範囲 T101-T109→T101-T110、INFO数 5→6、disable例にT110追加 |
| `docs/STATUS.md` | Active Rules表にT110行追加 |

## Test List

1. Given skip-only test (has_skip_call=true, assertion_count=0), When evaluate_rules, Then T110 INFO diagnostic
2. Given skip+assertion test (has_skip_call=true, assertion_count>0), When evaluate_rules, Then no T110
3a. Given normal test with assertions (has_skip_call=false, assertion_count>0), When evaluate_rules, Then no T001, no T110
3b. Given assertion-free non-skip test (has_skip_call=false, assertion_count=0), When evaluate_rules, Then T001 fires, no T110 (排他関係の証明)
4. Given T110 disabled in config, When evaluate_rules, Then no T110
5. Given T110 suppressed inline, When evaluate_rules, Then no T110
6. Given Python fixture t110_violation.py, When extract+evaluate, Then T110 fires
7. Given PHP fixture t110_violation.php, When extract+evaluate, Then T110 fires
8. Regression: existing t001_pass_skip_only fixtures still suppress T001 (回帰確認)
9. Regression: all existing tests pass
10. SARIF output includes T110 in rules array (RULE_REGISTRY整合性)

## Design Notes

- T006 (low-assertion-density) との相互作用: skip-onlyテストはdensity計算に含まれる（現状維持）
- KNOWN_RULE_IDS + RULE_REGISTRY の両方にT110を追加（SARIF整合性）
- 既存の t001_pass_skip_only fixtures は T110 violation fixtures としても再利用可能

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .  # self-dogfooding: BLOCK 0
```

## Phase Log

| Phase | Status | Notes |
|-------|--------|-------|
| KICKOFF | DONE | Cycle doc作成。レビュー指摘3点反映: RULE_REGISTRY追加、スコープ明示、Python検出範囲caveat |
| RED | DONE | Core/Python/PHP failing tests added for T110 coverage |
| GREEN | DONE | T110 rule, registry, docs, fixtures implemented |
| REFACTOR | DONE | SARIF registry test generalized to avoid per-rule maintenance |
| REVIEW | DONE | Review feedback absorbed with extra regression coverage |
| COMMIT | DONE | Docs finalized and changes committed |

## Progress Log

### 2026-03-12 - RED
- Added failing tests for T110 in core rules, SARIF registry, and Python/PHP fixture-based extraction paths.
- Added dedicated T110 violation fixtures for Python and PHP.
- Phase completed

### 2026-03-12 - GREEN
- Implemented T110 in rule evaluation, config validation, and SARIF rule metadata.
- Synced README, SPEC, and STATUS docs with the new rule definition and counts.
- Phase completed

### 2026-03-12 - REFACTOR
- Replaced T110-specific SARIF test with generic `sarif_rules_include_all_registry_entries` that validates all RULE_REGISTRY entries. Eliminates per-rule SARIF test maintenance.
- Simplify review (3-agent parallel): no actionable issues beyond the SARIF test generalization. All other findings were pre-existing architectural patterns.
- Phase completed

### 2026-03-12 - REVIEW
- Code review returned WARN with 1 important and 2 optional items; all three were addressed in the same cycle.
- Added regression tests for existing skip-only fixtures, T006 density participation, and PHP `markTestIncomplete` coverage.
- Phase completed

### 2026-03-12 - COMMIT
- Verified `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` passed after the follow-up review fixes.
- Finalized cycle documentation and project status for T110 completion.
- Phase completed
