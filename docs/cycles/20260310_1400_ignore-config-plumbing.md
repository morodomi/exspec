---
issue: "#54"
topic: "[paths] ignore config not applied to file discovery"
phase: REVIEW
created: 2026-03-10
updated: 2026-03-10
---

# Cycle: [paths] ignore config plumbing (#54)

## Problem

`.exspec.toml` の `[paths] ignore` はパースされるが、`discover_files()` に渡されていないため無視パターンが効かない。Self-dogfooding時にfixture（意図的な違反サンプル）がBLOCKとして検出される。

## Root Cause

- `ExspecConfig.paths.ignore` はパース済み
- `From<ExspecConfig> for Config` でパス情報が捨てられる
- `discover_files()` は `Config` を受け取らない

## Approach

`Config` に `ignore_patterns` フィールドを追加し、`discover_files()` でフィルタリング。
Substring-based matching（将来glob対応の余地あり）。

## Test List

- [x] Config伝播: `ExspecConfig` → `Config` で `ignore_patterns` が保持される
- [x] Config伝播(空): デフォルトでは空Vec
- [x] discover_files フィルタ: ignoreパターン指定時にマッチするファイルが除外される
- [x] discover_files 非マッチ: 無関係なファイルは残る（over-exclusion regression）
- [x] discover_files 空ignore: パターンなしなら何も除外しない
- [x] discover_files 空文字パターン: `""` パターンでも全ファイル除外しない
- [x] discover_files 相対パス: パターンはroot相対パスにマッチし、絶対パスのprefixにはマッチしない

## Files Changed

| File | Change |
|------|--------|
| `crates/core/src/rules.rs` | `Config` に `ignore_patterns: Vec<String>` 追加 |
| `crates/core/src/config.rs` | `From<ExspecConfig>` で `paths.ignore` を伝播 + 2テスト |
| `crates/cli/src/main.rs` | `discover_files()` にignore引数追加 + substring filter + 3テスト |

## Discovered

- [ ] Substring matching の過剰除外リスク（例: `fixtures` が `fixtures_helper` にもマッチ）。将来glob/path-segment matchingに移行検討。
- [ ] ignore_patterns Vec の上限なし（custom_assertion_patternsと同じ未解決パターン）

## Progress Log

### 2026-03-10 - RED
- 5テスト作成（config伝播2 + discover_files3）
- コンパイルエラーで失敗確認
- Phase completed

### 2026-03-10 - GREEN
- `Config.ignore_patterns` 追加 + Default実装
- `From<ExspecConfig>` で `ec.paths.ignore` 伝播
- `discover_files()` に `ignore_patterns: &[String]` 引数追加、substring filter実装
- 既存呼び出し全て `&[]` に更新、main()は `&config.ignore_patterns` を渡す
- 全606テスト通過、clippy/fmt clean
- E2E確認: `ignore = ["tests/fixtures"]` で BLOCK 3→0
- Phase completed

### 2026-03-10 - REVIEW
- Security reviewer: PASS (score 8). 空文字パターン・Vec上限なしをoptionalで指摘
- Correctness reviewer: PASS (score 42). 空文字パターン(important)・絶対パスマッチ(important)を指摘
- 両指摘を追加修正: 空文字skipフィルタ + root相対パスでマッチ + 2テスト追加
- 全608テスト通過、clippy/fmt/dogfooding clean
- Phase completed
