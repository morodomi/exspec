# Cycle: T104 hardcoded-only + T105 deterministic-no-metamorphic

**Date**: 2026-03-07
**Type**: Feature (Tier 2 remaining rules)

## Goal

Tier 2 残り2ルールを実装し、Phase 5B を完了する。

- T104: テスト関数内のassertionが全てハードコードリテラルのみかを検出
- T105: ファイル内の全assertionが完全一致（equality）のみで、関係演算がないかを検出

## Scope

- Python + TypeScript 対応
- PHP/Rust: deferred (false固定でノイズ抑制)
- CLAUDE.md: T105 severity WARN → INFO 変更

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| T104: Node API (not .scm query) | callee除外ロジックがquery predicatesでは表現困難 |
| T104: callee chain walk (TS) | `expect(add(1,2)).toBe(3)` で `add` をcalleeとして除外するため |
| T105: INFO (WARN→変更) | equalityのみのファイルは多く、WARNだとノイズが大きい |
| T105: 閾値 ≥5 | 小さいファイルのノイズ防止 |
| T105: relational_assertion.scm | T103/T005と同じhas_any_matchパターン |

## Test List

### core/rules.rs
- [x] `t104_hardcoded_only_produces_info` - hardcoded_only=true → INFO
- [x] `t104_not_hardcoded_no_diagnostic` - hardcoded_only=false → no diagnostic
- [x] `t104_disabled_no_diagnostic` - disabled → no diagnostic
- [x] `t104_suppressed_no_diagnostic` - suppressed → no diagnostic
- [x] `t105_all_equality_above_threshold_produces_info` - 5+ equality → INFO
- [x] `t105_has_relational_no_diagnostic` - relational=true → no diagnostic
- [x] `t105_below_threshold_no_diagnostic` - 2 assertions → no diagnostic
- [x] `t105_disabled_no_diagnostic` - disabled → no diagnostic
- [x] `t105_custom_threshold` - min_assertions_for_t105=3 → fires at 3

### core/extractor.rs
- [x] `test_analysis_default_all_zero_or_empty` - hardcoded_only default false
- [x] `file_analysis_fields_accessible` - has_relational_assertion accessible

### core/config.rs
- [x] `parse_valid_config` - min_assertions_for_t105 parsed
- [x] `convert_full_config_to_rules_config` - min_assertions_for_t105 converted

### core/output.rs
- [x] `sarif_rules_has_13_entries` - RULE_REGISTRY 11→13

### lang-python
- [x] `hardcoded_only_violation` - `assert add(1,2) == 3` → hardcoded_only=true
- [x] `hardcoded_only_pass_variable` - `assert result == 3` → false
- [x] `hardcoded_only_pass_computed` - `assert decode(encode(data)) == data` → false
- [x] `hardcoded_only_no_assertion` - assertion無し → false
- [x] `hardcoded_only_pass_loop` - `for x: assert f(x) == x` → false
- [x] `relational_assertion_violation` - all `==` → has_relational=false
- [x] `relational_assertion_pass_greater_than` - `assert x > 0` → true
- [x] `relational_assertion_pass_contains` - `assert x in y` → true
- [x] `relational_assertion_pass_unittest` - `self.assertGreater()` → true
- [x] `query_capture_names_relational_assertion` - @relational capture exists

### lang-typescript
- [x] `hardcoded_only_violation` - `expect(add(1,2)).toBe(3)` → hardcoded_only=true
- [x] `hardcoded_only_pass_variable` - `expect(result).toBe(3)` → false
- [x] `relational_assertion_violation` - all `toBe`/`toEqual` → has_relational=false
- [x] `relational_assertion_pass_greater_than` - `toBeGreaterThan()` → true
- [x] `relational_assertion_pass_truthy` - `toBeTruthy()` → true
- [x] `query_capture_names_relational_assertion` - @relational capture exists

## Phase Summary

| Phase | Status |
|-------|--------|
| SPEC | plan modeで設計 (外部レビュー済み) |
| KICKOFF | cycle doc (本ファイル、事後作成) |
| RED | fixtures作成 + struct fields + rule tests |
| GREEN | Node API detection (T104) + .scm query (T105) |
| REFACTOR | clippy + fmt 修正 |
| REVIEW | PASS (blocking_score: 40) |
| COMMIT | `9151e03` |

## Metrics

- Tests: 406 → 431 (+25)
- New files: 19 (fixtures + queries)
- Modified files: 11
- RULE_REGISTRY: 11 → 13

## Technical Notes

### T104: callee chain walk (TypeScript)

`expect(add(1,2)).toBe(3)` のAST構造:
```
call_expression (outer: .toBe(3))
  function: member_expression
    object: call_expression (inner: expect(add(1,2)))
      function: identifier "expect"
      arguments: [call_expression "add(1,2)"]
    property: "toBe"
  arguments: [3]
```

外側call_expressionのfunction fieldをスキップすると、`expect()`の引数にある`add(1,2)`も見落とす。
→ `has_non_callee_identifier_in_callee_chain()` で callee chain を辿りながら各levelのargumentsをチェック。

### T105: Python comparison_operator

Python の `comparison_operator` ノードは `==` も `>` も同じ型。
`operators` フィールドで `>`, `<`, `>=`, `<=`, `in`, `not in`, `is`, `is not` を個別マッチし、`==`/`!=` を除外。

## DISCOVERED

| # | Issue | Severity | Action |
|---|-------|----------|--------|
| 1 | Python T104: `keyword_argument` のname fieldが変数と誤認識 (`assert add(x=1, y=2) == 3` で false negative) | important | issue起票 |
| 2 | TypeScript T104: テストカバレッジがPythonより薄い (pass_computed, no_assertion, .not chain 未テスト) | important | issue起票 |
| 3 | Python T105: `is`/`is not` がrelationalとしてT105を抑制 (実質equality check) | important | 設計判断として維持 (ノイズ削減優先) |
| 4 | Python T105: `assertTrue`/`assertFalse` もT105を抑制 (property checkはexact equalityではない) | important | 設計判断として維持 (plan記載済み) |
| 5 | `sarif_rules_has_10_entries` テスト名が古い (実際は13) | optional | issue起票 |
| 6 | PHP/Rust T104 deferral comments 欠如 + T104/T101 同時発火テスト未記述 | important | issue起票 |
