---
feature: php-facade-should-receive
cycle: 20260311_1048
phase: DONE
complexity: trivial
test_count: 4
risk_level: low
created: 2026-03-11 10:48
updated: 2026-03-11 10:48
---

# #58: T001 FP: PHP Facade::shouldReceive() static Mockery calls

## Scope Definition

### In Scope
- [ ] assertion.scm に scoped_call_expression パターン3種追加
- [ ] fixture ファイル作成 (t001_pass_facade_mockery.php)
- [ ] unit tests 3件 + integration test 1件

### Out of Scope
- helper delegation パターン (custom_patterns territory)
- 他言語への影響

### Files to Change (target: 10 or less)
- crates/lang-php/queries/assertion.scm (edit)
- tests/fixtures/php/t001_pass_facade_mockery.php (new)
- crates/lang-php/src/lib.rs (edit)

## Environment

### Scope
- Layer: Backend
- Plugin: Rust (tree-sitter query)
- Risk: 5 (PASS)

### Runtime
- Language: Rust

### Dependencies (key packages)
- tree-sitter-php: existing

### Risk Interview (BLOCK only)
N/A (PASS)

## Context & Dependencies

### Reference Documents
- docs/dogfooding-results.md - Laravel dogfooding結果
- crates/lang-php/queries/assertion.scm - 既存Mockeryパターン

### Dependent Features
- #38: T001 FP: PHP Mockery (member_expression版、実装済み)

### Related Issues/PRs
- Issue #58: T001 FP: PHP Facade::shouldReceive() static Mockery calls

## Test List

### TODO
(none)

### WIP
(none)

### DISCOVERED
(none)

### DONE
- [x] TC-01: Given `Log::shouldReceive('error')->once()` When extracted Then assertion_count >= 1
- [x] TC-02: Given `Log::shouldHaveReceived('info')` When extracted Then assertion_count >= 1
- [x] TC-03: Given `Log::shouldNotHaveReceived('debug')` When extracted Then assertion_count >= 1
- [x] TC-04: Given facade fixture全体 When T001評価 Then BLOCK 0件

## Implementation Notes

### Goal
Laravel Facadeの静的Mockery呼び出し (`Log::shouldReceive()` 等) をassertion として認識し、T001 FPを解消する。

### Background
Laravel FacadeはPHPの静的メソッド呼び出し構文を使う。tree-sitterではこれが `scoped_call_expression` としてパースされる。既存のMockeryパターンは `member_call_expression` のみ対応しており、静的呼び出しにマッチしない。実プロジェクトで5 BLOCK FP確認済み。

### Design Approach
既存の `member_call_expression` パターン (#38) の横展開。`scoped_call_expression` ノードに対して同じ3メソッド (shouldReceive, shouldHaveReceived, shouldNotHaveReceived) をマッチさせる。scope制約なし (name/relative_scope/qualified_name全てにマッチ)。

## Progress Log

### 2026-03-11 10:48 - KICKOFF
- Cycle doc created
- Scope definition ready

### 2026-03-11 10:49 - RED
- Test code created, 4 tests failing
- TC-01~03: assertion_count == 0 (expected >= 1)
- TC-04: 4 T001 BLOCKs (expected 0)
- Phase completed

### 2026-03-11 10:50 - GREEN
- assertion.scm に scoped_call_expression パターン3種追加
- 617テスト全通過 (4新規テスト含む)
- Phase completed

### 2026-03-11 10:51 - REFACTOR
- simplify: 3-agent review (reuse/quality/efficiency) - no changes needed
- Verification Gate: tests 617 PASS, clippy 0, fmt OK, self-dogfooding BLOCK 0 (real)
- Phase completed

### 2026-03-11 10:52 - REVIEW
- review(code) score:0 verdict:PASS
- Security: PASS (no issues, scope-agnostic pattern intentional)
- Correctness: PASS (patterns correct, captures unique, fixture realistic)
- Phase completed

### 2026-03-11 10:53 - COMMIT
- Committed: f79e0ed
- Phase completed

---

## Next Steps

1. [Done] KICKOFF
2. [Done] RED
3. [Done] GREEN
4. [Done] REFACTOR
5. [Done] REVIEW
6. [Done] COMMIT <- Complete
