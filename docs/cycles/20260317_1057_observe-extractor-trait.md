---
feature: "Phase 9a — ObserveExtractor trait extraction"
cycle: "20260317_1057"
phase: DONE
complexity: standard
test_count: 7
risk_level: medium
codex_session_id: ""
created: 2026-03-17 10:57
updated: 2026-03-17 10:57
---

# Cycle: ObserveExtractor trait extraction (Phase 9a)

## Scope Definition

### In Scope
- [ ] `ObserveExtractor` trait を `crates/core/` に新規定義
- [ ] `crates/core/lib.rs` に `observe` モジュールを追加してトレイトを公開
- [ ] `crates/lang-typescript/src/observe.rs` を `ObserveExtractor` 実装に変換
- [ ] `crates/cli/src/main.rs` をトレイトオブジェクト経由のディスパッチに変更
- [ ] 既存 TypeScript observe テスト 99 件が変更なしでパスすること

### Out of Scope
- Python/Rust/PHP observe 実装 (Phase 9b/9c/9d)
- route extraction (framework-specific, 将来対応)
- `ObserveReport.routes` フィールドのリファクタリング (route extraction 対応時に実施)
- TypeScript barrel/re-export 解決ロジックの変更

### Files to Change
- `crates/core/src/observe.rs` (new)
- `crates/core/src/lib.rs` (edit — `pub mod observe` 追加)
- `crates/lang-typescript/src/observe.rs` (edit — `ObserveExtractor` 実装)
- `crates/cli/src/main.rs` (edit — trait-based dispatch)

## Environment

### Scope
- Layer: Backend (Rust crate)
- Plugin: dev-crew:rust-quality (cargo test / clippy / fmt)
- Risk: 45/100 (WARN)

### Runtime
- Language: Rust (stable)
- Workspace: 6 crates (core / lang-python / lang-typescript / lang-php / lang-rust / cli)

### Dependencies (key crates)
- tree-sitter: AST解析
- serde / serde_json: ObserveReport シリアライズ

### Risk Interview (WARN)
- Risk type: Refactoring (大規模リネーム、既存テスト互換性)
- リファクタリング規模: 3,880行 (lang-typescript/observe.rs)
- 既存テスト互換性: 99件のテストが変更なしでパスすること必須
- API破壊のリスク: trait 境界を慎重に設計することで軽減

## Context & Dependencies

### Reference Documents
- [ROADMAP.md §9a] — ObserveExtractor trait extraction の設計方針
- [docs/cycles/20260316_2005_helper_filter_extension.md] — 直前の observe サイクル (Task 7.5)
- [docs/cycles/20260315_0821_production_function_extractor.md] — observe 基盤実装
- [docs/observe-eval-results.md] — 現在の evaluate 結果 (TS: Precision 90.3%)
- [docs/observe-boundaries.md] — observe のスコープ境界

### Dependent Features
- Phase 9b (Python observe): このサイクルの trait 定義に依存
- Phase 9c (Rust observe): 同上
- Phase 9d (PHP observe): 同上

### Related Issues/PRs
- ROADMAP.md Phase 9a: ObserveExtractor trait extraction

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] TC-01: MockExtractor で `map_test_files` の Layer 1 stem matching が動作
  - Given: core/observe.rs に MockExtractor
  - When: `map_test_files(mock, ...)`
  - Then: stem matching でテストファイルがプロダクションファイルにマッピングされる
- [x] TC-02: MockExtractor で `resolve_import_path` が mock の source_extensions でプローブ
  - Given: core/observe.rs に MockExtractor
  - When: `resolve_import_path(mock, ...)`
  - Then: mock の source_extensions で拡張子をプローブして解決
- [x] TC-03: MockExtractor で `is_barrel_file` が mock の index_file_names で判定
  - Given: core/observe.rs に MockExtractor
  - When: `is_barrel_file(mock, path)`
  - Then: mock の index_file_names に含まれるファイル名で判定
- [x] TC-04: 既存 TypeScript observe テスト全件が thin wrapper 経由でパス
  - Given: TypeScriptExtractor が ObserveExtractor を impl
  - When: `cargo test -p exspec-lang-typescript`
  - Then: 全件 PASS (テスト側変更なし)
- [x] TC-05: CLI テストが trait-based dispatch でパス
  - Given: trait-based CLI dispatch
  - When: `cargo test -p exspec`
  - Then: CLI テスト通過
- [x] TC-06: `cargo clippy -- -D warnings` で 0 errors
  - Given: 全変更適用後
  - When: `cargo clippy -- -D warnings`
  - Then: 0 errors
- [x] TC-07: self-dogfooding — `cargo run -- --lang rust .` で BLOCK 0件
  - Given: リファクタリング完了後の exspec 自身
  - When: `cargo run -- --lang rust .`
  - Then: BLOCK 0件

## Implementation Notes

