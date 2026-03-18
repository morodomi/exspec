---
feature: "Phase 12 — Python observe dogfooding"
cycle: "20260318_2351"
phase: DONE
complexity: standard
test_count: 3
risk_level: medium
codex_session_id: ""
created: 2026-03-18 23:51
updated: 2026-03-19
---

# Cycle: Phase 12 — Python observe dogfooding

## Scope Definition

### In Scope
- [ ] GT スクリプト作成: `scripts/generate_python_gt.py`
- [ ] GT スクリプトのテスト: `scripts/test_generate_python_gt.py` (3 tests)
- [ ] httpx dogfooding (commit b5addb64) — P/R/F1 測定
- [ ] Requests spot-check — psf/requests
- [ ] 非公開 Web プロジェクト実用検証
- [ ] 結果分析 + `docs/dogfooding-results.md` 更新
- [ ] `docs/STATUS.md` 更新 (Python observe メトリクス)
- [ ] observe 修正 (dogfood 結果次第 — 別 cycle で TDD 実施)

### Out of Scope
- Python observe の実装変更 (`crates/lang-python/src/observe.rs`) — dogfood 結果次第で別 cycle
- TypeScript/Rust/PHP observe への変更
- 新規ルール追加

### Files to Change
- `scripts/generate_python_gt.py` (新規作成)
- `scripts/test_generate_python_gt.py` (新規作成)
- `docs/dogfooding-results.md` (更新)
- `docs/STATUS.md` (更新)

## Environment

### Scope
- Layer: `scripts/`, `docs/`, `crates/lang-python` (修正が必要な場合)
- Plugin: dev-crew:python-quality (pytest / mypy / Black)
- Risk: 30/100 (WARN) — Python observe 未検証、src layout/barrel 解決の問題発見の可能性
- Runtime: Rust (cargo test) + Python 3 (GT スクリプト)
- Dependencies: tree-sitter (既存、追加依存なし)

### Risk Interview

(WARN — リスク 30/100)
- src layout (`src/` 配下) での import 解決が未検証
- barrel (`__init__.py`) チェーン解決の精度が不明
- 修正が必要な場合は別 cycle で TDD 実施する方針

## Context & Dependencies

### Background

Phase 11 (TS observe re-dogfood) 完了後、ROADMAP "Next" として Python observe dogfooding に着手。

Python observe は Phase 9b で実装済み:
- L1: filename convention マッチ
- L2: relative + absolute import トレース
- barrel: `__init__.py` 解決
- FastAPI / Django route extraction

ただし実プロジェクトでの dogfooding が未実施。精度メトリクス (P/R/F1) が不明。

**評価基準 (ROADMAP Phase 9 / CONSTITUTION Section 8):**
- first-pass (experimental): P >= 90%, R >= 80% — `[experimental]` ラベルで出荷可
- stable: P >= 98%, R >= 90% — experimental 解除の基準
- 本 Phase は first-pass 基準で評価する

### Design Approach

**GT スクリプト設計:**

1. テストファイルを引数で受け取り、Python `ast` モジュールで import 文を解析 (exspec の tree-sitter とは独立 → tautological evaluation 回避)
2. absolute import (`from httpx._decoders import ...`) → `httpx/_decoders.py` を candidate に追加
3. filename convention (`tests/test_utils.py` → `httpx/_utils.py`) → `filename_match` evidence を記録
4. barrel import chain (`import httpx` → `__init__.py` → 実ファイル) は手動アノテーション (GT スクリプトで再実装すると exspec と同等になり循環評価リスク)
5. 結果を JSON で出力 — evaluate_observe.py が期待するスキーマに準拠

**GT JSON スキーマ (evaluate_observe.py 準拠):**

```json
{
  "file_mappings": {
    "tests/test_foo.py": {
      "primary_targets": ["httpx/_foo.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_foo.py": ["direct_import", "filename_match"]
      }
    }
  }
}
```

**Dogfood フロー:**

1. httpx を commit b5addb64 で checkout
2. `exspec observe --lang python --format json .` を実行
3. JSON 出力と GT を差分比較
4. P/R/F1 を算出 (first-pass 基準: P >= 90%, R >= 80%)
5. 修正が必要な FP/FN を分析 → 別 cycle かどうか判断

### Reference Documents
- `crates/lang-python/src/observe.rs` — Python observe 実装
- `docs/dogfooding-results.md` — dogfooding 結果 (更新対象)
- `docs/STATUS.md` — 現在のメトリクス (更新対象)
- ROADMAP.md (Phase 12: Python observe dogfooding)
- `docs/cycles/20260317_1530_python_observe.md` — Python observe 実装サイクル

### Related Issues/PRs
- Phase 9b: Python observe 実装 (完了済み、参照元)
- Phase 11: TS observe re-dogfood (完了済み、参照元)

