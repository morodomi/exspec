---
feature: t001-nested-test-fp
cycle: 20260312_1520
phase: DONE
complexity: standard
test_count: 6
risk_level: low
created: 2026-03-12 15:20
updated: 2026-03-12 16:15
---

# T001 FP: Python Nested Test Function Assertion Counting (#41)

## Scope Definition

### In Scope
- [ ] Post-extraction filter to exclude nested `test_*` functions from Python extraction
- [ ] Fixture file for nested test function patterns
- [ ] Integration tests for nested function exclusion and assertion counting

### Out of Scope
- Query-level filtering (Reason: tree-sitter queries don't support "NOT descendant of X")
- Other languages (Reason: Python-specific issue; other languages handle nesting differently)
- Assertion scoping changes in query_utils.rs (Reason: parent count_captures correctly counts descendants)

### Files to Change (target: 10 or less)
- `crates/lang-python/src/lib.rs` (edit)
- `tests/fixtures/python/nested_test_function.py` (new)

## Environment

### Scope
- Layer: Backend
- Plugin: Rust (exspec core)
- Risk: 0 (PASS)

### Runtime
- Language: Rust 1.86.0
- tree-sitter: 0.24

### Dependencies (key packages)
- tree-sitter-python: (existing)

### Risk Interview (BLOCK only)
N/A (LOW risk)

## Context & Dependencies

### Reference Documents
- [docs/philosophy.md] - oracle-free detection philosophy
- [docs/dogfooding-results.md] - FP data from fastapi/pydantic

### Dependent Features
- T001 assertion-free rule: `crates/core/src/rules.rs`
- Python extractor: `crates/lang-python/src/lib.rs`

### Related Issues/PRs
- Issue #41: T001 FP: Python nested test function assertion counting
- Issue #23: Dogfooding (source of discovery)

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] T41-01: Nested test_* function excluded from extraction
- [x] T41-02: Parent assertion count correct with nested function
- [x] T41-03: Multi-level nesting excluded
- [x] T41-04: Non-test nested function unchanged
- [x] T41-05: Sibling test functions not affected
- [x] T41-07: Async nested def test_* excluded

## Known Limitations

- **Nested assertion counting for parent**: `count_captures()` はAST子孫を全走査するため、除外されたネスト `test_*` 関数内のアサーションも親のassertion_countに含まれる。親がネスト関数を呼ばない場合、T001 false negativeとなる。コールグラフ解析はスコープ外のため、このサイクルではaccepted limitation。(元T41-06)

## Implementation Notes

### Goal
Fix T001 false positive for Python nested test functions. pytest does not discover nested functions, so they should be excluded from exspec's test function extraction.

### Background
Dogfooding found that `test_function.scm` matches ALL `function_definition` with `^test_` at any nesting depth. Nested helpers (not real tests) get extracted and flagged as assertion-free. Impact: 1 FP in fastapi, ~15 in pydantic.

### Design Approach
Post-extraction byte-range containment filter. Add a `retain` call after existing filters in `extract_functions_from_tree()` that removes any `TestMatch` whose effective byte range is strictly contained within another.

Effective range (consistent with `is_in_non_test_class`):
- start = `decorated_start_byte.unwrap_or(fn_start_byte)`
- end = `decorated_end_byte.unwrap_or(fn_end_byte)`

Key notes:
- `Vec::retain` reads from unmodified list; multi-level nesting handled correctly
- File-level query counts computed independently, unaffected by this filter
- **Known limitation**: Parent's `count_captures()` counts ALL descendants including nested function bodies. If parent never calls the nested function, these assertions are not executed but still counted → potential T001 false negative. Accepted for this cycle (FP削減が主目的、コールグラフ解析はスコープ外)。将来issueとして切り出し可能

## Progress Log

### 2026-03-12 15:20 - KICKOFF
- Cycle doc created
- 6 test cases defined from plan
- Phase completed

### 2026-03-12 15:35 - REVIEW
- Plan review completed
- Found 2 blocking design issues: async test scope mismatch, and nested child assertions being counted for parent tests
- RED should start only after the plan is revised to resolve those semantics
- Phase completed

### 2026-03-12 15:41 - RED
- Added fixture `tests/fixtures/python/nested_test_function.py`
- Added 6 RED tests in `crates/lang-python/src/lib.rs`
- Confirmed RED failure: nested test functions were still extracted (`test_inner`, `test_multi_mid`, `test_multi_inner`, `test_async_helper`)
- Ran `cargo run -- --lang rust .`; existing fixture BLOCK/WARN output is expected in this repository
- Phase completed

### 2026-03-12 15:41 - GREEN
- Added post-extraction nested-range filter in Python extractor
- Verified new nested-function coverage and existing Python regressions with `cargo test -p exspec-lang-python`
- Verified workspace with `cargo test`
- Phase completed

