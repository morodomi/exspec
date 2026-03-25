---
feature: php-observe-parent-class-import-propagation
cycle: 20260326_0819
phase: GREEN
complexity: standard
test_count: 6
risk_level: medium
codex_session_id: ""
created: 2026-03-26 08:19
updated: 2026-03-26
---

# PHP observe: parent class import propagation

## Scope Definition

### In Scope
- [ ] New tree-sitter query `extends_class.scm` to extract parent class name from `class Foo extends Bar`
- [ ] New function `extract_parent_class_imports()` in `crates/lang-php/src/observe.rs`
- [ ] Merge parent imports into child's `raw_specifiers` before L2 resolution
- [ ] External namespace guard (skip PHPUnit\Framework\TestCase etc.)
- [ ] Circular inheritance guard (visited set)

### Out of Scope
- Grandparent traversal (2+ levels) (Reason: complexity vs. gain; 1-level covers AbstractBladeTestCase pattern)
- `implements` interface resolution (Reason: interfaces do not carry `use` statements for production classes)

### Files to Change (target: 10 or less)
- `crates/lang-php/src/observe.rs` (edit)
- `crates/lang-php/queries/extends_class.scm` (new)
- `tests/fixtures/php/` (new fixture files for parent propagation)

## Environment

### Scope
- Layer: Backend
- Plugin: lang-php (Rust implementation)
- Risk: 30 (PASS)

### Runtime
- Language: Rust (exspec implementation language)

### Dependencies (key packages)
- tree-sitter: workspace version
- tree-sitter-php: workspace version

### Risk Interview (BLOCK only)
N/A -- Risk 30 (PASS)

## Context & Dependencies

### Reference Documents
- `crates/lang-php/src/observe.rs` -- existing `map_test_files_with_imports`, `extract_raw_import_specifiers`
- `crates/lang-php/queries/import_mapping.scm` -- existing import extraction query

### Dependent Features
- L2 import resolution: `crates/lang-php/src/observe.rs` (`map_test_files_with_imports`)
- PSR-4 namespace resolution: existing infrastructure in `crates/lang-php/src/observe.rs`

### Related Issues/PRs
- (none tracked)

## Test List

### TODO
- [ ] TC-01: Given test file extends ParentClass in same dir, When parent has `use Illuminate\Foo`, Then Foo is in test's import list
- [ ] TC-02: Given test file extends ParentClass, When parent has no production imports, Then no additional imports are added (guard)
- [ ] TC-03: Given test file extends external class (PHPUnit\TestCase), When resolve parent, Then skip (external namespace guard)
- [ ] TC-04: Given circular inheritance (A extends B, B extends A), When resolve, Then no infinite loop
- [ ] TC-05: Given Laravel observe after fix, When measure recall, Then R > 90% (#[ignore] integration)
- [ ] TC-06: Given Laravel observe after fix, When check precision, Then no new FP (#[ignore] integration)

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal
PHP observe recall improvement: R=88.6% -> ~94.5% on Laravel ground truth. AbstractBladeTestCase pattern -- child test classes do not directly `use` production namespaces; they inherit them from a parent class. Propagating parent `use` statements to child classes enables L2 resolution to find these mappings.

### Background
PHP observe R=88.6% の 54 FN は AbstractBladeTestCase 経由の parent class import が原因。子テストが直接 `use Illuminate\...` を持たず、親クラスの import が L2 で追跡されない。

### Design Approach

**Injection Point**: `crates/lang-php/src/observe.rs` の `map_test_files_with_imports()` (line 469 付近)。L2 resolution の前に parent class imports をマージする。

**Step 1 -- New tree-sitter query** (`extends_class.scm`):
Extract `Bar` from `class Foo extends Bar`.

**Step 2 -- New function** `extract_parent_class_imports(source, test_path, scan_root, production_files)`:
1. tree-sitter で `extends ParentClass` を検出
2. 同一ディレクトリから `ParentClass.php` を探す (L1 stem match)
3. 見つからなければ PSR-4 で解決
4. 親ファイルの source を読み、`extract_raw_import_specifiers()` で import 取得
5. 外部 namespace filter 済みの import を返す

**Step 3 -- Merge**: `raw_specifiers` + `parent_specifiers` を結合して L2 resolution に渡す。

**Constraints**:
- 1-level のみ (grandparent は追跡しない)
- `extends` のみ (`implements` は対象外)
- 親が external namespace (PHPUnit\Framework\TestCase 等) の場合はスキップ
- circular inheritance guard (visited set)

**Expected Impact**:
- Laravel: +54 FN 解消 (AbstractBladeTestCase) -> R=88.6% -> ~94.5%
- FP risk: Low (親の import は子の import と同品質)

## Verification

```bash
cargo run -- observe --lang php --format json /tmp/laravel
```

Evidence: (orchestrate が自動記入)

## Design Review Gate

### Assessment

| Item | Status | Notes |
|------|--------|-------|
| Injection point is well-defined | PASS | `map_test_files_with_imports()` は L2 resolution の直前。変更箇所が局所的 |
| External namespace guard prevents FP | PASS | PHPUnit\Framework\TestCase など外部親クラスはスキップ設計 |
| Circular inheritance guard present | PASS | visited set で無限ループ防止 |
| 1-level only constraint is justified | PASS | grandparent は稀なケース。追加コストに対しゲインが薄い |
| FP impact analysis | PASS | 親 import は子 import と同品質。新規 FP は低リスク |
| Test coverage of edge cases | PASS | TC-03 (external skip), TC-04 (circular) がエッジをカバー |
| Integration tests are #[ignore] | PASS | Laravel GT テストは `#[ignore]` で CI を汚染しない |

**Verdict**: PASS -- 設計に問題なし。RED phase に進む。

## Progress Log

### 2026-03-26 08:19 - INIT
- Cycle doc created from plan file `/Users/morodomi/.claude/plans/spicy-sauteeing-pine.md`
- Scope definition ready
- Design Review Gate: PASS

### 2026-03-26 - GREEN
- `crates/lang-php/queries/extends_class.scm` 新規作成: `class Foo extends Bar` から `@parent_class` を抽出するクエリ
- `crates/lang-php/src/observe.rs` に `extract_parent_class_imports()` を追加 (1-level, same dir only)
- `map_test_files_with_imports()` で parent imports を child imports にマージし L2 resolution に渡す
- TC-01 to TC-04: all PASS (TC-05, TC-06 は #[ignore] integration test)
- Full suite: 全テスト PASS、clippy 0 errors、fmt clean、self-dogfooding BLOCK 0

---

## Next Steps

1. [Done] INIT
2. [Done] PLAN (approved)
3. [Done] RED
4. [Done] GREEN <- Current
5. [Next] REFACTOR
6. [ ] REVIEW
7. [ ] COMMIT
