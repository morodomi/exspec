---
feature: ts-observe-dynamic-import
cycle: 20260326_1157
phase: RED
complexity: standard
test_count: 4
risk_level: low
codex_session_id: ""
created: 2026-03-26 11:57
updated: 2026-03-26 11:57
---

# TS observe dynamic import support

## Scope Definition

### In Scope
- [ ] `import_mapping.scm` に dynamic import パターンを追加
- [ ] `observe.rs` の `extract_imports_impl` で dynamic import の module path を抽出
- [ ] symbol なしの場合も module path → production file マッピングが機能すること
- [ ] 既存 boundary テスト `boundary_b5_dynamic_import_not_extracted` を更新

### Out of Scope
- symbol の完全抽出 (`const { foo } = await import('...')` の `foo` まで追う): 今サイクルは module path のみ
- Python / PHP / Rust の dynamic import 対応

### Files to Change (target: 10 or less)
- `crates/lang-typescript/queries/import_mapping.scm` (edit)
- `crates/lang-typescript/src/observe.rs` (edit)
- `tests/fixtures/typescript/observe/import_dynamic.ts` (edit)
- テストファイル (edit/new)

## Environment

### Scope
- Layer: Backend (static analysis)
- Plugin: ts (lang-typescript crate)
- Risk: 10 (PASS)

### Runtime
- Language: Rust (Cargo workspace)
- tree-sitter-typescript: workspace依存バージョン

### Dependencies (key packages)
- tree-sitter: workspace
- tree-sitter-typescript: workspace
- lang-typescript: workspace crate

### Risk Interview (BLOCK only)
(なし — Risk PASS)

## Context & Dependencies

### Reference Documents
- `docs/languages/typescript.md` - TS observe 仕様
- `CONSTITUTION.md` - observe ship criteria (P>=98%, R>=90%)
- `crates/lang-typescript/queries/import_mapping.scm` - 変更対象クエリ

### Dependent Features
- TS path alias 解決 (Layer 2b): `observe.rs` 内の既存パイプライン。dynamic import も同パイプラインに流す

### Related Issues/PRs
(なし)

## Test List

### TODO
- [ ] TC-01: Given test with `await import('./user.service')`, When observe runs, Then maps to `user.service.ts`
- [ ] TC-02: Given test with `await import('@/lib/api-client')`, When observe runs with tsconfig paths, Then maps to `src/lib/api-client.ts`
- [ ] TC-03: Given test with `const { foo } = await import('./bar')`, When observe runs, Then maps to `bar.ts` (destructured dynamic import)
- [ ] TC-04: Existing boundary test `boundary_b5_dynamic_import_not_extracted` needs update (was asserting empty, now should assert extraction)

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal
TS observe で dynamic import (`import('...')`) からモジュールパスを抽出し、既存の path alias 解決パイプラインに流すことで、内部 dogfooding プロジェクト A の observe R を 49% から大幅改善する。

### Background
内部 dogfooding プロジェクト A (TS/Next.js) で observe R=49% (37/75)。Root cause: 38件の unmapped テストのうち約30件が `await import('@/lib/...')` (dynamic import + path alias) パターンを使用。exspec の TS import extraction は静的 import のみ対応しており、dynamic import を検出していない。

### Design Approach
1. `import_mapping.scm` に dynamic import パターンを追加:
   ```scm
   ;; Dynamic import: import('./module') or await import('./module')
   (call_expression
     function: (import)
     arguments: (arguments
       (string
         (string_fragment) @module_specifier)))
   ```
   Note: tree-sitter-typescript では `import(...)` の `import` は `identifier` ではなく `import` ノード。

2. `observe.rs` の `extract_imports_impl` で:
   - dynamic import から `module_specifier` を抽出（`@symbol_name` capture なし）
   - symbol なしの場合でも module path として `extract_all_import_specifiers` に渡す
   - 既存の path alias 解決 (Layer 2b) が自動適用

3. Symbol の扱い: 簡易アプローチとして symbol を空 Vec で渡す。`extract_all_import_specifiers` は既に (specifier, symbols) のタプルを返すため互換。

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-26 11:57 - INIT
- Cycle doc created
- Scope definition ready

---

## Next Steps

1. [Done] INIT <- Current
2. [Done] PLAN
3. [Next] RED
4. [ ] GREEN
5. [ ] REFACTOR
6. [ ] REVIEW
7. [ ] COMMIT
