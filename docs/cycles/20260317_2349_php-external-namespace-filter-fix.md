---
feature: "Fix #94: PHP external namespace filter causes false negatives for framework self-tests"
cycle: 20260317_2349
phase: DONE
complexity: trivial
test_count: 4
risk_level: low
codex_session_id: ""
created: 2026-03-17 23:49
updated: 2026-03-18
---

# Fix #94: PHP external namespace filter causes false negatives for framework self-tests

## Scope Definition

### In Scope
- [x] `is_external_namespace` に `scan_root: Option<&Path>` 引数を追加
- [x] `map_test_files_with_imports` で scan_root を考慮したフィルタリング (raw import 抽出 + scan_root 付きフィルタ + full namespace パス解決)

### Out of Scope
- PSR-4 精度向上 (#93) (別 Issue)
- Django tests.py (#95) (別 Issue)
- Rust crate prefix (#96) (別 Issue)

### Files to Change (target: 10 or less)
- `crates/lang-php/src/observe.rs` (edit)

## Environment

### Scope
- Layer: Backend
- Plugin: php
- Risk: 30 (PASS)

### Runtime
- Language: Rust

### Dependencies (key packages)
- tree-sitter: workspace
- lang-php: workspace

### Risk Interview (BLOCK only)
(N/A — Risk 30, PASS)

## Context & Dependencies

### Reference Documents
- `docs/languages/php.md` - PHP observe 設計
- `ROADMAP.md` - Phase 9d (PHP observe) 完了ログ

### Dependent Features
- PHP observe (Layer 2): `crates/lang-php/src/observe.rs`

### Related Issues/PRs
- Issue #94: PHP external namespace filter causes false negatives for framework self-tests

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] PHP-FW-01: laravel layout (`src/Illuminate/Http/Request.php` + `tests/Http/RequestTest.php` + `use Illuminate\Http\Request`) → map_test_files_with_imports → test mapped to prod via Layer 2
- [x] PHP-FW-02: normal app (`app/Models/User.php` + `tests/UserTest.php` + `use Illuminate\Http\Request`、ローカル Illuminate dir なし) → map_test_files_with_imports → Illuminate import filtered (no mapping via import)
- [x] PHP-FW-03: `use PHPUnit\Framework\TestCase`、ローカル PHPUnit source なし → integration test → PHPUnit still filtered (既存挙動維持 regression)
- [x] PHP-FW-04: symfony layout (`src/Symfony/Component/Request.php` + test + `use Symfony\Component\Request`) → map_test_files_with_imports → test mapped via Layer 2

## Implementation Notes

### Goal

PHP observe の `is_external_namespace()` がフレームワーク自身のソースを scan した場合（例: laravel/framework）に全 import をフィルタして 0% match になる問題を修正する。

### Background

PHP observe の `is_external_namespace()` が `Illuminate`, `Symfony` 等の既知フレームワーク名前空間を無条件にフィルタする。フレームワーク自体を scan した場合（例: laravel/framework）、全ての import がフィルタされ observe が 0% match になる。

Dogfooding: laravel/framework — 1888 prod / 894 test → 0 mapped。

Root Cause: `crates/lang-php/src/observe.rs` L170-175 の `is_external_namespace()` が `scan_root` を考慮せず EXTERNAL_NAMESPACES リストだけで判定。フレームワーク自身のソースも external 扱いになってしまう。

### Design Approach

`is_external_namespace` に `scan_root: Option<&Path>` 引数を追加する。

- `scan_root` が `Some(root)` の場合: 名前空間をパスに変換し `root` 配下に存在するか確認。存在すれば external ではない（=フレームワーク自身のソース）
- `scan_root` が `None` の場合: 既存の EXTERNAL_NAMESPACES リスト判定のみ（後方互換）

呼び出し箇所の修正:
- `extract_all_import_specifiers` (L287): `is_external_namespace(&fs_path, None)` — 従来互換
- `map_test_files_with_imports` (L381-447): Layer 2 ループ内で scan_root 付きフィルタ

## Progress Log

### 2026-03-17 23:49 - INIT
- Cycle doc created
- Scope definition ready

### 2026-03-18 - RED
- PHP-FW-01〜04 テスト4件作成
- PHP-FW-01 (laravel self-test), PHP-FW-04 (symfony self-test) が期待通り失敗
- PHP-FW-02 (normal app filtered), PHP-FW-03 (PHPUnit regression) が既存動作で PASS
- Phase completed

### 2026-03-18 - GREEN
- `is_external_namespace` に `scan_root: Option<&Path>` 引数追加
- `extract_raw_import_specifiers` private メソッド追加 (external フィルタなしの raw import 抽出)
- `map_test_files_with_imports` Layer 2 で raw imports + scan_root 付きフィルタに変更
- Layer 2 で full namespace パス (common prefix 付き) の解決を追加
- 全4テスト PASS
- Phase completed

### 2026-03-18 - REFACTOR
- 変更が小さく REFACTOR 不要
- Phase completed

### 2026-03-18 - REVIEW
- cargo test: 全テスト通過
- cargo clippy -- -D warnings: 0 errors
- cargo fmt --check: 差分なし
- cargo run -- --lang rust .: BLOCK 0
- Dogfooding: laravel/framework 0% → 49.8% mapped (968/1944)
- Codex review: skipped (usage limit reached, Mar 19 7:44 AM reset)
- Phase completed

### 2026-03-18 - COMMIT
- All gates passed
- Phase completed

---

## Next Steps

1. [Done] INIT
2. [Done] RED
3. [Done] GREEN
4. [Done] REFACTOR
5. [Done] REVIEW
6. [Done] COMMIT
