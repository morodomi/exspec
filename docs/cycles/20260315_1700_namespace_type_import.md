---
feature: Namespace import + type-only import filtering
phase: COMMIT
complexity: low
test_count: 2
risk_level: low
created: 2026-03-15
updated: 2026-03-15
---

# Namespace import (#83) + type-only import filtering (#84)

Phase 8b: Layer 2 import tracingの未対応パターン修正。

## Context

Layer 2 import tracing (Issue #78) は named/default import のみサポート。REVIEW phaseで2つの未対応パターンが発見された:
- **#83**: `import * as Ns from './module'` (namespace import) が未検出 → recall低下
- **#84**: `import type { X } from './module'` がフィルタされず → precision低下

## Files to Change

- `crates/lang-typescript/queries/import_mapping.scm` — namespace importパターン追加
- `crates/lang-typescript/src/observe.rs` — type-only filter + テスト追加
- `tests/fixtures/typescript/observe/import_namespace.ts` — 新規fixture
- `tests/fixtures/typescript/observe/import_type_only.ts` — 新規fixture

## Design Approach

### #83 Namespace import
import_mapping.scm に3つ目のパターンを追加。`namespace_import` ノード内の `identifier` をキャプチャ。

### #84 Type-only import filtering
tree-sitter queryの否定マッチは限定的なため、Rust側でフィルタ。`import_statement`/`import_specifier`の子ノードに`"type"`キーワードがあればスキップ。

- `import type { X }` → import_statementに"type"子ノード
- `import { type X }` → import_specifierに"type"子ノード

## Test List

| ID | テスト | Status |
|----|--------|--------|
| IM8 | `im8_namespace_import` — `import * as X`が抽出される | GREEN |
| IM9 | `im9_type_only_import_excluded` — `import type { X }`が除外、通常importは残る | GREEN |

## Progress Log

- 2026-03-15: RED — IM8, IM9テスト作成、両方失敗確認
- 2026-03-15: GREEN — import_mapping.scm にnamespace pattern追加、observe.rsにtype-only filter追加。全テスト通過
- 2026-03-15: REVIEW — design-reviewer WARN: IM9にinline type modifier (`import { type X }`) アサーション欠落指摘。修正済み
- 2026-03-15: COMMIT — 全品質チェック通過 (431 tests, clippy 0, fmt clean, BLOCK 0)
