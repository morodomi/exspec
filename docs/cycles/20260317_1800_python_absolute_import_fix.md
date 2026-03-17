---
feature: "Fix #91: Python absolute imports fail to resolve from project root"
phase: DONE
complexity: S
test_count: 5
risk_level: low
created: 2026-03-17
updated: 2026-03-17
---

# Fix #91: Python absolute imports fail to resolve from project root

## Context

Python observe (Layer 2) で `from models.cars import Car` のような絶対 import が解決されない。Flask/FastAPI の flat layout プロジェクトで observe が 0% match になる。

Dogfooding で発見: 43 prod / 14 test → 0 mapped。

## Root Cause

`crates/lang-python/src/observe.rs` の `map_test_files_with_imports` (L534) が `extract_imports` のみを使用しているが、`extract_imports` (L229) は相対 import (`./`, `../`) のみをフィルタする。絶対 import は `extract_all_import_specifiers` に分離されているが、Layer 2 の解決ループで使われていない。

つまり:
1. `extract_imports` → 相対 import のみ → Layer 2 に渡される
2. `extract_all_import_specifiers` → 絶対 import → **Layer 2 で未使用**

## Design Decision

**Python 側のみ修正 (core 変更なし)**

`map_test_files_with_imports` で `extract_all_import_specifiers` の結果も処理する。絶対 import は `scan_root` ベースで `resolve_absolute_base_to_file` を直接呼ぶ。

core の `resolve_import_path` は変更しない → TS/Rust への regression リスクゼロ。

Design review で指摘: 元プランの core 変更は TS barrel 解決 (`resolve_barrel_exports_inner`) への影響が未検討だった。Python 側のみの修正で影響範囲を限定する。

## 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `crates/lang-python/src/observe.rs` | `map_test_files_with_imports` に `extract_all_import_specifiers` による絶対 import 解決ループ追加 |

## Test List

| ID | Status | Given | When | Then |
|----|--------|-------|------|------|
| PY-ABS-01 | PASS | `from models.cars import Car` in `tests/unit/test_car.py`, `models/cars.py` exists at scan_root | map_test_files_with_imports | test mapped to `models/cars.py` via Layer 2 |
| PY-ABS-02 | PASS | `from utils.publish_state import ...` in `tests/test_pub.py`, `utils/publish_state.py` exists at scan_root | map_test_files_with_imports | test mapped to `utils/publish_state.py` via Layer 2 |
| PY-ABS-03 | PASS | relative import `from .models import X` in test file, `models.py` exists relative to test | map_test_files_with_imports | resolves from from_file parent (既存挙動維持) |
| PY-ABS-04 | PASS | `from nonexistent.module import X` in test file | map_test_files_with_imports | no mapping added (graceful skip) |
| PY-ABS-05 | PASS | mixed: absolute import + relative import in same test file | map_test_files_with_imports | both resolved correctly |

## Implementation Order

1. PY-ABS-01〜05 テスト作成 (RED)
2. `map_test_files_with_imports` に絶対 import 解決ループ追加 (GREEN)
3. 既存テスト全通過確認 (regression)

## Progress Log

### 2026-03-17 - REVIEW (plan)
- Design review: WARN (72) — root cause 分析修正、core 変更取りやめ、Python 側のみ修正に変更
- Security review: PASS (12) — optional な改善提案のみ
- Plan 修正完了、再レビュー不要 (修正方向は影響範囲を縮小)

### 2026-03-17 - RED
- 5件のテストを `crates/lang-python/src/observe.rs` に追加
- `crates/lang-python/Cargo.toml` に `[dev-dependencies] tempfile = "3"` 追加
- 結果: 137 passed / 3 failed (PY-ABS-01, 02, 05) / 0 regression
  - PY-ABS-01: FAIL (絶対 import 未解決)
  - PY-ABS-02: FAIL (絶対 import 未解決)
  - PY-ABS-03: PASS (相対 import 既存挙動維持)
  - PY-ABS-04: PASS (存在しない module は graceful skip)
  - PY-ABS-05: FAIL (mixed: 絶対 import 側が未解決)
- Self-dogfooding: BLOCK 0件確認

### 2026-03-17 - GREEN
- `map_test_files_with_imports` に絶対 import 解決ループ追加 (L594-612 in observe.rs)
  - `extract_all_import_specifiers` の結果を `canonical_root.join(specifier)` でベースパスに変換
  - `resolve_absolute_base_to_file` を直接呼び出し、`collect_import_matches` でマッピング収集
- 結果: 140 passed / 0 failed (PY-ABS-01, 02, 05 が PASS に転換)
- clippy: 0 errors, fmt: 差分なし, self-dogfooding: BLOCK 0件

### 2026-03-17 - REFACTOR
- チェックリスト7項目確認: リファクタリング不要 (追加18行、重複なし、命名整合)
- Verification Gate: 574 passed / 0 failed, clippy 0, fmt OK, BLOCK 0
- Phase completed

### 2026-03-17 - REVIEW (code)
- Security: PASS (5) — 境界チェック正常、DoS リスクなし
- Correctness: WARN (62) — issue 1 は FP (extract_imports は L229 で絶対 import を除外済み)、issue 2 はスコープ外
- Adjusted total: PASS (~20)
- Phase completed

## DISCOVERED
- multi-level package + 同名モジュール共存時の resolve 優先順テスト (correctness reviewer 指摘、既存挙動、スコープ外)
