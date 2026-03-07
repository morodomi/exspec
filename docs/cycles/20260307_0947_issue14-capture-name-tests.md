---
feature: query-capture-verification
cycle: issue14-capture-name-tests
phase: DONE
created: 2026-03-07 09:47
updated: 2026-03-07 09:47
---

# Issue #14: has_any_match capture name verification tests

## Scope Definition

### In Scope
- [x] Python: 7 capture name verification tests
- [x] TypeScript: 7 capture name verification tests
- [x] PHP: 7 capture name verification tests (6 exists + 1 comment-only)
- [x] Rust: 7 capture name verification tests (6 exists + 1 comment-only)
- [x] docs/STATUS.md: テスト数更新 (317 -> 345)

### Out of Scope
- `count_captures` / `has_any_match` エラーセマンティクス統一 (DISCOVERED候補)

### Files Changed (4 + 1)
- crates/lang-python/src/lib.rs - edit: 7 tests追加
- crates/lang-typescript/src/lib.rs - edit: 7 tests追加
- crates/lang-php/src/lib.rs - edit: 7 tests追加
- crates/lang-rust/src/lib.rs - edit: 7 tests追加
- docs/STATUS.md - edit: テスト数更新

## Context

`query_utils::has_any_match()` はcapture nameが存在しない場合、silentに `false` を返す。
これはcomment-only .scmファイル (PHP import_pbt.scm, Rust import_contract.scm) では意図的だが、
.scmファイルからcapture nameが誤って削除されたリグレッションを検出できない。

## Design Decision

各言語の `#[cfg(test)] mod tests` ブロックに `make_query()` ヘルパーを追加し、
7つのクエリファイルごとに期待されるcapture nameの存在を検証する。

Comment-onlyファイルは `is_none()` でアサートし、将来ライブラリが追加された際に
テストが失敗してcall siteの更新を促すようにした。

## Phase Summary: RED + GREEN (combined)

テスト追加のみ（実装変更なし）のため、RED/GREENを統合。

- 28 tests added (7 per language)
- All 345 tests passing
- clippy clean, fmt clean

### Verified captures per query file

| Query file | Python | TypeScript | PHP | Rust |
|-----------|--------|-----------|-----|------|
| test_function.scm | name, function, decorated | name, function | name, function | test_attr |
| assertion.scm | assertion | assertion | assertion | assertion |
| mock_usage.scm | mock | mock | mock | mock |
| mock_assignment.scm | var_name | var_name | var_name | var_name |
| parameterized.scm | parameterized | parameterized | parameterized | parameterized |
| import_pbt.scm | pbt_import | pbt_import | NONE (comment-only) | pbt_import |
| import_contract.scm | contract_import | contract_import | contract_import | NONE (comment-only) |