## Test List

### TODO
- [ ] GT-01: absolute import 解決
      Given: test file with `from httpx._decoders import ...`
      When: generate_python_gt.py で解析
      Then: httpx/_decoders.py が primary candidate に含まれる
- [ ] GT-02: barrel import は手動アノテーション対象として記録
      Given: test file with `import httpx` + `httpx.Client(...)`
      When: generate_python_gt.py で解析
      Then: `import httpx` が barrel_import evidence として記録され、手動アノテーション候補に含まれる
- [ ] GT-03: filename convention match
      Given: tests/test_utils.py + httpx/_utils.py
      When: generate_python_gt.py で解析
      Then: filename_match evidence が記録される

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal

Python observe を httpx・Requests・非公開 Web プロジェクトの 3 プロジェクトで検証し、P/R/F1 を測定する。first-pass 基準 (P >= 90%, R >= 80%) を確認し、必要なら修正 issue を作成する。

### Background

Phase 9b で Python observe を実装したが、実プロジェクトでの検証が未実施。TypeScript observe が Phase 11 で P=100%, R=91.0% を達成したのと同様に、Python observe も実プロジェクトで精度を検証する必要がある。

GT スクリプトを TDD で作成し、observe の出力と比較することで精度メトリクスを算出する。

### Design Approach

**Verification コマンド:**

```bash
# GT script
python3 -m pytest scripts/test_generate_python_gt.py

# exspec quality gate
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .

# Dogfood results
# httpx: P >= 90%, R >= 80% (first-pass criteria)
# Requests: spot-check で重大な FP/FN なし
```

**Design Steps:**

1. httpx dogfooding (GT 作成) — encode/httpx commit b5addb64
2. Requests spot-check — psf/requests
3. 非公開 Web プロジェクト実用検証
4. 結果分析 + 修正 (条件付き、別 cycle)
5. Docs 更新

## Progress Log

### 2026-03-18 23:51 - INIT
- Cycle doc created

### 2026-03-18 23:51 - SYNC-PLAN
- Cycle doc generated from plan (Phase 12: Python observe dogfooding)

### 2026-03-18 23:52 - PLAN-REVIEW
- Design review: WARN (blocking_score: 62/100)
- Critical 1: ship criteria 二重定義 → first-pass/stable 基準を Cycle doc に明記して解決
- Critical 2: GT スキーマ未定義 → evaluate_observe.py 準拠の JSON スキーマを Design Approach に追記
- Important: GT-02 barrel chain の tautological evaluation リスク → barrel は手動アノテーションに変更
- Phase completed

### 2026-03-19 - REFACTOR
- `import os` 削除 (未使用 import)
- マジックストリングを定数化: `EVIDENCE_DIRECT_IMPORT`, `EVIDENCE_BARREL_IMPORT`, `EVIDENCE_FILENAME_MATCH`, `TEST_DIR_NAMES`, `TEST_FILE_PREFIX`
- evidence 追加パターンを `_add_evidence()` ヘルパーに共通化 (DRY)
- `analyze_test_file` を 4 関数に分割: `_resolve_import_nodes`, `_resolve_from_import`, `_resolve_plain_import`, `_apply_filename_convention`
- 全 26 tests PASS (3 GT + 23 evaluate_observe)
- Rust quality gate: `cargo test` OK / `cargo clippy` OK / `cargo fmt --check` OK / self-dogfooding BLOCK 0
- Phase completed

### 2026-03-19 - REVIEW
- correctness-reviewer: WARN (62/100)
- Accept: generate_ground_truth に needs_manual_review 追加, 相対 import で needs_manual_review=True, import foo.bar の優先順序修正, _find_production_files キャッシュ, SyntaxError 時 needs_manual_review=True, パストラバーサルガード
- Reject: Windows パスセパレータ (macOS/Linux 専用スクリプト)
- Python 3.9 互換性修正 (list[Path] | None → from __future__ import annotations)
- 全 26 tests PASS
- Phase completed

### 2026-03-19 - DOGFOOD
- httpx (encode/httpx @ b5addb64): P=66.7%, R=6.2%, F1=11.4% — FAIL
- Requests (psf/requests): ~0% recall — FAIL
- Root causes: L1 `_` prefix, L2 barrel, `src/` layout, cross-directory
- Ground truth: docs/observe-ground-truth-python-httpx.md (30 test files, 100% audit)
- Improvement plan: P0 (`_` prefix, `src/` layout), P1 (barrel, cross-dir)
- Phase completed

### 2026-03-19 - DOCS
- docs/dogfooding-results.md: Python observe セクション追加
- docs/STATUS.md: Phase 12 結果追加
- ROADMAP.md: Phase 12 完了記録 + Next 更新
- Phase completed
