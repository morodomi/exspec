---
feature: "Fix #99 — Rust observe: pub mod を wildcard re-export として扱う"
cycle: "20260318_0100"
phase: DONE
complexity: trivial
test_count: 4
risk_level: low
codex_session_id: ""
created: 2026-03-18 01:00
updated: 2026-03-18 01:00
---

# Cycle: Fix #99 — Rust observe: pub mod を wildcard re-export として扱う

## Scope Definition

### In Scope
- [ ] `crates/lang-rust/src/observe.rs` 編集 — `extract_barrel_re_exports` の `pub mod foo;` 分岐で `wildcard: true` に変更
- [ ] 既存テスト `rs_barrel_03_pub_mod` の期待値を `wildcard: true` に更新
- [ ] 2段・3段 re-export の Layer 2 マッチ統合テスト追加

### Out of Scope
- 他言語 observe への影響
- TypeScript barrel resolution の変更
- `pub use` の挙動変更

### Files to Change
- `crates/lang-rust/src/observe.rs` (edit)

## Environment

### Scope
- Layer: `crates/lang-rust`
- Plugin: dev-crew:rust-quality (cargo test / clippy / fmt)
- Risk: 15/100 (PASS)
- Runtime: Rust (cargo test)
- Dependencies: tree-sitter (既存)

## Risk Interview

(BLOCK なし — リスク 15/100)

## Context & Dependencies

### 背景
`extract_barrel_re_exports` は `pub mod foo;` を検出した場合に `wildcard: false` で re-export エントリを生成している。しかし `pub mod foo;` はモジュール全体を公開するため、`pub use foo::*;` と等価な wildcard 扱いが正しい。この誤りにより、`pub mod` を介した multi-hop import chain で Layer 2 マッチが機能しない。

tokio dogfooding では `src/models/mod.rs` → `pub mod user;` → `src/models/user.rs` のような2段・3段 re-export が多数存在し、現状では Layer 2 マッチが抜け落ちている。

### 参照ドキュメント
- `crates/lang-rust/src/observe.rs` (既存実装)
- `ROADMAP.md` (Phase 9d 完了済み)
- `docs/dogfooding-results.md` (Rust observe dogfooding 結果)

### Related Issues/PRs
- Issue #99: Rust observe: pub mod を wildcard re-export として扱う

## Test List

### TODO
- [ ] RS-BARREL-PUB-MOD-01: pub mod は wildcard として扱われる — `pub mod user;` in mod.rs → `extract_barrel_re_exports` → `wildcard: true`
- [ ] RS-DEEP-REEXPORT-01: 2段 re-export マッチ — tempdir: `src/models/mod.rs` に `pub mod user;`, `src/models/user.rs` 存在, `tests/test_models.rs` に `use my_crate::models::User;` → `map_test_files_with_imports` → test_models.rs → user.rs マッチ (Layer 2)
- [ ] RS-DEEP-REEXPORT-02: 3段 re-export マッチ — tempdir: `src/lib.rs` に `pub mod models;`, `src/models/mod.rs` に `pub mod user;`, `src/models/user.rs`, `tests/test_user.rs` に `use my_crate::models::user::User;` → `map_test_files_with_imports` → test_user.rs → user.rs マッチ (Layer 2)
- [ ] RS-DEEP-REEXPORT-03: pub use + pub mod 混在 — tempdir: mod.rs に `pub mod internal;` + `pub use internal::Exported;`, `internal.rs` 存在 → `extract_barrel_re_exports` → wildcard=true (pub mod) + symbols=["Exported"] (pub use) 両方返す

### WIP
(none)

### DISCOVERED
- [x] Rust `file_exports_any_symbol` 未実装: デフォルト (常に true) のためシンボルフィルタが機能せず FP リスクあり → issue #100

### DONE
(none)

## Implementation Notes

### Goal
`pub mod foo;` を `wildcard: true` の re-export として扱い、2段・3段の `pub mod` チェーンを介した Layer 2 マッチを有効にする。

### Background
`extract_barrel_re_exports` の `pub mod foo;` 分岐では現在 `wildcard: false` で エントリを生成している。`pub mod` はモジュール全体を親 namespace に公開するため、`pub use foo::*;` と等価であり `wildcard: true` が正しい。

### Design Approach
変更は `crates/lang-rust/src/observe.rs` の1箇所のみ:

1. `extract_barrel_re_exports` の `pub mod foo;` 分岐で `wildcard: false` → `wildcard: true` に変更 (1行)
2. 既存テスト `rs_barrel_03_pub_mod` の期待値を `wildcard: true` に更新

追加テスト (RS-DEEP-REEXPORT-01/02/03) は tempdir を使った統合テストとして `crates/lang-rust/src/observe.rs` の `#[cfg(test)]` 内に追加する。

## Progress Log

### 2026-03-18 01:00 - INIT
- Cycle doc created
- Scope definition ready

## Next Steps

1. [Done] INIT <- Current
2. [Next] RED
3. [ ] GREEN
4. [ ] REFACTOR
5. [ ] REVIEW
6. [ ] COMMIT
