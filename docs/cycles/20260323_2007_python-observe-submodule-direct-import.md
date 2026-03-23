---
feature: python-observe-submodule-direct-import
cycle: 20260323_2007
phase: DONE
complexity: standard
test_count: 4
risk_level: low
codex_session_id: ""
created: 2026-03-23 20:07
updated: 2026-03-23 20:07
---

# Issue #119: Python sub-module direct import resolution

## Scope Definition

### In Scope
- [ ] L2 absolute loop で non-barrel resolved file を `direct_import_indices` に記録
- [ ] Assertion filter で `direct_import_indices` を `asserted_matched` に union して bypass

### Out of Scope
- `test_exported_members.py` → `__init__.py` の FN (設計上の制約: production_stem=None)
- L1 マッチング・barrel suppression ロジックの変更

### Files to Change (target: 10 or less)
- `crates/lang-python/src/observe.rs` (edit)

## Environment

### Scope
- Layer: Backend
- Plugin: rust (cargo test)
- Risk: 25 (PASS)

### Runtime
- Language: Rust

### Dependencies (key packages)
- tree-sitter: workspace
- exspec-core: workspace (observe trait, collect_import_matches)

### Risk Interview (BLOCK only)
(not applicable — PASS)

## Context & Dependencies

### Reference Documents
- CONSTITUTION.md - static AST analysis only。assertion filter bypass は static 解析の範囲内
- ROADMAP.md v0.4.2 - #119 は recall improvement target

### Dependent Features
- assertion filter (PY-AF-06a/06b/09): existing fallback logic が正しく継続動作すること (PY-SUBMOD-04 で保護)

### Related Issues/PRs
- Issue #119: Python observe L2 absolute import — assertion filter bypass for direct sub-module imports

## Test List

### TODO
(none)

### WIP
- [x] PY-SUBMOD-01: direct import + barrel coexist, assertion filter bypass
  - Given: `pkg/_urlparse.py`, `pkg/_client.py`, `pkg/__init__.py` (re-exports _client only), `tests/test_whatwg.py` (`from pkg._urlparse import normalize; import pkg; assert pkg.URL(...)`)
  - When: `map_test_files_with_imports`
  - Then: `_urlparse.py` IS mapped (direct import bypasses assertion filter)
- [x] PY-SUBMOD-02: un-re-exported sub-module with direct import
  - Given: `pkg/_internal.py` (NOT in `__init__.py`), `tests/test_internal.py` (`from pkg._internal import helper; assert helper()`)
  - When: `map_test_files_with_imports`
  - Then: `_internal.py` IS mapped
- [x] PY-SUBMOD-03: nested sub-module (`from pkg._internal._helpers import util`)
  - Given: `pkg/_internal/_helpers.py`, `tests/test_helpers.py` (`from pkg._internal._helpers import util; assert util()`)
  - When: `map_test_files_with_imports`
  - Then: `_helpers.py` IS mapped
- [x] PY-SUBMOD-04: regression — barrel-only import still filtered by assertion (existing behavior preserved)
  - Given: `pkg/_config.py`, `pkg/_models.py`, `pkg/__init__.py` (re-exports both), `tests/test_foo.py` (`import pkg; assert pkg.Config()` — no assertion on Model)
  - When: `map_test_files_with_imports`
  - Then: `_models.py` is NOT mapped (barrel import, assertion filter still applies)

### DISCOVERED
- [x] #144: relative direct import should also bypass assertion filter (correctness-reviewer finding)

### DONE
(none)

## Implementation Notes

### Goal
Python observe の recall を改善する。httpx dogfooding (Phase 21) で FN=3、R=96.8% (30/31 test files)。
`from pkg._submodule import X` 形式の direct import が assertion filter で除外されていた FN 2件を修正する。

### Background

httpx dogfooding (Phase 21) で FN(primary)=3。R=96.8% で ship criteria は PASS だが recall 改善の余地あり。

直接的な sub-module import (`from pkg._submodule import Symbol`) が assertion filter に除外されるのが根本原因。
テストが barrel import (`import httpx`) と直接 import を併用する場合、barrel 経由の symbol が assertion に現れると、
直接 import の symbol が assertion filter で落とされる。

#### Root Cause

