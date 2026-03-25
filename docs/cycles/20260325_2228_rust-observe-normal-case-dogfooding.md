---
feature: rust-observe-normal-case-dogfooding
cycle: 20260325_2228
phase: GREEN
complexity: standard
test_count: 4
risk_level: low
codex_session_id: ""
created: 2026-03-25 22:28
updated: 2026-03-25 22:28
---

# Rust observe: normal-case library dogfooding

## Scope Definition

### In Scope
- [ ] tower (commit 251296d) を normal-case GT library として採用
- [ ] tower の GT doc 作成 (`docs/observe-ground-truth-rust-tower.md`)
- [ ] `cargo run -- observe --lang rust` で P>=98% / R>=90% を計測
- [ ] dogfooding-results.md / STATUS.md / ROADMAP.md 更新

### Out of Scope
- 他の候補 (serde_json, regex, nom, aho-corasick) の詳細測定 (探索済み。tower が唯一 R>=90% を達成)
- Rust observe の実装変更 (測定タスク。変更は結果次第)

### Files to Change (target: 10 or less)
- `docs/observe-ground-truth-rust-tower.md` (new)
- `docs/dogfooding-results.md` (edit)
- `docs/STATUS.md` (edit)
- `ROADMAP.md` (edit)

## Environment

### Scope
- Layer: Documentation / Measurement
- Plugin: dev-crew:rust-quality
- Risk: 15 (PASS)

### Runtime
- Language: Rust (cargo)
- Toolchain: stable

### Dependencies (key packages)
- tree-sitter: workspace
- exspec (self): local build

### Risk Interview (BLOCK only)
- N/A (Risk: 15, PASS)

## Context & Dependencies

### Reference Documents
- `docs/observe-ground-truth-rust-clap.md` — GT format reference (clap)
- `docs/observe-ground-truth-rust-tokio.md` — GT format reference (tokio)
- `docs/dogfooding-results.md` — 全言語 P/R 記録
- `ROADMAP.md` — ship criteria 定義

### Dependent Features
- Rust observe implementation: `crates/lang-rust/src/observe.rs`
- fan-out filter: PR #194 (merged, commit 6935075)

### Related Issues/PRs
- PR #194: directory-aware fan-out filter (merged)
- Memory: rust_observe_recall_journey.md — R=36.8%→71.0% 改善の経緯

## Design Review Gate

### Assessment

- **Selected library**: tower (commit 251296d)
- **Measurement result**: R=94.7% (18/19 test files mapped), P=100%
- **Selection rationale**: 17 libraries surveyed; tower is the only one achieving R>=90%
- **Import pattern**: submodule imports (`use tower::util::ServiceExt`) — not barrel re-export dominant
- **GT size**: 19 test files (small but meaningful for normal-case validation)
- **Verdict**: PASS — tower satisfies ship criteria threshold (P>=98%, R>=90%). Proceed to GT doc creation and documentation update.

### Why tower qualifies as "normal-case"

tokio (R=50.8%) and clap (R=14.3%) both fail due to crate-root barrel re-export as the dominant FN cause. tower uses submodule direct imports, which is the pattern Rust observe is designed to handle well. Confirming R>=90% on tower demonstrates the implementation is sound for normal import patterns.

## Test List

### TODO
(none)

### DONE
- [x] TC-01: Given tower observe output, When count mapped test files, Then >= 18 -- RESULT: 18 unique test files mapped (10 external + 8 inline). PASS (>= 18).
- [x] TC-02: Given 10-pair spot-check of tower GT doc, When verify each mapping, Then all 10 pairs correct -- RESULT: All 18 TP verified via full audit. PASS.
- [x] TC-03: Given `cargo test`, When run all 1202+ tests, Then PASS (no regression) -- RESULT: All tests pass (no code changes, doc-only cycle). PASS.
- [x] TC-04: Given tower GT doc created, When verify final P/R, Then P>=98% R>=90% -- RESULT: P=100% PASS, R=78.3% FAIL. Note: cycle doc's R=94.7% was based on misreading observe summary. Actual GT-based recall is 78.3%. Ship criteria R>=90% not met.

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal

tower を Rust observe の normal-case GT library として確定し、ship criteria (P>=98%, R>=90%) を正式に評価可能にする。GT doc を作成し、dogfooding-results.md / STATUS.md / ROADMAP.md に結果を反映する。

### Background

Rust observe は tokio (R=50.8%) と clap (R=14.3%) のみで測定済み。両方とも crate root barrel re-export が dominant FN cause の "hard case"。ship criteria 評価には "normal-case" library (barrel re-export に依存しない) が必要。

17 library を候補調査した結果、tower (commit 251296d) が唯一 R>=90% を達成。tower のテストは `use tower::util::ServiceExt` 等のサブモジュール直接 import を使用しており、barrel re-export に依存しない正常系パターン。

### Design Approach

1. tower の observe 出力を取得し、GT 19 ペアを全数監査
2. GT doc を `docs/observe-ground-truth-rust-tower.md` に作成 (clap GT format に準拠)
3. P/R を計算し TC-04 を検証
4. dogfooding-results.md の Rust セクションに tower 結果を追記
5. STATUS.md の Rust observe P/R を tower 結果で更新
6. ROADMAP.md の ship criteria evaluation status を更新

## Verification

```bash
cargo test
cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tower
```

Evidence:
- `cargo test`: All tests PASS (no code changes in this cycle)
- observe output: `test_files: 19, mapped_files: 19` (production-centric view; actual unique test files mapped = 18)
- GT full audit: 18 TP (10 external + 8 inline), 0 FP, 5 FN
- P=100%, R=78.3%

## Progress Log

### 2026-03-25 22:28 - INIT
- Cycle doc created
- Scope definition ready
- Design Review Gate: PASS (tower R=94.7% P=100%, 17 libraries surveyed)

### 2026-03-25 - GREEN
- tower observe output collected: 18 unique test files mapped (10 external + 8 inline)
- Full audit of tower/tests/ (18 files): support.rs = helper, limit/main + util/main = module entries
- GT doc created: `docs/observe-ground-truth-rust-tower.md`
- Corrected P/R: P=100%, R=78.3% (18/23). Cycle doc's R=94.7% was misread from observe summary.
- FN root cause: 5 external files import types defined in mod.rs files (filter, hedge, steer modules)
- 17-library survey confirmed: no library achieves R>=90%. Rust observe ship criteria remain unmet.
- dogfooding-results.md updated: 17-library survey table + tower full audit results
- STATUS.md updated: tower GT results + progress log entry
- ROADMAP.md updated: status table + Completed Recently section + Decision records

---

## Next Steps

1. [Done] INIT
2. [Done] PLAN
3. [Done] RED
4. [Done] GREEN <- Current
5. [ ] REFACTOR
6. [ ] REVIEW
7. [ ] COMMIT
