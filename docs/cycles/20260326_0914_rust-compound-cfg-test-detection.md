---
feature: rust-compound-cfg-test-detection
cycle: 20260326_0914
phase: RED
complexity: trivial
test_count: 5
risk_level: low
codex_session_id: ""
created: 2026-03-26 09:14
updated: 2026-03-26 09:14
---

# Rust observe: compound cfg(test) inline detection

## Scope Definition

### In Scope
- [ ] `crates/lang-rust/queries/cfg_test.scm` に compound pattern を追加 (`cfg(all(test, ...))` / `cfg(any(test, ...))`)
- [ ] `crates/lang-rust/src/observe.rs` の `detect_inline_tests` が変更不要であることを確認 (既存ロジック流用)
- [ ] テスト fixture: `#[cfg(all(test, not(loom)))]` ケース
- [ ] テスト fixture: `#[cfg(any(test, fuzzing))]` ケース
- [ ] regression guard: `#[cfg(test)]` 既存パターンが継続マッチすること

### Out of Scope
- `task/mod.rs` の barrel + `cfg(all(test, ...))` パターン (barrel self-match #199 経由で対処済み・対象外)
- cross-crate FN (別サイクル)
- cfg に `test` を含まない attribute の検出

### Files to Change (target: 10 or less)
- `crates/lang-rust/queries/cfg_test.scm` (edit) — compound pattern 追加
- `crates/lang-rust/src/observe.rs` (確認のみ、変更不要の可能性)
- `tests/fixtures/rust/` または `crates/lang-rust/src/observe.rs` テストモジュール (new/edit) — TC-01〜TC-04 fixture 追加

## Environment

### Scope
- Layer: lang-rust (query layer)
- Plugin: rust
- Risk: 20 (PASS)

### Runtime
- Language: Rust (edition 2021)

### Dependencies (key packages)
- tree-sitter: workspace
- tree-sitter-rust: workspace

### Risk Interview (BLOCK only)
N/A — Risk 20 (PASS)

## Context & Dependencies

### Reference Documents
- [CONSTITUTION.md] — Ship criteria: P>=98%, R>=90%
- [ROADMAP.md] — Rust observe recall improvement (P1)
- [docs/languages/rust.md] — Rust observe 設計詳細

### Dependent Features
- L1 cross-directory subdir stem matching (#189): 既存の Rust observe ベース
- barrel self-match (#199 / 20260325_2303): barrel 経由の inline detection とは別経路

### Related Issues/PRs
- tokio: `orphan.rs` (`cfg(all(test, not(loom)))`), `linked_list.rs` (`cfg(any(test, fuzzing))`) が FN

## Test List

### TODO
- [ ] TC-01: Given `#[cfg(all(test, not(loom)))] mod test { #[test] fn foo() {} }`, When detect_inline_tests, Then true
- [ ] TC-02: Given `#[cfg(any(test, fuzzing))] mod tests { #[test] fn bar() {} }`, When detect_inline_tests, Then true
- [ ] TC-03: Given `#[cfg(all(not(test), feature = "bench"))]`, When detect_inline_tests, Then false (test が否定されているため)
- [ ] TC-04: Given `#[cfg(test)]` (simple existing pattern), When detect_inline_tests, Then true (regression guard)
- [ ] TC-05: Given tokio observe after fix, When count mapped inline files, Then >= 241 (239 + 2) [#ignore]

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal

`cfg_test.scm` に compound pattern を追加し、`cfg(all(test, ...))` / `cfg(any(test, ...))` を `detect_inline_tests` が検出できるようにする。tokio の `orphan.rs` と `linked_list.rs` の FN を解消し、Rust observe recall を改善する。

### Background

`detect_inline_tests` は `cfg_test.scm` クエリでマッチした attribute を検査する。現行クエリは `token_tree` 直下の `(identifier) test` のみ対象のため、`cfg(all(test, not(loom)))` のような nested `token_tree` 構造はマッチしない。tokio では loom concurrency テスト分離パターン (`cfg(all(test, not(loom)))`) と fuzz テスト共有パターン (`cfg(any(test, fuzzing))`) が該当。

### Design Approach

`cfg_test.scm` に Pattern 2 (nested token_tree) を追加する。`@attr_name == "cfg"` && `@cfg_arg == "test"` のフィルタは `detect_inline_tests` 側の既存ロジックでそのまま機能する:

```scheme
;; Pattern 1: #[cfg(test)] — simple (既存)
(attribute_item
  (attribute
    (identifier) @attr_name
    arguments: (token_tree
      (identifier) @cfg_arg))) @cfg_test_attr

;; Pattern 2: #[cfg(all(test, ...))] or #[cfg(any(test, ...))] — compound (追加)
(attribute_item
  (attribute
    (identifier) @attr_name
    arguments: (token_tree
      (token_tree
        (identifier) @cfg_arg)))) @cfg_test_attr
```

TC-03 (`cfg(all(not(test), ...))`) は `(not (identifier) test)` 構造のため `token_tree` 内の `(identifier) test` がなく、自然にマッチしない。追加コードなし。

**Impact 想定:**
- tokio: `orphan.rs` (+5 tests), `linked_list.rs` (+4 tests) = +2 inline files → mapped >= 241
- 他 library: `cfg(all(test, ...))` を使う全ライブラリで改善
- FP リスク: cfg に `test` を含む = テストモジュールの意図なので near-zero

## Design Review Gate

### Assessment

**PASS (trivial complexity confirmed)**

| 観点 | 判定 | 備考 |
|------|------|------|
| スコープ明確性 | PASS | query 拡張のみ。コード変更なし (or 最小) |
| FP リスク | PASS | `cfg` に `test` を含む = テスト意図。誤検出の余地なし |
| TC-03 (negated test) | PASS | `(not (identifier) test)` は `(identifier) test` を持たないため自然に除外 |
| 既存テストへの影響 | PASS | Pattern 1 はそのまま残存。regression なし |
| TC-05 integration | INFO | `#[ignore]` 付きで追加。CI では skip、手動確認 |
| 設計の過不足 | PASS | nested token_tree 1階層で `all`/`any` 両対応。深い nesting は現実のコードにほぼ存在しない |

**Blocking issues: 0**

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
# tokio integration (manual / #[ignore])
cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tokio
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-26 09:14 - INIT
- Cycle doc created from plan file `spicy-sauteeing-pine.md`
- Design Review Gate: PASS (trivial, blocking issues 0)
- Scope definition ready

---

## Next Steps

1. [Done] INIT
2. [Done] PLAN
3. [ ] RED
4. [ ] GREEN
5. [ ] REFACTOR
6. [ ] REVIEW
7. [ ] COMMIT
