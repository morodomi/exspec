---
feature: fan-out filter name-match exemption 改善
phase: DONE
complexity: Medium
test_count: 8
risk_level: Medium (30)
created: 2026-03-25T16:51:00+09:00
updated: 2026-03-25T17:03:00+09:00
---

# fan-out filter name-match exemption 改善

## Context

fan-out filter が PHP/Rust の正しいマッピングを大量に除去している:

- PHP (Laravel): R=81.5% (fan-out on) → 88.5% (off)、+63 files blocked
- Rust (clap): R=23.1% (fan-out on) → 90.3% (off)、大量 blocked

現在の name-match は `test_stem.contains(&prod_class)` (forward) のみで、
- `AuthGuardTest` contains `AuthenticationException`? → NO → 除去 (FN)
- `DatabaseEloquentIntegrationTest` contains `Model`? → NO → 除去 (FN)

これらは L2 import tracing で正しくマッピングされたのに、fan-out filter で除去される。

## TDD Context

- **Feature**: fan-out filter name-match exemption 改善
- **Complexity**: Medium (CLI の filter ロジック変更)
- **Risk**: Medium (30) — filter 変更は P/R 両方に影響。FP 増加リスク
- **Language/Framework**: Rust

## Design Approach

### 改善点 1: Forward fan-out — bidirectional + directory match

現在 (single-direction):
```
test_stem.contains(&prod_class)  // "authguardtest" contains "authenticationexception"? NO
```

改善 (bidirectional + directory):
```
test_stem.contains(&prod_class) || prod_class.contains(&test_stem)
// OR: directory segment match
```

### 改善点 2: Directory segment match

```
fn has_common_directory_segment(test_path, prod_path) -> bool
```

tests/Auth/AuthGuardTest.php と src/Illuminate/Auth/Guard.php → "auth" 共通 → KEEP

generic segments (src, tests, etc.) は除外する。

### 改善点 3: Reverse fan-out — directory match 追加

high fan-in test の retain 条件に directory segment match を追加。

## Key Files

- `crates/cli/src/main.rs` — apply_fan_out_filter, apply_reverse_fan_out_filter
- `crates/core/src/config.rs` — ObserveConfig

## Test List

1. **Given** prod fan-out > threshold + test dir matches prod dir, **When** forward filter, **Then** test KEPT
2. **Given** prod fan-out > threshold + test dir does NOT match, **When** forward filter, **Then** test REMOVED
3. **Given** prod fan-out > threshold + prod_stem contains test_stem, **When** forward filter, **Then** test KEPT (bidirectional)
4. **Given** test maps to >5 prods + test dir matches prod dir, **When** reverse filter, **Then** mapping KEPT
5. **Given** generic segments only (src, tests), **When** directory match, **Then** NOT matched
6. **Given** Laravel observe after fix, **When** measure recall, **Then** R > 85%
7. **Given** clap observe after fix, **When** measure recall, **Then** R > 30%
8. **Given** tokio observe after fix, **When** measure recall, **Then** R >= 50.8% (no regression)

### WARN: Design Review Gate 指摘事項

**W1: recall 目標と ship criteria の乖離**
テスト 6 の目標 R > 85% は CONSTITUTION ship criteria (R>=90%) を下回る。
本 cycle は改善の中間ステップとして許容するが、PHP の最終リリース前に R>=90% を達成すること。

**W2: short stem FP ケース未テスト**
`prod_class.contains(&test_stem)` の bidirectional match において、
`Guard`, `Auth`, `Manager` など汎用クラス名が他 prod ファイル名にマッチして FP を生じるケースのテストが不足。
実装時にテストケースを追加すること（例: test_stem="auth" が prod_class="authentication" に誤マッチ）。

## Verification

1. `cargo test` — 全テスト PASS
2. `cargo clippy -- -D warnings` — 0 errors
3. `cargo run -- observe --lang php --format json /tmp/laravel` — PHP recall
4. `cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/clap/` — Rust recall
5. `cargo run -- observe --lang rust --format json /tmp/exspec-dogfood/tokio/` — tokio regression

## Progress Log

### 2026-03-25T16:51:00+09:00 — sync-plan

Plan from `/Users/morodomi/.claude/plans/fancy-roaming-harp.md` を Cycle doc に同期。

Design Review Gate: WARN (score=50)
- 観点1 CONSTITUTION整合性: 10pt (recall 目標が ship criteria 未達)
- 観点2 スコープ妥当性: 5pt (2ファイル、YAGNI違反なし)
- 観点3 リスク評価: 20pt (FP増加リスクあり、short stem FP 未テスト)
- 観点4 Test List品質: 15pt (正常系/境界値/異常系網羅、regression guardあり)

警告付きで Cycle doc 生成。W1・W2 は RED phase 開始前に対処すること。

### 2026-03-25T17:03:00+09:00 — RED phase 完了

8件のテストを `crates/cli/src/main.rs` の末尾テストモジュールに追加。

RED 確認結果:
- FO-01 FAILED: forward filter で dir segment match 未実装
- FO-03 FAILED: bidirectional match (prod_class.contains(test_stem)) 未実装
- FO-04 FAILED: reverse filter で dir segment match 未実装
- FO-02 PASSED: guard test (no dir match → REMOVE は既存動作で正しい)
- FO-05 PASSED: guard test (generic segments → no match は既存動作で正しい)
- FO-W2 PASSED: guard test (short stem "io" は FP になるので除去 = 既存動作で正しい)
- FO-INT-01 IGNORED: integration test (#[ignore])
- FO-INT-02 IGNORED: integration test (#[ignore])

既存 147 tests: 全て PASS。BLOCK 0件 (self-dogfooding)。
clippy: 0 errors。fmt: 差分なし。
