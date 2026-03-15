---
feature: Import Tracing (Layer 2)
phase: REVIEW
complexity: medium
test_count: 16
risk_level: low
codex_mode: no
created: 2026-03-15
updated: 2026-03-15
---

# Import Tracing (Layer 2)

Phase 8b Task #3b: Issue #78

Layer 1 (filename convention) は同一ディレクトリのテストのみマッチする。NestJS 等の実プロジェクトではテストが `test/` や `__tests__/` に分離されるケースが多く、Layer 1 だけでは recall が不足する。Layer 2 はテストファイルの `import` 文を tree-sitter で解析し、import 先のパスを解決してプロダクションファイルとのマッピングを確立する。

## Files to Change

- `crates/lang-typescript/queries/import_mapping.scm` (新規)
- `crates/lang-typescript/src/observe.rs` (変更: 型追加、関数追加、テスト追加)
- `crates/lang-typescript/Cargo.toml` (変更: `tempfile` dev-dependency 追加)
- `tests/fixtures/typescript/observe/import_named.ts` (新規 fixture)
- `tests/fixtures/typescript/observe/import_default.ts` (新規 fixture)
- `tests/fixtures/typescript/observe/import_mixed.ts` (新規 fixture)

## Design Approach

### 責務分離

| 層 | 責務 |
|----|------|
| `import_mapping.scm` | `(module_specifier, symbol_name)` ペアの抽出のみ |
| `extract_imports()` | クエリ実行 + 相対パスフィルタ |
| `resolve_import_path()` | パス解決 + セキュリティガード |
| `map_test_files_with_imports()` | Layer 1 → Layer 2 の統合 |

### スコープ外

- barrel export (`index.ts` re-export) の再帰解決
- directory index 解決 (`../foo` → `../foo/index.ts`)
- `import * as ns` (namespace import)
- `require()` 呼び出し

### 型の追加・変更

