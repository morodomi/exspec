---
feature: "Fix #96 — Rust observe: crate name import support for integration tests"
cycle: "20260318_0022"
phase: DONE
complexity: standard
test_count: 9
risk_level: low
codex_session_id: ""
created: 2026-03-18 00:22
updated: 2026-03-18 00:22
---

# Cycle: Fix #96 — Rust observe: crate name import support for integration tests

## Scope Definition

### In Scope
- [ ] `crates/lang-rust/src/observe.rs` 編集 — `parse_crate_name` 追加、`extract_use_declaration` 拡張、`extract_import_specifiers_with_crate_name` 追加、`map_test_files_with_imports` 修正

### Out of Scope
- Cargo workspace クロスクレート import 解決
- `use super::*` の外部テストファイルからの解決
- 他言語 observe への影響
- trait シグネチャ変更 (`extract_all_import_specifiers`)

### Files to Change
- `crates/lang-rust/src/observe.rs` (edit)

## Environment

### Scope
- Layer: `crates/lang-rust`
- Plugin: dev-crew:rust-quality (cargo test / clippy / fmt)
- Risk: 12/100 (PASS)
- Runtime: Rust (cargo test)
- Dependencies: `std::fs`, `std::path::Path` (Cargo.toml パース)

## Risk Interview

(BLOCK なし — リスク 12/100)

## Context & Dependencies

### 背景
Rust integration tests (`tests/` ディレクトリ) は独立クレートとしてコンパイルされるため、`use crate::...` ではなく `use my_crate::module::Symbol` でインポートする。現在の `extract_use_declaration` は `crate::` プレフィックスのみを処理しており、integration test の import を全て無視している。

tokio dogfooding: 271 test files のうち Layer 2 マッチが 0 件（Layer 0 の inline test のみ 40 件）。

### 参照ドキュメント
- `crates/lang-rust/src/observe.rs` (既存実装)
- `ROADMAP.md` (Phase 9c 完了済み)

## Implementation Notes

### Goal
Rust integration tests の `use my_crate::module::Symbol` 形式の import を Layer 2 マッチで解決できるようにする。

### Background
`extract_use_declaration` は現在 `crate::` プレフィックスのみを処理する。integration test は独立クレートのため `crate::` を使えず、クレート名プレフィックスを使う。

### Design Approach
trait シグネチャ (`extract_all_import_specifiers(&self, source: &str)`) を変更せず、`map_test_files_with_imports` (inherent method) 内で Cargo.toml をパースしてクレート名を解決し、`extract_import_specifiers_with_crate_name(source, crate_name)` に委譲する。

実装ステップ:
1. `parse_crate_name(scan_root: &Path) -> Option<String>` を追加
2. `extract_use_declaration` に `crate_name: Option<&str>` パラメータ追加
3. `extract_import_specifiers_with_crate_name(source, crate_name)` を追加
4. trait の `extract_all_import_specifiers` を委譲に変更
5. `map_test_files_with_imports` を修正

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] RS-CRATE-01: parse_crate_name: 正常パース
- [x] RS-CRATE-02: parse_crate_name: ハイフンなし
- [x] RS-CRATE-03: parse_crate_name: ファイルなし
- [x] RS-CRATE-04: parse_crate_name: workspace (package なし)
- [x] RS-IMP-05: crate_name simple import
- [x] RS-IMP-06: crate_name use list
- [x] RS-IMP-07: crate_name=None ではスキップ
- [x] RS-IMP-08: crate:: と crate_name:: 混在
- [x] RS-L2-INTEG: 統合テスト (tempdir)

## Progress Log

- 2026-03-18 00:22: Cycle doc created, starting RED phase
- 2026-03-18: RED phase — 9 tests added to crates/lang-rust/src/observe.rs; RED state verified (8 compile errors: parse_crate_name x4, extract_import_specifiers_with_crate_name x4)

### Phase: SYNC-PLAN - Completed at 00:22
**Artifacts**: Cycle doc created with PLAN section, Test List (9 items)
**Decisions**: architecture=inherent method delegation (trait unchanged), test strategy=unit + integration (tempdir)
**Pre-Review**: verdict=WARN, score=35, issues=hyphen-underscore conversion boundary, scan_root crate root assumption
**Next Phase Input**: Test List items RS-CRATE-01 ~ RS-L2-INTEG

- 2026-03-18: GREEN phase — parse_crate_name, extract_import_specifiers_with_crate_name, extract_use_declaration拡張を実装。103テスト全通過
- 2026-03-18: REFACTOR phase — チェックリスト7項目確認、改善不要。Verification Gate PASS (939 tests, clippy 0, fmt OK, BLOCK 0)
- Phase completed

### 2026-03-18 - REVIEW
- PASS (score=15), security=PASS, correctness=PASS
**Artifacts**: review results (mode: code)
**Decisions**: verdict=PASS, score=15, security=PASS(0), correctness=PASS(15)
**Next Phase Input**: all tests passing, ready to commit
- Phase completed

### 2026-03-18 - COMMIT
- feat: Rust observe crate name import support (#96)
- Phase completed
