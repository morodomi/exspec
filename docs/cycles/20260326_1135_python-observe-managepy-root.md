---
feature: python-observe-managepy-root
cycle: 20260326_1135
phase: RED
complexity: standard
test_count: 3
risk_level: low
codex_session_id: ""
created: 2026-03-26 11:35
updated: 2026-03-26 11:35
---

# Python observe manage.py root detection

## Scope Definition

### In Scope
- [ ] `find_manage_py_root()` 関数の実装 (depth 2 まで manage.py を探索)
- [ ] L2 absolute import 解決に manage.py root フォールバックを追加
- [ ] Django layout (manage.py in subdirectory) の fixture + テスト作成

### Out of Scope
- manage.py が depth 3 以上にある場合 (Reason: Django 標準レイアウトは depth 1-2 の範囲内)
- settings.py 等を用いた DJANGO_SETTINGS_MODULE 解析 (Reason: 過剰。manage.py の場所で十分)

### Files to Change (target: 10 or less)
- `crates/lang-python/src/observe.rs` (edit)
- `tests/fixtures/python/` (new — Django layout fixture)

## Environment

### Scope
- Layer: Backend
- Plugin: python
- Risk: 10 (PASS)

### Runtime
- Language: Rust (exspec 自体は Rust。解析対象は Python)

### Dependencies (key packages)
- tree-sitter: workspace version
- tree-sitter-python: workspace version

### Risk Interview (BLOCK only)
(not applicable — PASS)

## Context & Dependencies

### Reference Documents
- [docs/dogfooding-results.md] — Python observe R=6% の根拠データ
- [crates/lang-python/src/observe.rs] — 変更対象の L2 absolute import 解決ロジック

### Dependent Features
- Python observe L2 import resolution: `crates/lang-python/src/observe.rs`

### Related Issues/PRs
- Internal dogfooding project C (Python/Django): R=6% (1/18) の改善

## Test List

### TODO
- [ ] TC-01: Given Django project with `project/manage.py` and `from app.models import X` in test, When observe runs, Then test maps to `project/app/models.py`
- [ ] TC-02: Given project without manage.py in subdirectory, When observe runs, Then behavior unchanged (existing tests still pass)
- [ ] TC-03: Given manage.py at scan_root itself, When observe runs, Then `find_manage_py_root` returns None (already covered by canonical_root)

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal

Python observe の Django プロジェクトにおける recall 改善。internal dogfooding project C で R=6% (1/18) だった原因を修正し、Django absolute import (`from app.models import X` など) を正しく解決できるようにする。

### Background

Django プロジェクトは `project/` サブディレクトリに配置されることが多く、Python パッケージのルートは `manage.py` の場所になる。現状の observe は `scan_root` を基準に absolute import を解決するため、`scan_root/web/devices/models.py` を探すが、実際は `scan_root/project/web/devices/models.py` に存在する。

### Design Approach

`map_test_files_with_imports()` 内の canonical_root 計算後に `manage.py` を depth 2 まで検索し、見つかった場合は追加の解決ルートとして保持する。L2 absolute import 解決で `canonical_root.join(specifier)` が失敗した場合、`manage_py_root.join(specifier)` をフォールバックで試行する。

```rust
// After canonical_root calculation:
let manage_py_root = find_manage_py_root(&canonical_root);

// In L2 absolute import resolution (fallback chain):
let resolved = resolve_absolute_base_to_file(self, &base, &canonical_root)
    .or_else(|| {
        let src_base = canonical_root.join("src").join(specifier);
        resolve_absolute_base_to_file(self, &src_base, &canonical_root)
    })
    .or_else(|| {
        if let Some(ref mpr) = manage_py_root {
            let django_base = mpr.join(specifier);
            resolve_absolute_base_to_file(self, &django_base, &canonical_root)
        } else {
            None
        }
    });
```

`find_manage_py_root` は scan_root の depth 1-2 のサブディレクトリを走査し、`manage.py` を持つ最初のディレクトリを返す。scan_root 自体 (canonical_root でカバー済み) はスキップする。

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-26 11:35 - INIT
- Cycle doc created
- Scope definition ready

---

## Next Steps

1. [Done] INIT <- Current
2. [Next] RED
3. [ ] GREEN
4. [ ] REFACTOR
5. [ ] REVIEW
6. [ ] COMMIT