新規 `ImportMapping`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ImportMapping {
    pub symbol_name: String,       // "UsersController"
    pub module_specifier: String,  // "./users.controller"
    pub file: String,
    pub line: usize,
}
```

`MappingStrategy` に `ImportTracing` variant を追加:

```rust
pub enum MappingStrategy {
    FileNameConvention,
    ImportTracing,  // 追加
}
```

## Test List

### extract_imports テスト (fixture ベース)

- [x] IM1: named import の symbol と specifier が抽出される
- [x] IM2: 複数 named import (`{ A, B }`) が 2件返る
- [x] IM3: エイリアス import (`{ A as B }`) で元の名前 "A" が返る
- [x] IM4: default import の symbol と specifier が抽出される
- [x] IM5: npm パッケージ import (`@nestjs/testing`) が除外される (空Vec)
- [x] IM6: 相対 `../` パスが含まれる
- [x] IM7: 空ソースで空Vec が返る

### resolve_import_path テスト (tempdir)

- [x] RP1: 拡張子なし specifier + 実在 `.ts` ファイル → Some(canonical path)
- [x] RP2: 拡張子付き specifier (`.ts`) + 実在ファイル → Some(canonical path)
- [x] RP3: 存在しないファイル → None
- [x] RP4: scan_root 外のパス (`../../outside`) → None
- [x] RP5: 拡張子なし specifier + 実在 `.tsx` ファイル → Some(canonical path)

### map_test_files_with_imports テスト (tempdir)

- [x] MT1: Layer 1 マッチ + Layer 2 マッチが共存 → 両方マッピングされる
- [x] MT2: クロスディレクトリ import → ImportTracing でマッチ
- [x] MT3: npm import のみ → 未マッチ
- [x] MT4: 1テストが複数 production を import → 両方にマッピング

## Implementation Order

1. RED: IM1-IM7 テスト + fixture 作成 → 失敗確認
2. GREEN: `import_mapping.scm` + `extract_imports()` 実装
3. RED: RP1-RP5 テスト → 失敗確認
4. GREEN: `resolve_import_path()` 実装
5. RED: MT1-MT4 テスト → 失敗確認
6. GREEN: `map_test_files_with_imports()` 実装
7. REFACTOR
8. Self-dogfooding: `cargo run -- --lang rust .` BLOCK 0件確認

## Design Clarifications (from plan review)

1. **`map_test_files_with_imports()` シグネチャ**: `fn map_test_files_with_imports(&self, production_files: &[String], test_sources: &HashMap<String, String>, scan_root: &Path) -> Vec<FileMapping>` — `test_files` 引数は削除、`test_sources.keys()` で代替
2. **strategy 1対多**: Layer 1 マッチ済み production_file に Layer 2 テストを追加する場合、strategy は `FileNameConvention` を維持。Layer 2 のみでマッチした場合のみ `ImportTracing`。FileMapping は production_file ごとに1つ。
3. **canonicalize**: `scan_root` 自体も `canonicalize()` した上で比較する
4. **require() 除外**: `import_statement` ノードは `require()` を含まないため、.scm 設計で自然に除外される

## Progress Log

### sync-plan (2026-03-15)

- Design Review Gate: PASS (score: 0)
  - Scope: 変更ファイル 6件、スコープ外明示、YAGNI 問題なし
  - Architecture: OnceLock キャッシュパターン踏襲、`cached_query()` / `FileMapping` / `MappingStrategy` 既存型との整合確認
  - Test List: 16件、正常系/境界値/異常系カバー、Given/When/Then 形式
  - Risk: path traversal ガード設計済み (RP4 がカバー)
- Cycle doc created: `docs/cycles/20260315_1555_import_tracing_layer2.md`

### RED (2026-03-15)

- 型追加: `ImportMapping`, `MappingStrategy::ImportTracing`
- スタブ追加: `extract_imports()`, `resolve_import_path()`, `map_test_files_with_imports()`
- fixture 作成: `import_named.ts`, `import_default.ts`, `import_mixed.ts`
- `Cargo.toml`: `tempfile = "3"` dev-dependency 追加
- テスト作成: IM1-IM7, RP1-RP5, MT1-MT4 (計16件) — 全件 `todo!()` パニックで失敗を確認
- Self-dogfooding: BLOCK 0件確認

### Plan Review (2026-03-15)

- Claude design-reviewer: WARN (score: 35) → 2件は plan 既出、解決済み
- Codex review: WARN (3 warn, 1 info)
  - Accept: `test_files` 引数削除、`test_sources.keys()` で代替
  - Accept: directory index 解決をスコープ外に明示
  - Reject: `symbol_name` は将来の gap analysis で必要（observe PoC の設計判断）
- 統合判定: PASS — findingsへの対応完了、Block 2 進行可

### GREEN (2026-03-15)

- `import_mapping.scm` 作成: named import + default import パターン
- `extract_imports()` 実装: OnceLock キャッシュ + 相対パスフィルタ
- `resolve_import_path()` 実装: 拡張子試行 + canonicalize ガード
- `map_test_files_with_imports()` 実装: Layer 1 → Layer 2 統合
- dotted module name の拡張子置換問題を修正 (format!で append)
- 141 tests passed

### REFACTOR (2026-03-15)

- Layer 2 ループ内の重複テスト追加防止 (HashSet で dedup)
- Verification Gate: 141 tests, clippy 0, fmt OK, BLOCK 0
- Phase completed

### REVIEW (2026-03-15)

- Risk: HIGH (score: 75) → security + correctness + performance panel
- Security: PASS (5) — canonicalize+starts_with ガード正常、symlink 防御OK
- Correctness: WARN (35) → namespace import 未対応 (DISCOVERED)、mixed strategy (Reject: 設計意図通り)
- Performance: PASS — 冗長 canonicalize は idempotent、PoC 許容
- Codex review: 的外れ (dev-crew プラグイン自体をレビュー)
- 統合: PASS (score: 20)
- Phase completed

## DISCOVERED

- [x] namespace import (`import * as Ns from './module'`) の Layer 2 対応 → issue #83
