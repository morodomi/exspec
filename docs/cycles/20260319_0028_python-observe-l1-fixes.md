---
feature: "Phase 13 — Python observe L1 fixes"
cycle: "20260319_0028"
phase: REFACTOR
complexity: standard
test_count: 5
risk_level: medium
codex_session_id: ""
created: 2026-03-19 00:28
updated: 2026-03-19 01:00
---

# Cycle: Phase 13 — Python observe L1 fixes

## Scope Definition

### In Scope
- [ ] Fix 1: `_` prefix stripping in `production_stem()` (3 tests)
- [ ] Fix 3: `src/` layout detection for L2 absolute import (2 tests)
- [ ] Re-dogfood httpx + Requests to verify improvement

### Out of Scope
- Fix 2: Cross-directory L1 matching (design review で P1 として次サイクルに延期)
- TypeScript/Rust/PHP observe への変更
- 新規ルール追加
- L2 import tracing ロジックの全面改修
- core crate の変更

### Files to Change
- `crates/lang-python/src/observe.rs` (修正)
- `crates/lang-python/tests/` (テスト追加)

## Environment

### Scope
- Layer: `crates/lang-python/src/observe.rs`
- Plugin: dev-crew:python-quality は不使用 (Rust crate)
- Risk: 40/100 (WARN) — L1 ロジック変更、FP リスク評価が必要
- Runtime: Rust (cargo test)
- Dependencies: tree-sitter (既存、追加依存なし)

### Risk Interview

(WARN — リスク 40/100)
- `_` prefix strip による FP: 現在マッチしていないケースへの新規マッチなので FP 増加リスクは低い
- `src/` layout fallback: 既存の non-src layout との共存を regression テストで確認が必要

## Context & Dependencies

### Background

Phase 12 dogfooding で Python observe が httpx P=66.7%/R=6.2%, Requests ~0% recall と判明。
L1 filename convention matching が Python の一般的なプロジェクト構造で機能していない。

根本原因は3つ:
1. `_` prefix の未処理 (`httpx/_decoders.py` vs `tests/test_decoders.py` でマッチしない)
2. 同一ディレクトリ制約 (core `map_test_files` が `(directory, stem)` でマッチ)
3. `src/` layout 未検出 (`src/requests/*.py` への absolute import 解決不能)

### Design Approach

**Fix 1: `_` prefix stripping in `production_stem()`**

`production_stem()` (line 60-72) にて `strip_prefix('_')` で先頭の `_` を1つだけ除去。
`__init__.py` は先に除外済みなので問題なし。
`__version__.py` → `Some("_version")` (1つだけ strip)。

**Fix 3: `src/` layout detection (L2 absolute import fallback)**

L2 absolute import 解決で `canonical_root.join("src")` もフォールバック先として試す。
既存の `canonical_root` 直下解決をメインとし、`src/` は第2候補。

## Test List

### TODO
(none)

### WIP
- [x] PY-STEM-07: `_decoders.py` -> production_stem strips single leading underscore — FAIL (RED)
- [x] PY-STEM-08: `__version__.py` -> production_stem strips only one underscore (returns `Some("_version")`) — FAIL (RED)
- [x] PY-STEM-09: `decoders.py` -> production_stem unchanged (regression) — PASS (correct: no impl change needed)
- [x] PY-SRCLAYOUT-01: `src/` layout absolute import resolved — FAIL (RED)
- [x] PY-SRCLAYOUT-02: non-`src/` layout still works (regression) — PASS (correct: no impl change needed)

### DISCOVERED
- [x] PY-STEM-10: `___triple.py` の動作を確定するテスト (correctness review 指摘) — 955376c
- [x] PY-SRCLAYOUT-01/02 に `strategy` assertion 追加 (test review 指摘) — 955376c
- [x] integration テスト boilerplate のヘルパー抽出 (test review 指摘) — d601fe5

### DONE
All DISCOVERED items resolved.

## Progress Log

### 2026-03-19 — RED Phase 完了

5テスト追加。結果:
- FAIL (RED): PY-STEM-07, PY-STEM-08, PY-SRCLAYOUT-01 (3件 — 実装待ち)
- PASS: PY-STEM-09, PY-SRCLAYOUT-02 (2件 — regression テストとして正常)
- 既存 174 テストは全て PASS のまま

### 2026-03-19 — REFACTOR Phase 完了

- `production_stem()` の doc comment に `_` prefix stripping の例を追加 (`_decoders.py` -> `Some("decoders")`)
- 変更箇所チェックリスト確認: 重複なし、定数化不要、未使用importなし、命名一貫性OK
- cargo test: 全 PASS (121+237+10+135+177+124+225 tests)
- cargo clippy -- -D warnings: 0 errors
- cargo fmt --check: 差分なし
- Phase completed

### 2026-03-19 00:28 — Cycle doc 作成 (sync-plan)

planファイルから Cycle doc を生成。
Phase 12 dogfooding で判明した Python observe L1 の3件の根本原因修正サイクルを開始。
test_count: 5 (Fix1: 3, Fix3: 2). Fix 2 は design review で P1 として次サイクルに延期