```
test_whatwg.py:
  import httpx              → barrel → _client.py, _models.py, etc.
  from httpx._urlparse import normalize_url  → direct → _urlparse.py

assertions: assert httpx.URL("...") == ...
  → asserted_imports = {"URL", "httpx", ...}
  → _urlparse idx_to_symbols = {"normalize_url"} → NOT in asserted_imports
  → asserted_matched non-empty (barrel indices pass)
  → _urlparse.py excluded (PY-AF-06b fallback doesn't trigger)
```

#### 3 FN の内訳

| # | Pair | Cause | Fixable |
|---|------|-------|---------|
| 1 | `test_exported_members.py` → `__init__.py` | `__init__.py` は production_stem=None | No (設計上) |
| 2 | `test_whatwg.py` → `_urlparse.py` | assertion filter | Yes |
| 3 | `test_timeouts.py` → `_config.py` (推定) | assertion filter | Yes |

### Design Approach

**直接 sub-module import (non-barrel L2 absolute) は assertion filter を bypass する。**

根拠: `from pkg._submodule import X` は「そのモジュールをテストする意図」の強いシグナル。
barrel 経由の incidental import とは異なり、明示的に特定の production file を指定している。

#### 実装

修正箇所: `crates/lang-python/src/observe.rs`

1. L2 absolute loop (L1010-1045) で、resolved file が barrel でない場合に matched indices を `direct_import_indices: HashSet<usize>` に記録
2. Assertion filter (L1047-1070) で `asserted_matched` に `direct_import_indices` を union

```rust
// L2 absolute loop 内 (collect_import_matches 後):
if !self.is_barrel_file(&resolved) {
    for &idx in all_matched.difference(&before) {
        direct_import_indices.insert(idx);
    }
}

// Assertion filter 内:
asserted_matched.extend(
    direct_import_indices.intersection(&all_matched)
);
```

#### Verification

```bash
cargo test                                    # 全テスト通過
cargo clippy -- -D warnings                   # 0 errors
cargo fmt --check                             # 差分なし
cargo run -- --lang rust .                    # self-dogfooding BLOCK 0
```

オプション: httpx re-dogfood で FN 2→0 (or 1) を確認

## Progress Log

### 2026-03-23 20:07 - INIT
- Cycle doc created from plan file purrfect-painting-origami.md
- Design Review Gate: PASS (score 15/100)
- Scope definition ready

### 2026-03-23 - RED
- PY-SUBMOD-01〜04 テスト追加: `crates/lang-python/src/observe.rs` (末尾 mod tests 内)
- RED state verification:
  - PY-SUBMOD-01: FAILED (assertion filter が direct import を除外するバグを再現)
  - PY-SUBMOD-02: PASSED (assert helper() で helper がマッチ — 既存実装で正常動作)
  - PY-SUBMOD-03: PASSED (assert util() で util がマッチ — 既存実装で正常動作)
  - PY-SUBMOD-04: PASSED (barrel-only regression — 既存実装で正常動作)
- 結論: PY-SUBMOD-01 が FAIL = assertion filter bypass が未実装であることを確認 (RED)

### 2026-03-23 - GREEN
- `direct_import_indices: HashSet<usize>` を L2 absolute loop で tracking
- non-barrel resolved file の新規 matched indices を direct_import_indices に記録
- Assertion filter で asserted_matched に direct_import_indices を union
- 全 1115 テスト PASS、PY-SUBMOD-01 も PASS

### 2026-03-23 - REFACTOR
- チェックリスト 7項目確認: 改善不要
- cargo fmt 適用
- Verification Gate: PASS (tests 1115 pass, clippy 0, fmt OK, self-dogfood BLOCK 0)
- Phase completed

### 2026-03-23 - REVIEW
- Security reviewer: PASS (3/100). No security risk.
- Correctness reviewer: PASS (22/100). 1 important finding: relative import asymmetry (#144 filed).
- Aggregate: PASS (22/100)
- `is_direct` comment improved per optional finding
- Phase completed

### 2026-03-23 - COMMIT
- Branch: feat/python-observe-direct-import-bypass
- Commit: 3857391
- Phase completed

---

## Next Steps

1. [Done] INIT <- Current
2. [Done] PLAN
3. [ ] RED
4. [ ] GREEN
5. [ ] REFACTOR
6. [ ] REVIEW
7. [ ] COMMIT