### Goal
TypeScript 固有の observe 実装から言語非依存の `ObserveExtractor` trait を抽出し、CLI をトレイトオブジェクト経由でディスパッチする構造に変換する。Phase 9b 以降の Python/Rust/PHP observe 実装が trait を implement するだけで CLI に統合できる状態にする。

### Background
現在 observe ロジックは全て `crates/lang-typescript/src/observe.rs` に集中している。CLI (`cli/main.rs`) も TypeScript をハードコードでディスパッチしている。Phase 9 で多言語展開するには、共通インタフェースが必要。

ROADMAP §9a より:
- `ObserveReport`, `ObserveFileEntry`, `ObserveSummary` はすでに `core/observe_report.rs` にあり言語非依存
- Two-layer mapping アルゴリズムを trait に抽出
- CLI は `Box<dyn ObserveExtractor>` でディスパッチ

### Design Approach

**Two-layer mapping アルゴリズム** を **core の free function** として実装し、言語固有の処理は trait メソッドで注入する。

```
ObserveExtractor trait (crates/core/src/observe.rs)
  // --- Language-specific extractors ---
  fn extract_production_functions(&self, source, file_path) -> Vec<ProductionFunction>
  fn extract_imports(&self, source, file_path) -> Vec<ImportMapping>
  fn extract_all_import_specifiers(&self, source) -> Vec<(String, Vec<String>)>
  fn extract_barrel_re_exports(&self, source, file_path) -> Vec<BarrelReExport>
  // --- Language configuration ---
  fn source_extensions(&self) -> &[&str]
  fn index_file_names(&self) -> &[&str]
  fn production_stem(&self, path) -> Option<&str>
  fn test_stem(&self, path) -> Option<&str>
  fn is_non_sut_helper(&self, file_path, is_known_production) -> bool
  // --- Default impls ---
  fn is_barrel_file(&self, path) -> bool  // index_file_names check
  fn file_exports_any_symbol(&self, path, symbols) -> bool  // default: true
  fn resolve_alias_imports(&self, source, scan_root) -> Vec<...>  // default: empty

Core free functions (crates/core/src/observe.rs)
  pub fn map_test_files(ext: &dyn ObserveExtractor, ...) -> Vec<FileMapping>
  pub fn map_test_files_with_imports(ext: &dyn ObserveExtractor, ...) -> Vec<FileMapping>
  pub fn resolve_import_path(ext: &dyn ObserveExtractor, ...) -> Option<String>
  pub fn resolve_barrel_exports(ext: &dyn ObserveExtractor, ...) -> Vec<PathBuf>

TypeScriptExtractor (crates/lang-typescript/src/observe.rs)
  impl ObserveExtractor for TypeScriptExtractor
  // Route/Decorator/NestJS 固有コードは inherent method として維持

CLI dispatch (crates/cli/src/main.rs)
  let extractor: Box<dyn ObserveExtractor> = match lang {
    "typescript" => Box::new(TypeScriptExtractor::new()),
    other => bail!("observe not yet supported for {}", other),
  };
```

**trait 境界設計 (ROADMAP §9a review gate)**:
- trait に default impl としてアルゴリズムを置くと "God trait" になるため、free function + trait injection を採用
- 言語固有の extraction/configuration は trait メソッド、共通アルゴリズムは free function
- `is_non_sut_helper` は trait メソッド (言語ごとに判定ロジックが異なる)

**既存テスト戦略**:
- `lang-typescript` の既存テスト 99 件はファイルパスを変えず in-place でリファクタリング
- テスト側から見た関数シグネチャを維持する (テストを書き直さない)

## Progress Log

### 2026-03-17 10:57 - INIT
- Cycle doc 作成 (sync-plan)
- Plan: Phase 9a ObserveExtractor trait extraction
- Scope: core/observe.rs (new) + core/lib.rs + lang-typescript/observe.rs + cli/main.rs

### 2026-03-17 - plan-review
- design-reviewer PASS (score 35)
- resolve_barrel_exports_inner の具象型参照、file_exports_any_symbol 配置、tsconfig alias 解決の3点を確認

### 2026-03-17 - RED
- TC-01〜TC-06 作成、TC-01/TC-01b が todo! で失敗確認

### 2026-03-17 - GREEN
- core/observe.rs: trait + types + free functions 完成
- lang-typescript: impl ObserveExtractor, thin wrappers
- CLI: MappingStrategy import 更新
- 全839テスト PASS

### 2026-03-17 - REFACTOR
- resolve_absolute_base_to_file 重複を core 委譲に統一

### 2026-03-17 - REVIEW
- correctness-reviewer PASS (score 35)

### 2026-03-17 - COMMIT
- feat/observe-extractor-trait ブランチ (9f1e9fb)
- 5 files changed, 689 insertions, 279 deletions

---

## Next Steps

1. [Done] INIT
2. [Done] plan-review
3. [Done] RED
4. [Done] GREEN
5. [Done] REFACTOR
6. [Done] REVIEW
7. [Done] COMMIT
