---
feature: "Fix #98 — Rust observe: workspace-level aggregation for monorepos"
cycle: "20260318_1340"
phase: COMMIT
complexity: standard
test_count: 9
risk_level: low
codex_session_id: ""
created: 2026-03-18 13:40
updated: 2026-03-18 14:10
---

# Cycle: Fix #98 — Rust observe: workspace-level aggregation for monorepos

## Scope Definition

### In Scope
- [ ] `crates/lang-rust/src/observe.rs` 変更 — `WorkspaceMember` struct, `find_workspace_members`, `find_member_for_path`, L2 分岐ロジック

### Out of Scope
- 他言語 observe への影響
- TypeScript / Python / PHP の observe 変更
- L0/L1 マッピングロジックの変更

### Files to Change
- `crates/lang-rust/src/observe.rs` (edit)

## Environment

### Scope
- Layer: `crates/lang-rust`
- Plugin: dev-crew:rust-quality (cargo test / clippy / fmt)
- Risk: 25/100 (PASS)
- Runtime: Rust (cargo test)
- Dependencies: `std::fs` (既存、追加依存なし)

## Risk Interview

(BLOCK なし — リスク 25/100)

## Context & Dependencies

### 背景
`exspec observe --lang rust` は scan_root が Cargo workspace root の場合、`parse_crate_name()` が `None` を返すため Layer 2 (import tracing) が全無効になる。例: clap workspace (195 prod, 134 test) で L2=0。member 単位で実行すれば L2=2 (clap_complete) が得られる。

workspace でも L2 を有効にするため、メンバークレートを自動検出しメンバー別に L2 を実行するアグリゲーション機能を追加する。

### 設計方針
`map_test_files_with_imports` (observe.rs L718) で `parse_crate_name` が `None` を返した場合、workspace mode に分岐:

1. `find_workspace_members(scan_root)` で全メンバークレートを検出
2. 各テストファイルを所属メンバーに関連付け
3. メンバー別に crate_name + src root で L2 import tracing を実行
4. 結果を shared mappings にマージ

L0/L1 は変更不要 (workspace レベルで既に動作)。

### 参照ドキュメント
- `crates/lang-rust/src/observe.rs` (既存実装)
- Issue #98: workspace-level observe aggregation
- ROADMAP.md (Phase 9d: Rust observe)

### Related Issues/PRs
- Issue #98: Rust observe workspace aggregation

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] RS-WS-01: workspace with 2 members — `find_workspace_members` が 2 つの `WorkspaceMember` を返す
- [x] RS-WS-02: single crate (non-workspace) returns empty — workspace root でない場合は空 Vec を返す
- [x] RS-WS-03: target/ directory is skipped — `target/` 配下の Cargo.toml を無視する
- [x] RS-WS-04: hyphenated crate name → underscore conversion — `my-crate` が `my_crate` として返る
- [x] RS-WS-05: test file in member/tests/ — `find_member_for_path` が正しいメンバーを返す
- [x] RS-WS-06: test file not in any member — マッチするメンバーなしで `None` を返す
- [x] RS-WS-07: longest prefix match for nested members — ネストしたメンバー間で最深パスを選択する
- [x] RS-WS-E2E-01: workspace L2 import tracing works — workspace root 指定で L2 が有効になる
- [x] RS-WS-E2E-02: L0/L1 still work at workspace level — workspace root 指定で L0/L1 は維持される

## Implementation Notes

### Goal
`map_test_files_with_imports` が workspace root で呼ばれた場合に、メンバークレートを自動検出して L2 import tracing を有効化する。clap workspace のような大規模 Rust monorepo でも observe 精度を維持する。

### Background
`parse_crate_name(scan_root)` は workspace root の `Cargo.toml` に `[package]` セクションがない場合に `None` を返す。現状はこれで L2 が全無効化されるが、各メンバークレートは個別に `[package]` を持つため、メンバー単位で L2 を実行できる。

### Design Approach
1. `WorkspaceMember` struct を定義 — `crate_name: String`, `member_root: PathBuf`
2. `find_workspace_members(scan_root: &Path) -> Vec<WorkspaceMember>` を追加
   - `std::fs::read_dir` で再帰走査
   - `target/`, `.` 始まりディレクトリをスキップ
   - 各 `Cargo.toml` に `parse_crate_name(parent)` を呼ぶ
   - scan_root 直下は除外 (workspace root 自体)
3. `find_member_for_path(path: &Path, members: &[WorkspaceMember]) -> Option<&WorkspaceMember>` を追加
   - path が `member.member_root` のプレフィックスに一致するメンバーを返す
   - 複数マッチ時は最長パス (最深ネスト) を選択
4. `map_test_files_with_imports` に workspace 分岐を追加
   - `parse_crate_name` が `None` → `find_workspace_members` → メンバー別 L2 実行 → マージ

## Progress Log

### 2026-03-18 13:40 - INIT
- Cycle doc created
- Plan transferred from approved plan file (#98)

### 2026-03-18 14:10 - RED→GREEN→REFACTOR→REVIEW→COMMIT
- RED: 9テスト追加（RS-WS-01〜07, RS-WS-E2E-01〜02）。compile error でRED確認
- GREEN: `WorkspaceMember` struct, `find_workspace_members`, `find_member_for_path`, `apply_l2_imports` を実装。`map_test_files_with_imports` にworkspace分岐追加
- DISCOVERED: Layer 1 はディレクトリ内マッチのみ（既知の制約）。E2E-02を同ディレクトリテストに修正
- DISCOVERED: `parse_crate_name` が None でも `use crate::` は機能するため fallback（pseudo crate_name "crate"）を追加
- REFACTOR: `&mut Vec<FileMapping>` → `&mut [FileMapping]` (clippy ptr_arg)、rustfmt適用
- REVIEW: BLOCK 0件（self-dogfooding）、clippy 0 errors、全テスト通過 (971 total)
- COMMIT: Ready

## Next Steps

1. [Done] INIT
2. [Done] RED
3. [Done] GREEN
4. [Done] REFACTOR
5. [Done] REVIEW
6. [Current] COMMIT