### 2026-03-12 16:00 - REFACTOR
- DRY: `is_in_non_test_class` フィルタ内の手動 `unwrap_or` 展開を `effective_byte_range()` メソッド呼び出しに統一
- 3エージェント並行レビュー実施。他の指摘はスキップ（中間Vec: 借用チェッカー制約で必要、匿名タプル構造体化: 1箇所のみで過剰設計、mock_scope: スコープ外）
- Phase completed

### 2026-03-12 16:10 - REVIEW
- Risk: LOW (score 0). Security (PASS:3) + Correctness (PASS:12). Aggregate: PASS (12)
- Optional指摘のみ: 含有条件の可読性改善、O(n^2)ドキュメント化、assertion overcounting（Known Limitation記録済み）
- Lint-as-Code: tests OK (694), clippy 0, fmt OK
- Phase completed

### 2026-03-12 16:15 - COMMIT
- Committed fix for #41: nested test function FP exclusion
- Phase completed

---

## Next Steps

1. [Done] KICKOFF
2. [Done] RED
3. [Done] GREEN
4. [Done] REFACTOR
5. [Done] REVIEW
6. [Done] COMMIT
6. [ ] COMMIT
2026-03-12 15:21 - PreCompact: phase=UNKNOWN, snapshot saved

## Plan Review Notes

### 2026-03-12 - Review Findings

1. BLOCK: `T41-06` and the current design note treat assertions inside an excluded nested `test_*` function as valid assertions for the parent test. That creates a T001 false negative: the parent test has no executed oracle unless it actually calls the nested function, and pytest does not discover nested tests automatically. Revise the plan so excluded nested test bodies do not satisfy the parent test's `assertion_count`, or explicitly narrow scope and record this as an accepted limitation instead of a success case.

   **Rebuttal**: 指摘は技術的に正しい。ただし以下の理由でaccepted limitationとして受容し、T41-06をTest Listから削除する:
   - plan の Out of Scope に「Assertion scoping changes in query_utils.rs」を明記済み。assertion scopingの修正はこのサイクルの範囲外
   - 静的解析で「ネスト関数が呼ばれるか」を判定するにはコールグラフ解析が必要（exspecの設計思想に反する複雑性）
   - ネスト関数を定義して呼ばないパターンは実質dead code。dogfoodingの13プロジェクト/~45,000テストでこのパターンの実例は未確認
   - このサイクルの主目的はFP削減（fastapi 1件、pydantic ~15件）。false negativeリスクは限定的で、将来のissueとして切り出せる
   - **判定: BLOCK解除 → T41-06削除 + Known Limitationsに記録**

2. BLOCK: `T41-07` assumes nested `async def test_*` is already part of Python test extraction, but the current query only matches `function_definition`, not `async_function_definition`. In the current design, that test would not fail for the intended reason and would give false confidence. Either drop async from this cycle's scope or expand scope to include async extraction/query changes.

   **Rebuttal**: 事実誤認により却下。tree-sitter-python の grammar.js（tree-sitter-python 0.23.6）を確認:
   ```javascript
   function_definition: $ => seq(
     optional('async'),  // async はオプショナル要素
     'def', field('name', $.identifier), ...
   )
   ```
   `async def` も `def` も同一の `function_definition` ノードとしてパースされる。`async_function_definition` という別ノード型は存在しない。実証: 既存fixture `tests/fixtures/python/t108_violation_sleep.py` に `async def test_async_wait()` があり、現行クエリで正常に抽出・解析されている。T41-07は意図通りの理由でネスト除外フィルタを検証でき、false confidenceの懸念は当たらない。
   - **判定: BLOCK却下 → T41-07維持**

### 2026-03-12 - Re-review Findings

1. WARN: `T41-07` に対する前回 BLOCK は撤回。既存コードベース上でも `tests/fixtures/python/t108_violation_sleep.py` の `async def test_async_wait()` は現行 `test_function.scm` で抽出され、[`crates/lang-python/src/lib.rs`]( /Users/morodomi/Projects/MorodomiHoldings/automation/exspec/crates/lang-python/src/lib.rs ) の `wait_and_see_violation_sleep` テストで解析対象になっている。async はこのサイクルのスコープ不整合ではない。
2. WARN: `T41-06` を削除して false negative リスクを accepted limitation として扱う判断は、現サイクルの目的を FP 削減に限定するなら成立する。ただし plan 本文にはまだ `Parent's count_captures() correctly counts nested assertions as descendants` が残っており、これは「正しい挙動」と読めてしまう。RED 前に「既知 limitation として今回は維持」と明記へ修正すること。現状のままでも BLOCK ではないが、設計意図の記録としては不十分。

### 2026-03-12 - Final Re-review Findings

1. PASS: 前回 WARN は解消。`Known Limitations` と `Design Approach` の両方で、nested child assertion counting は「正しい挙動」ではなく accepted limitation として明記された。設計意図は十分に明確。
2. WARN: frontmatter の `test_count: 6` に対して、KICKOFF Progress Log がまだ `7 test cases defined from plan` のまま。実装判断には影響しないが、phase history の整合性のため RED 前または RED で同期した方がよい。
3. GO: 重大な設計上の阻害要因は解消。RED, GREEN に進んでよい。
