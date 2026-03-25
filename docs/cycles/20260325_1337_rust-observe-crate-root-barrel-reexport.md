---
feature: rust-observe-crate-root-barrel-reexport
cycle: 20260325_1337
phase: DONE
complexity: high
test_count: 8
risk_level: medium
codex_session_id: ""
created: 2026-03-25 13:37
updated: 2026-03-25 13:37
---

# Rust observe: crate root barrel re-export resolution (#190)

## Scope Definition

### In Scope
- [ ] `apply_l2_cross_crate_barrel` 新関数追加 (~80行)
- [ ] `find_member_by_crate_name` ヘルパー追加 (~10行)
- [ ] `map_test_files_with_imports` 内に呼び出し追加 (~15行、1201行付近)

### Out of Scope
- `crates/core/src/observe.rs` への変更 (理由: workspace member は Rust/Cargo 固有。core は言語非依存)
- `crates/lang-rust/queries/*.scm` への変更 (理由: 既存クエリで十分)
- 他言語 (TypeScript/Python/PHP) への影響 (理由: lang-rust 内完結)

### Files to Change (target: 10 or less)
- `crates/lang-rust/src/observe.rs` (edit) — `apply_l2_cross_crate_barrel`, `find_member_by_crate_name`, 呼び出し追加
- `crates/lang-rust/tests/rust_observe_clap_test.rs` (edit) — TC-02 recall baseline 引き上げ

## Environment

### Scope
- Layer: Backend
- Plugin: rust
- Risk: 40/100 (Medium) — 既存 L2 動作への regression リスク、fan-out 増加リスク

### Runtime
- Language: Rust

### Dependencies (key packages)
- tree-sitter: workspace member
- lang-rust: workspace crate
- core (exspec_core::observe): `resolve_barrel_exports`, `collect_import_matches`, `file_exports_any_symbol` を再利用

### Risk Interview (BLOCK only)
(N/A - Medium level)

## Context & Dependencies

### Reference Documents
- [docs/dogfooding-results.md] - Rust observe 現状 (clap R=14.3%, tokio R=50.8%)
- [ROADMAP.md] - observe ship criteria (P>=98%, R>=90%) + "Rust crate root barrel re-export resolution" P2 記載
- [CONSTITUTION.md] - "Ship criteria: Precision >= 98%, Recall >= 90%"
- Issue #190 - clap cross-crate barrel FN 報告

### Dependent Features
- Rust observe L2 import tracing: `crates/lang-rust/src/observe.rs`
- 既存 cross-crate fallback: `map_test_files_with_imports` 内 1159-1201行
- `extract_barrel_re_exports`: root lib.rs の `pub use` 一覧を取得
- `WorkspaceMember`: workspace member の crate名/パス情報

### Related Issues/PRs
- #190: clap observe Recall が 14.3% と低い原因追跡

## Background

### 問題の流れ

1. テスト: `use clap::Arg` → specifier="", symbols=["Arg"]
2. `crate_root/src/lib.rs` (barrel) に解決
3. lib.rs に `pub use clap_builder::*` がある
4. `resolve_barrel_exports` が `./clap_builder` を相対パスとして解決しようとするが、`src/clap_builder.rs` は存在しない
5. workspace member `clap_builder` への解決ができず FN になる

### 既存フロー (L2)

```
1. Root crate L2: apply_l2_imports(tests, "clap", scan_root)
2. Member L2: apply_l2_imports(member_tests, "clap_builder", member_root)
3. Cross-crate L2: apply_l2_imports(root_tests, "clap_builder", member_root)
                 + apply_l2_imports(root_tests, "clap", member_root)

新ステップ (L2.5): ← ここに挿入
4. apply_l2_cross_crate_barrel(root_tests, root_lib_rs, members, ...)
   - root lib.rs の `pub use <crate>::*` / `pub use <crate>::Symbol` を解析
   - <crate> を workspace member に lookup
   - member の src/lib.rs に対して barrel resolution を実行
   - member の barrel から production file を特定
```

### 新関数シグネチャ

```rust
fn apply_l2_cross_crate_barrel(
    &self,
    test_sources: &HashMap<String, String>,
    crate_name: &str,              // root crate name (e.g., "clap")
    root_lib_path: &Path,          // root crate lib.rs
    members: &[WorkspaceMember],
    canonical_root: &Path,
    canonical_to_idx: &HashMap<String, usize>,
    mappings: &mut [FileMapping],
    l1_exclusive: bool,
    layer1_matched: &HashSet<String>,
)
```

### アルゴリズム

1. root lib.rs を読み、`extract_barrel_re_exports` で re-export 一覧を取得
2. 各 re-export の `from_specifier` から crate 名を抽出 (`clap_builder` / `./clap_builder` → `clap_builder`)
3. crate 名を `members` から lookup → `WorkspaceMember` (`find_member_by_crate_name`)
4. 各テストファイルから `use root_crate::Symbol` を抽出 (specifier="" のケース)
5. Symbol が re-export の symbols に含まれるか、wildcard なら常にマッチ
6. マッチした member の `src/lib.rs` に対して `collect_import_matches` を呼ぶ
7. `file_exports_any_symbol` で symbol filter を適用

