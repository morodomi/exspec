---
feature: "Phase 9b — Python observe (test-to-code mapping)"
cycle: "20260317_1530"
phase: DONE
complexity: standard
test_count: 25
risk_level: low
codex_session_id: ""
created: 2026-03-17 15:30
updated: 2026-03-17 16:00
---

# Cycle: Python observe (test-to-code mapping) (Phase 9b)

## Scope Definition

### In Scope
- [ ] `crates/lang-python/src/observe.rs` 新規作成 (ObserveExtractor trait 実装)
- [ ] `crates/lang-python/queries/production_function.scm` 新規作成 (関数/クラスメソッド定義抽出)
- [ ] `crates/lang-python/queries/import_mapping.scm` 新規作成 (`from X import Y` / `import X` 抽出)
- [ ] `crates/lang-python/queries/exported_symbol.scm` 新規作成 (`__all__` 定義のシンボル抽出)
- [ ] `crates/lang-python/queries/re_export.scm` 新規作成 (`__init__.py` re-export パターン)
- [ ] `tests/fixtures/python/observe/` 配下の observe 用フィクスチャ群
- [ ] `crates/lang-python/src/lib.rs` 編集 (`pub mod observe;` 追加 + PythonExtractor re-export)
- [ ] `crates/cli/src/main.rs` 編集 (`run_observe` に Python ディスパッチ追加)
- [ ] 出力に `[experimental]` マーカー
- [ ] Precision >= 90%, Recall >= 80% の達成確認 (first-pass基準。stable昇格基準 P>=98%/R>=90% は別フェーズで評価)

### Out of Scope
- Rust/PHP observe 実装 (Phase 9c/9d)
- NestJS route decorator 相当の Python フレームワーク固有拡張 (将来対応)
- Python 仮想環境・パッケージ依存解決 (インストール済みパッケージのインポート解決)
- TypeScript tsconfig alias 相当の Python path 設定解決

### Files to Change
- `crates/lang-python/src/observe.rs` (new)
- `crates/lang-python/queries/production_function.scm` (new)
- `crates/lang-python/queries/import_mapping.scm` (new)
- `crates/lang-python/queries/exported_symbol.scm` (new)
- `crates/lang-python/queries/re_export.scm` (new)
- `tests/fixtures/python/observe/` (new — フィクスチャ群)
- `crates/lang-python/src/lib.rs` (edit)
- `crates/cli/src/main.rs` (edit)

## Environment

### Scope
- Layer: `crates/lang-python` + `crates/cli`
- Plugin: dev-crew:rust-quality (cargo test / clippy / fmt)
- Risk: 25/100 (PASS)

### Runtime
- Language: Rust (stable)
- Workspace: 6 crates (core / lang-python / lang-typescript / lang-php / lang-rust / cli)

### Dependencies (key crates)
- tree-sitter + tree-sitter-python: Python AST 解析
- exspec-core: ObserveExtractor trait (Phase 9a で定義済み)
- serde / serde_json: ObserveReport シリアライズ

### Risk Interview (PASS)
- Risk type: 既存パターンの横展開 (TypeScript observe の Python 版)
- trait 境界: Phase 9a で確立済み。実装は trait を impl するだけ
- Python import 体系: TypeScript barrel より単純。`__init__.py` が barrel 相当
- 命名規約: `test_*.py` / `*_test.py` が最も強い言語規約
- 既存テストへの影響: Python lint テスト群は変更なし

## Context & Dependencies

### Reference Documents
- [ROADMAP.md §9b] — Python observe の選定理由・成功基準
- [docs/cycles/20260317_1057_observe-extractor-trait.md] — Phase 9a (ObserveExtractor trait 定義)
- [crates/lang-typescript/src/observe.rs] — 横展開元の TypeScript 実装
- [crates/core/src/observe.rs] — ObserveExtractor trait 定義
- [docs/observe-boundaries.md] — observe スコープ境界

### Dependent Features
- Phase 9c (Rust observe): このサイクルと同一 trait を利用
- Phase 9d (PHP observe): 同上

### Related Issues/PRs
- ROADMAP.md Phase 9b: Python observe (test-to-code mapping)

## Test List

