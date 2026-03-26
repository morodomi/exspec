---
feature: route-coverage-status-gap-reasons
cycle: 20260326_1510
phase: RED
complexity: standard
test_count: 6
risk_level: low
codex_session_id: ""
created: 2026-03-26 15:10
updated: 2026-03-26 15:10
---

# Route Coverage Status + gap_reasons in Observe Output

## Scope Definition

### In Scope
- [ ] `crates/core/src/observe_report.rs` -- `ObserveRouteEntry` に `status` + `gap_reasons` フィールド追加、`ObserveSummary` に gap/unmappable カウント追加、`format_terminal`/`format_json` 更新、`format_ai_prompt` 新設
- [ ] `crates/cli/src/main.rs` -- route entry 生成時に `status`/`gap_reasons` を設定するロジック追加
- [ ] `ROADMAP.md` -- closure mapping (route name + URL path) を backlog に追加

### Out of Scope
- 将来の拡張フィールド (assertion density, error test coverage) の実装 (Reason: 今サイクルは status/gap_reasons のみ)
- check コマンドの format_ai_prompt 変更 (Reason: observe 専用として新設)

### Files to Change (target: 10 or less)
- `crates/core/src/observe_report.rs`
- `crates/cli/src/main.rs`
- `ROADMAP.md`

## Environment

### Scope
- Layer: Core + CLI
- Plugin: observe (TypeScript/PHP/Python/Rust)
- Risk: low (PASS)

### Runtime
- Language: Rust (Cargo workspace)

### Dependencies (key packages)
- crates/core: observe_report struct
- crates/cli: observe dispatch + route matching logic

### Risk Interview (BLOCK only)
(N/A -- Risk PASS)

## Context & Dependencies

### Reference Documents
- [CONSTITUTION.md](../../CONSTITUTION.md) - "no runtime" 原則
- [crates/lang-typescript/src/observe.rs] - Route struct + route extraction パターンの参照実装
- [crates/core/src/observe_report.rs] - 既存 ObserveRouteEntry / ObserveSummary 定義

### Dependent Features
- PHP observe route extraction: `crates/lang-php/src/observe.rs` (20260326_1305)
- CLI observe dispatch: `crates/cli/src/main.rs`

### Related Issues/PRs
- (none)

## Test List

### TODO
- [ ] TC-01: Given route with test_files, When report built, Then status="covered" and gap_reasons=[]
- [ ] TC-02: Given route with handler but no test_files, When report built, Then status="gap" and gap_reasons=["no_test_mapping"]
- [ ] TC-03: Given route with empty handler, When report built, Then status="unmappable" and gap_reasons=[]
- [ ] TC-04: Terminal output contains "Routes: X total, Y covered, Z gap, W unmappable"
- [ ] TC-05: JSON output contains status and gap_reasons fields
- [ ] TC-06: AI prompt output lists gap routes with handler info

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal
observe の route coverage 出力に `status`/`gap_reasons` フィールドを追加し、terminal/JSON/ai-prompt でギャップ情報を可視化する。将来の拡張 (assertion density, error test coverage) に備えた構造にする。

### Background
現在の route coverage は `## Route Coverage (33/82)` のテーブルのみ。どの route がギャップか、なぜカバーされていないかが出力から読み取れない。closure による unmappable route が区別されていない。

### Design Approach

#### ObserveRouteEntry 変更

```rust
pub struct ObserveRouteEntry {
    pub http_method: String,
    pub path: String,
    pub handler: String,
    pub file: String,
    pub test_files: Vec<String>,
    pub status: String,           // "covered" | "gap" | "unmappable"
    pub gap_reasons: Vec<String>, // ["no_test_mapping"] etc.
}
```

#### ObserveSummary 変更

```rust
pub struct ObserveSummary {
    // ... existing fields ...
    pub routes_total: usize,
    pub routes_covered: usize,
    pub routes_gap: usize,
    pub routes_unmappable: usize,
}
```

#### CLI での status 設定ロジック

| 条件 | status | gap_reasons |
|------|--------|-------------|
| `test_files` あり | `"covered"` | `[]` |
| `handler` あり + `test_files` なし | `"gap"` | `["no_test_mapping"]` |
| `handler` なし (closure) | `"unmappable"` | `[]` |

#### Terminal 出力追加

現在: `## Route Coverage (33/82)` テーブルのみ
追加: summary 行 `Routes: 82 total, 33 covered, 28 gap, 21 unmappable`

#### AI prompt 出力 (observe 専用)

gap route 一覧を含め、テスト生成を提案するコンテキストとして出力。check の `format_ai_prompt` とは別に observe 用として新設。

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
cargo run -- observe --lang php --format json ~/Documents/NewsService/sr108 | jq '.routes[0]'  # status field present
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-26 15:10 - INIT
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