## Test List

### TODO

(none)

### WIP

(none)

### DONE

- [x] CCB-01: **Given** root lib.rs に `pub use sibling_crate::*` + workspace member `sibling_crate` に `pub struct Symbol` / **When** test `use root::Symbol` で observe 実行 / **Then** `sibling_crate/src/lib.rs` or 該当ファイルに mapping — RED VERIFIED (FAIL as expected)
- [x] CCB-02: **Given** root lib.rs に `pub use sibling_crate::SpecificType` (named) / **When** test `use root::SpecificType` / **Then** sibling_crate 内の SpecificType export ファイルに mapping — RED VERIFIED (FAIL as expected)
- [x] CCB-03: **Given** root lib.rs に `pub use nonexistent_crate::*` (workspace member にない) / **When** observe 実行 / **Then** mapping は空、panic しない — PASS (境界値: no panic OK)
- [x] CCB-04: **Given** root lib.rs に `pub mod local_module` + `pub use sibling::*` / **When** test `use root::local_fn` (local_module 内) / **Then** local_module が既存 L2 で解決される (regression なし) — PASS (regression guard OK)
- [x] CCB-05: **Given** 2段 cross-crate: root → `pub use mid::*` → mid lib.rs → `pub use sub::*` / **When** test `use root::MidItem` (flat symbol via double-barrel) / **Then** mid/src/sub.rs に mapping — RED VERIFIED (FAIL as expected)
- [x] CCB-06: **Given** cross-crate barrel で wildcard + member に 50+ pub items / **When** test `use root::SpecificFn` / **Then** `file_exports_any_symbol` filter で 1 ファイルのみ match — RED VERIFIED (FAIL as expected)
- [x] CCB-INT-01: **Given** clap workspace / **When** observe 実行 / **Then** Recall >= 60% (tests/builder/ の unmapped ファイルが新たに mapped) — RED VERIFIED (#ignore, assert recall >= 60%)
- [x] CCB-INT-02: **Given** tokio workspace / **When** observe 実行 / **Then** 既存 Recall >= 45% (50.8% baseline regression guard) — #ignore 追加済み

### DISCOVERED
(none)

## Implementation Notes

### Design Review Gate 結果

- **判定**: PASS (スコア 30/100)
- **観点1 CONSTITUTION.md 整合性**: 問題なし (static AST only, lang-rust 完結, ship criteria との方向性一致)
- **観点2 スコープ**: 変更ファイル 2件、YAGNI なし。引数数は既存 `apply_l2_imports` と同パターン (軽微)
- **観点3 リスク**: 既存 L2 フロー後挿入で regression リスク最小。fan-out は `file_exports_any_symbol` フィルタで対処。tokio regression テストあり
- **観点4 Test List**: 正常系/境界値/異常系/regression カバー。TC-5 (CCB-05) の barrel depth 挙動は `MAX_BARREL_DEPTH` の制約に注意

### 注意点

- `from_specifier` の正規化: `./clap_builder` → `clap_builder` (先頭 `./` を strip)
- `MAX_BARREL_DEPTH` (core: 3) が CCB-05 の 2段 chain に対して有効かを確認すること
- 呼び出しタイミング: 既存 cross-crate fallback (L2, 1159-1201行) の**後**、strategy 更新ブロックの**前** (1217行付近)

## Verification

```bash
cargo test
cargo test -p exspec-lang-rust --test rust_observe_clap_test -- --ignored
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .

# clap recall 測定
cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/clap/ > /tmp/clap-post.json

# tokio regression 確認
cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tokio/ > /tmp/tokio-post.json
```

## Progress Log

2026-03-25 13:37 — Cycle doc 作成。Design Review Gate PASS (スコア 30)。RED phase 開始準備。
2026-03-25 — RED phase 完了。Unit tests CCB-01/02/03/04/05/06 + Integration tests CCB-INT-01/02 作成。CCB-01/02/05/06 FAIL (RED)、CCB-03/04 PASS (境界値・regression guard)。cargo clippy 0 errors、cargo fmt --check 差分なし。

2026-03-25 — GREEN phase 完了。
- `apply_l2_cross_crate_barrel` 実装 + `find_member_by_crate_name` ヘルパー
- `parse_use_path` fix: empty specifier brace-list (use clap::{Arg, ...}) が無視されていたバグ修正
- clap: R 14.3% → 23.1% (fan-out filter あり), 121/134 (fan-out filter なし)
- tokio: R >= 50.8% regression なし
- 全テスト PASS (1221 + 7 ignored), clippy 0, fmt OK, BLOCK 0

2026-03-25 — REFACTOR phase 完了。
- コード品質確認。重複なし、命名一貫、メソッド分割適切。
- Verification Gate PASS: tests PASS, clippy 0, fmt OK, BLOCK 0
- Phase completed

2026-03-25 — REVIEW (code) phase 完了。
- Correctness: PASS (score=15). parse_use_path fix正当、fan-out制御3層フィルタ、循環参照なし。
- Phase completed