### TODO
- [ ] PY-STEM-01: `test_user.py` に対して test_stem を呼ぶと `Some("user")` を返す
- [ ] PY-STEM-02: `user_test.py` に対して test_stem を呼ぶと `Some("user")` を返す
- [ ] PY-STEM-03: `test_user_service.py` に対して test_stem を呼ぶと `Some("user_service")` を返す
- [ ] PY-STEM-04: `user.py` に対して production_stem を呼ぶと `Some("user")` を返す
- [ ] PY-STEM-05: `__init__.py` に対して production_stem を呼ぶと `None` を返す
- [ ] PY-STEM-06: `test_user.py` に対して production_stem を呼ぶと `None` を返す
- [ ] PY-HELPER-01: `conftest.py` は is_non_sut_helper が true
- [ ] PY-HELPER-02: `constants.py` は is_non_sut_helper が true
- [ ] PY-HELPER-03: `__init__.py` は is_non_sut_helper が true
- [ ] PY-HELPER-04: `tests/utils.py` は is_non_sut_helper が true
- [ ] PY-HELPER-05: `models.py` は is_non_sut_helper が false
- [ ] PY-FUNC-01: `def create_user(): ...` を extract_production_functions すると name="create_user", class_name=None
- [ ] PY-FUNC-02: `class User:` 内の `def save(self):` を extract_production_functions すると name="save", class_name=Some("User")
- [ ] PY-FUNC-03: decorated `def endpoint():` を extract_production_functions すると抽出される
- [ ] PY-IMP-01: `from .models import User` を extract_imports すると specifier=`./models`, symbols=["User"]
- [ ] PY-IMP-02: `from ..utils import helper` を extract_imports すると specifier=`../utils`, symbols=["helper"]
- [ ] PY-IMP-03: `from myapp.models import User` を extract_all_import_specifiers すると ("myapp/models", ["User"])
- [ ] PY-IMP-04: `import os` を extract_all_import_specifiers すると stdlib として解決不可でスキップ
- [ ] PY-IMP-05: `from . import views` を extract_imports すると specifier=`./views`, symbols=["views"]
- [ ] PY-BARREL-01: `__init__.py` は is_barrel_file が true
- [ ] PY-BARREL-02: `__init__.py` に `from .module import Foo` があると extract_barrel_re_exports で symbols=["Foo"], from_specifier="./module"
- [ ] PY-BARREL-03: `__init__.py` に `__all__ = ["Foo"]` があると file_exports_any_symbol(["Foo"]) が true
- [ ] PY-BARREL-04: `__init__.py` に `__all__ = ["Foo"]` があると file_exports_any_symbol(["Bar"]) が false
- [ ] PY-E2E-01: fixture pkg `models.py` + `tests/test_models.py` を map_test_files_with_imports すると Layer 1 でマッチ
- [ ] PY-E2E-02: fixture pkg `views.py` + test が `from ..views import index` を map_test_files_with_imports すると Layer 2 でマッチ
- [ ] PY-E2E-03: `conftest.py` が import されている場合 map_test_files_with_imports で helper として除外
- [ ] PY-CLI-01: `observe --lang python .` を実行すると正常に動作する

### WIP
(none)

### DISCOVERED
- [ ] PY-BOUNDARY-01: 絶対importが解決不可な場合（サードパーティパッケージ）はスキップする → #90

### DONE
(none)

## Implementation Notes

### Goal
`ObserveExtractor` trait (Phase 9a) を Python 向けに実装し、`test_*.py` / `*_test.py` の命名規約と `from X import Y` 体系を活用した test-to-code mapping を実現する。TypeScript observe と同等の Layer 1 (stem matching) + Layer 2 (import tracing) で動作し、出力に `[experimental]` マーカーを付与する。

### Background
Python は命名規約が最も強い言語（`test_*.py`）であり、import 体系も TypeScript barrel より単純（barrel = `__init__.py`）。Phase 9a で `ObserveExtractor` trait が core に確立されたため、Python は trait を implement するだけで CLI に統合できる。

ROADMAP §9b より:
- 成功基準: Precision >= 90%, Recall >= 80%
- TypeScript observe の横展開が基本戦略
- Python 固有: `__all__` による明示的 export 定義、相対 import (`from .module import X`)

### Design Approach

**実装構造**:

```
PythonExtractor (crates/lang-python/src/observe.rs)
  impl ObserveExtractor for PythonExtractor
    // stem matching
    fn test_stem(path) -> Option<&str>
      - "test_*.py" → stem = name.strip_prefix("test_")
      - "*_test.py" → stem = name.strip_suffix("_test")
    fn production_stem(path) -> Option<&str>
      - 通常の .py ファイル (test_ prefix なし、__init__.py 除外)
    // helper filtering
    fn is_non_sut_helper(file_path, is_known_production) -> bool
      - conftest.py, constants.py, __init__.py, tests/utils.py 等
    // barrel
    fn index_file_names() -> &[&str]  // ["__init__.py"]
    fn source_extensions() -> &[&str] // [".py"]
    fn file_exports_any_symbol(path, symbols) -> bool
      - __all__ が定義されていればシンボル照合、なければ true
    // extraction (tree-sitter queries)
    fn extract_production_functions(source, file_path) -> Vec<ProductionFunction>
    fn extract_imports(source, file_path) -> Vec<ImportMapping>
    fn extract_all_import_specifiers(source) -> Vec<(String, Vec<String>)>
    fn extract_barrel_re_exports(source, file_path) -> Vec<BarrelReExport>
```

