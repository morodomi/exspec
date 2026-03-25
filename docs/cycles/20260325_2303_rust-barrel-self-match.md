---
feature: rust-barrel-self-match
cycle: 20260325_2303
phase: GREEN
complexity: standard
test_count: 6
risk_level: medium
codex_session_id: ""
created: 2026-03-25 23:03
updated: 2026-03-25 23:30
---

# Rust observe: mod.rs barrel self-match fix

## Scope Definition

### In Scope
- [ ] `collect_import_matches` (core/src/observe.rs:327-342) を修正し、barrel file 自体も production candidate に含める
- [ ] `file_exports_any_symbol` チェックを barrel 分岐後に追加
- [ ] fixture: barrel file に直接シンボルを定義するケース (mod.rs + `pub struct`)
- [ ] fixture: barrel file にシンボルを直接定義しないケース (子ファイルのみ)
- [ ] regression guard: TS index.ts / Python `__init__.py` の挙動が変わらないこと

### Out of Scope
- tokio/clap の crate root barrel FN (別経路、本サイクルでは対象外)
- cross-crate FN (別サイクル)
- TS/Python/PHP のバレル解決アルゴリズム変更

### Files to Change (target: 10 or less)
- `crates/core/src/observe.rs` (edit) — `collect_import_matches` 修正
- `crates/lang-rust/src/observe.rs` (edit) — `file_exports_any_symbol` 実装確認・修正
- `tests/fixtures/rust/` (new/edit) — barrel self-match テスト fixture 追加

## Environment

### Scope
- Layer: Core (言語非依存 observe エンジン) + lang-rust
- Plugin: rust
- Risk: 30 (PASS)

### Runtime
- Language: Rust (edition 2021)

### Dependencies (key packages)
- tree-sitter: workspace
- tree-sitter-rust: workspace

### Risk Interview (BLOCK only)
N/A — Risk 30 (PASS)

## Context & Dependencies

### Reference Documents
- [ROADMAP.md] — P2 "Rust mod.rs barrel type resolution"
- [CONSTITUTION.md] — Ship criteria: P>=98%, R>=90%
- [docs/observe-ground-truth-rust-tower.md] — 5 FN = mod.rs barrel パターン

### Dependent Features
- L1 cross-directory subdir stem matching (#189): 既存の Rust observe ベースに重ねる修正

### Related Issues/PRs
- (なし)

## Test List

### TODO
- [x] TC-01: Given barrel file (mod.rs) with `pub struct Foo`, When test imports `use crate::module::Foo`, Then mod.rs is matched as production target
- [x] TC-02: Given barrel file (mod.rs) without direct symbol definition (only re-exports child), When test imports symbol from child, Then only child is matched (mod.rs は candidate に含まれない)
- [x] TC-03: Given TS index.ts barrel, When test imports from barrel, Then behavior unchanged (regression guard)
- [x] TC-04: Given Python `__init__.py` barrel, When test imports, Then behavior unchanged (regression guard)
- [x] TC-05: Given tower observe after fix, When measure recall, Then R > 78.3% (improvement confirmed)
- [x] TC-06: Given tokio observe after fix, When measure recall, Then R >= 50.8% (no regression)

### WIP
- [x] TC-01, TC-02, TC-03, TC-04: core/observe.rs に追加済み
- [x] TC-05, TC-06: #[ignore] integration test として追加済み

### DISCOVERED
(none)

### DONE
- [x] TC-01: FAIL (RED verified) — barrel file (mod.rs) は現在 candidate から除外されるため FAIL
- [x] TC-02: PASS (regression guard) — symbol なし barrel は正しく空のまま
- [x] TC-03: FAIL (RED verified) — TS index.ts barrel も同様に FAIL
- [x] TC-04: FAIL (RED verified) — Python __init__.py barrel も同様に FAIL
- [x] TC-05: #[ignore] 追加済み
- [x] TC-06: #[ignore] 追加済み

## Implementation Notes

### Goal

`collect_import_matches` の barrel 分岐で mod.rs 自体が production candidate から除外されているバグを修正し、tower の Rust observe recall を 78.3% → 95%+ に改善する。

### Background

tower の 5 FN はすべて mod.rs barrel パターンが原因。`collect_import_matches` (core/src/observe.rs:327-342) の barrel 分岐は子ファイルのみを results に追加し、mod.rs 自体を candidate から除外している。そのため mod.rs に直接定義された `pub struct AsyncFilter` 等がマッピングされない。

### Design Approach

barrel resolution 後に、barrel file 自体も `file_exports_any_symbol` チェックして candidate に含める:

```rust
if ext.is_barrel_file(resolved) {
    // 既存: 子ファイルを resolve
    let resolved_files = resolve_barrel_exports(...);
    for prod in resolved_files { ... }

    // NEW: barrel 自体も candidate に含める (シンボルが直接定義されている場合)
    if ext.file_exports_any_symbol(Path::new(resolved), symbols) {
        if !ext.is_non_sut_helper(resolved, canonical_to_idx.contains_key(resolved)) {
            if let Some(&idx) = canonical_to_idx.get(resolved) {
                indices.insert(idx);
            }
        }
    }
}
```

**Impact 想定:**
- tower: 3-5 FN 解消 (filter/mod.rs, hedge/mod.rs, steer/mod.rs, possibly limit/, util/)
- tokio/clap: 影響なし (別経路の FN)
- TS/Python/PHP: index.ts, `__init__.py` は通常シンボルを直接定義しないので影響なし。定義していても正しくマッチするのは意図した動作

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tower
cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tokio
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-25 23:03 - INIT
- Cycle doc created
- Scope definition ready

---
### 2026-03-25 23:15 - RED phase complete

- テスト追加: `crates/core/src/observe.rs` の `#[cfg(test)] mod tests` に TC-01〜TC-06 追加
- RED 確認: TC-01/TC-03/TC-04 FAIL, TC-02 PASS, TC-05/TC-06 #[ignore]
- 既存テスト 245 PASS (regression なし)
- 失敗テスト: observe::tests::tc01_barrel_self_match_when_exports_symbol_directly
              observe::tests::tc03_ts_index_barrel_self_match_regression_guard
              observe::tests::tc04_python_init_barrel_self_match_regression_guard
- `BarrelSelfMatchMock` 構造体を新設 (exports_symbol: bool フィールドで制御)
- `PythonBarrelMock` を TC-04 内に inline で定義 (__init__.py barrel)

---
### 2026-03-25 23:30 - GREEN phase complete

- 修正: `crates/core/src/observe.rs` の `collect_import_matches` に barrel self-match チェックを追加
  - barrel 分岐の `for` ループ後に `file_exports_any_symbol` チェックを追加
  - barrel ファイル自体がシンボルを直接定義している場合、production candidate として追加
- 既存テスト `core_cim_01` の期待値を新仕様に合わせて更新
  - 旧: `file_exports_any_symbol=true` でも `indices` が空
  - 新: `file_exports_any_symbol=true` なら barrel 自体が追加される
- TC-01/TC-03/TC-04 PASS、TC-02 PASS（regression guard 維持）
- 全テスト PASS（FAILED 0件）、clippy 0 errors、fmt clean

---


## Next Steps

1. [Done] INIT <- Current
2. [Done] PLAN
3. [Done] RED
4. [Done] GREEN
5. [ ] REFACTOR
6. [ ] REVIEW
7. [ ] COMMIT
