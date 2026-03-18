---
feature: "Fix #100 — Rust observe: implement file_exports_any_symbol"
cycle: "20260318_1130"
phase: RED
complexity: standard
test_count: 8
risk_level: low
codex_session_id: ""
created: 2026-03-18 11:30
updated: 2026-03-18 11:30
---

# Cycle: Fix #100 — Rust observe: implement file_exports_any_symbol

## Scope Definition

### In Scope
- [ ] `crates/lang-rust/queries/exported_symbol.scm` 新規作成 — 7パターンの tree-sitter クエリ
- [ ] `crates/lang-rust/src/observe.rs` 変更 — `EXPORTED_SYMBOL_QUERY` 定数 + `file_exports_any_symbol` override + テスト
- [ ] `tests/fixtures/rust/observe/exported_pub_symbols.rs` 新規作成 — pub シンボル fixture
- [ ] `tests/fixtures/rust/observe/no_pub_symbols.rs` 新規作成 — pub なし fixture
- [ ] `tests/fixtures/rust/observe/pub_use_only.rs` 新規作成 — pub use/mod のみ fixture

### Out of Scope
- 他言語 observe への影響
- TypeScript / Python / PHP の `file_exports_any_symbol` 変更
- barrel 解決ロジック本体の変更

### Files to Change
- `crates/lang-rust/queries/exported_symbol.scm` (new)
- `crates/lang-rust/src/observe.rs` (edit)
- `tests/fixtures/rust/observe/exported_pub_symbols.rs` (new)
- `tests/fixtures/rust/observe/no_pub_symbols.rs` (new)
- `tests/fixtures/rust/observe/pub_use_only.rs` (new)

## Environment

### Scope
- Layer: `crates/lang-rust`
- Plugin: dev-crew:rust-quality (cargo test / clippy / fmt)
- Risk: 20/100 (PASS)
- Runtime: Rust (cargo test)
- Dependencies: tree-sitter (既存)

## Risk Interview

(BLOCK なし — リスク 20/100)

## Context & Dependencies

### 背景
`RustExtractor` が `file_exports_any_symbol` を override しておらず、デフォルト `true` を返している。Issue #99 で `pub mod` が `wildcard: true` の `BarrelReExport` として扱われるようになったため、barrel 解決チェーン (`resolve_barrel_exports_inner` L301-303) でシンボルフィルタが機能せず、FP が発生する。

`file_exports_any_symbol` は TypeScript 実装が参照実装。tree-sitter で `pub` visibility を持つトップレベル定義（`pub fn`, `pub struct`, `pub enum` 等）を検出し、要求シンボルのいずれかが含まれているかを返す。

### 参照ドキュメント
- `crates/lang-rust/src/observe.rs` (既存実装)
- `crates/lang-typescript/src/observe.rs` (参照実装: `file_exports_any_symbol`)
- `ROADMAP.md` (Phase 9d)
- Issue #99: pub mod wildcard fix (直前のサイクル)

### Related Issues/PRs
- Issue #100: Rust `file_exports_any_symbol` 未実装

## Test List

### TODO
- [ ] RS-EXPORT-01: pub fn マッチ — `exported_pub_symbols.rs` に `pub fn create_user` | `file_exports_any_symbol(path, ["create_user"])` | `true`
- [ ] RS-EXPORT-02: pub struct マッチ — 同ファイルに `pub struct User` | `file_exports_any_symbol(path, ["User"])` | `true`
- [ ] RS-EXPORT-03: 存在しないシンボル — 同ファイルに `NonExistent` なし | `file_exports_any_symbol(path, ["NonExistent"])` | `false`
- [ ] RS-EXPORT-04: pub なしファイル — `no_pub_symbols.rs` に pub シンボルなし | `file_exports_any_symbol(path, ["internal_only"])` | `false`
- [ ] RS-EXPORT-05: pub use/mod のみ — `pub_use_only.rs` に `pub use`/`pub mod` のみ | `file_exports_any_symbol(path, ["Foo"])` | `false`
- [ ] RS-EXPORT-06: 空シンボルリスト — 任意のファイル | `file_exports_any_symbol(path, [])` | `true`
- [ ] RS-EXPORT-07: ファイル不在 — 存在しないパス | `file_exports_any_symbol(nonexistent, ["Foo"])` | `true` (楽観的)
- [ ] RS-EXPORT-08: 既存テスト全通過 — 実装後 | `cargo test -p exspec-lang-rust` | 全テスト PASS

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal
`RustExtractor::file_exports_any_symbol` を実装し、`pub` visibility を持つトップレベル定義のシンボルフィルタを有効にする。これにより Issue #99 の wildcard fix と連動して FP を抑制する。

### Background
デフォルト実装は常に `true` を返すため、`pub mod` が wildcard barrel として解決された場合にシンボルフィルタが完全に機能しない。TypeScript 実装と同様に、tree-sitter で実ファイルの pub シンボル一覧を抽出し、要求シンボルとの照合を行う必要がある。

### Design Approach
1. `exported_symbol.scm` 新規作成 — `pub` visibility を持つ 7 種のトップレベル定義を `@symbol_name` でキャプチャ
   - `function_item`, `struct_item`, `enum_item`, `type_item`, `const_item`, `static_item`, `trait_item`
2. `observe.rs` に `EXPORTED_SYMBOL_QUERY` 定数を追加
3. `impl ObserveExtractor for RustExtractor` に `file_exports_any_symbol` を追加 — TypeScript 実装に準拠
4. fixture 3ファイルを `tests/fixtures/rust/observe/` に作成
5. テスト 8件を `crates/lang-rust/src/observe.rs` の `#[cfg(test)]` 内に追加

## Progress Log

### 2026-03-18 11:30 - INIT
- Cycle doc created
- Plan transferred from approved plan file

## Next Steps

1. [Done] INIT <- Current
2. [Next] RED
3. [ ] GREEN
4. [ ] REFACTOR
5. [ ] REVIEW
6. [ ] COMMIT