**tree-sitter queries**:
- `production_function.scm`: `function_definition` + `class_definition > block > function_definition` を捕捉
- `import_mapping.scm`: `import_from_statement` を捕捉し、相対 import の `.`/`..` を `./`/`../` に変換
- `exported_symbol.scm`: `__all__ = [...]` の文字列リテラルを捕捉
- `re_export.scm`: `__init__.py` 内の `from .X import Y` パターンを捕捉

**`map_test_files_with_imports` の帰属**:
TypeScript と同じパターンで PythonExtractor の具象メソッドとして実装する。Trait には追加しない（言語ごとに import 解決ロジックが異なるため）。CLI 側は `match args.lang` で分岐し、各 Extractor の `map_test_files_with_imports` を直接呼ぶ。

**`is_non_sut_helper` 判定ロジック**:
ファイル名パターン: `conftest.py`, `constants.py`, `setup.py`, `__init__.py`。
パス成分: `tests/` or `test/` or `__pycache__/` 配下の非 `test_*` ファイル。
つまり `tests/utils.py` は helper (test_ prefix なし) だが `tests/test_utils.py` は test file (helper ではない)。

**Implementation Order**:
1. スケルトン + stem/helper (PY-STEM-*, PY-HELPER-*)
2. production_function.scm + extract (PY-FUNC-*)
3. import_mapping.scm + extract_imports (PY-IMP-01, 02, 05)
4. extract_all_import_specifiers (PY-IMP-03, 04)
5. barrel: re_export.scm + exported_symbol.scm (PY-BARREL-*)
6. map_test_files_with_imports (PY-E2E-*)
7. CLI dispatch (PY-CLI-01)

## Progress Log

### 2026-03-17 15:30 - INIT
- Cycle doc 作成 (sync-plan)
- Plan: Phase 9b Python observe (test-to-code mapping)
- Scope: lang-python/observe.rs (new) + queries/*.scm (new) + lib.rs + cli/main.rs

### 2026-03-17 16:00 - RED
- `crates/lang-python/src/observe.rs` 作成 (スケルトン + 26テスト)
- `crates/lang-python/queries/production_function.scm` 作成
- `crates/lang-python/queries/import_mapping.scm` 作成
- `crates/lang-python/queries/exported_symbol.scm` 作成
- `crates/lang-python/queries/re_export.scm` 作成
- `crates/lang-python/src/lib.rs` 編集 (`pub mod observe;` 追加)
- `tests/fixtures/python/observe/` フィクスチャ群作成
- テスト結果: FAILED 10件 / PASS 16件 (RED状態確認)
  - PASS (スケルトンで正しく動作): stem/helper/barrel_01/e2e_01/e2e_03/imp_04 等
  - FAILED (実装待ち): FUNC-01〜03, IMP-01〜03/05, BARREL-02/04, E2E-02
- self-dogfooding: BLOCK 0件 確認

### 2026-03-17 17:00 - GREEN
- `extract_production_functions`: tree-sitter query で4パターン対応 (top-level, class method, decorated, decorated method)
- `extract_imports`: 相対 import (`from .X`, `from ..X`, `from . import X`) を `./`, `../` 形式に変換
- `extract_all_import_specifiers`: 絶対 import (`from myapp.models`) を `myapp/models` に変換
- `extract_barrel_re_exports`: `__init__.py` の re-export パターン抽出
- `file_exports_any_symbol`: `__all__` 定義によるシンボル照合
- `map_test_files_with_imports`: Layer 1 (stem) + Layer 2 (import tracing) 統合
- CLI: `run_observe` に Python ディスパッチ追加、共通 `build_observe_report` 抽出
- テスト結果: 26/26 PASS (865 tests total)

### 2026-03-17 17:30 - REFACTOR
- `is_non_sut_helper`: 未使用の `in_test_dir` 変数を削除
- チェックリスト全項目確認 (重複/定数/未使用import/メソッド分割/N+1/命名)
- Verification Gate: tests PASS + clippy 0 + fmt OK + self-dogfooding BLOCK 0
- Phase completed

### 2026-03-17 18:00 - REVIEW
- Security review: PASS (score: 12) - optional issues only
- Correctness review: WARN (score: 62) - 3 important bugs detected and fixed:
  1. `__all__ = []` 誤判定: `exported_symbol.scm` に empty list/tuple パターン追加
  2. `from .. import X` Layer 2 未解決: bare relative import の per-symbol 解決を一般化
  3. Strategy 更新の論理バグ: `layer1_matched` を per-production-file 追跡に変更
- CLI `--lang` ドキュメント更新 ("typescript only" → "typescript, python")
- 修正後 re-verification: 全テスト PASS, clippy 0, fmt OK, BLOCK 0
- Phase completed

---

## Next Steps

1. [In Progress] INIT
2. [ ] plan-review
3. [ ] RED
4. [ ] GREEN
5. [ ] REFACTOR
6. [ ] REVIEW
7. [ ] COMMIT
