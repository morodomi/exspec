---
feature: lang-rust
cycle: phase5a-rust-support
phase: DONE
created: 2026-03-07 00:37
updated: 2026-03-07 00:37
---

# Phase 5A: Rust Language Support

## Scope Definition

### In Scope
- [ ] Step 1: Scaffolding + 依存関係 (crates/lang-rust/, Cargo.toml)
- [ ] Step 2: テスト関数抽出 (#[test], #[tokio::test] 等)
- [ ] Step 3: アサーション検出 (assert!, assert_eq!, assert_ne!)
- [ ] Step 4: モック検出 (mockall patterns)
- [ ] Step 5: 巨大テスト検出 (T003 line count)
- [ ] Step 6: ファイルレベルルール (parameterized, PBT, contract)
- [ ] Step 7: インラインサプレッション
- [ ] Step 8: CLI統合 (Language::Rust, discover, SUPPORTED_LANGUAGES)
- [ ] Step 9: E2Eテスト
- [ ] Step 10: ドキュメント更新 (STATUS.md, SPEC.md)

### Out of Scope
- #[cfg(test)] mod tests {} インラインテスト検出 (Reason: アーキテクチャ変更が大きい、v0.2で対応)
- Tier 2 ルール (Reason: Phase 5B)

### Files to Change (target: 10 or less)
- Cargo.toml (root) - edit: workspace members追加
- crates/lang-rust/Cargo.toml - new
- crates/lang-rust/src/lib.rs - new: RustExtractor + unit tests
- crates/lang-rust/queries/*.scm - new: 7 query files
- crates/cli/src/main.rs - edit: Language::Rust + CLI統合
- tests/fixtures/rust/*.rs - new: ~12 fixture files
- docs/STATUS.md - edit
- docs/SPEC.md - edit

## Environment

### Scope
- Layer: Backend (CLI tool)
- Plugin: rust (Rust analyzer)
- Risk: 35 (PASS)

### Runtime
- Language: Rust (stable)

### Dependencies (key packages)
- tree-sitter: 0.24
- tree-sitter-rust: 0.23 (ABI 14 - must NOT use 0.24 which is ABI 15)
- streaming-iterator: 0.1
- exspec-core: workspace

### Risk Interview (BLOCK only)
N/A (PASS)

## Context & Dependencies

### Reference Documents
- docs/SPEC.md - ルール仕様
- docs/STATUS.md - 言語別対応状況
- crates/lang-php/src/lib.rs - 直近の言語追加パターン参照

### Dependent Features
- LanguageExtractor trait: crates/core/src/extractor.rs
- Rule evaluation: crates/core/src/rules.rs
- CLI discovery: crates/cli/src/main.rs

### Related Issues/PRs
- (none)

## Test List

### TODO

#### Step 2: テスト関数抽出
- [ ] TC-01: #[test] attribute付き関数を正しく抽出する
- [ ] TC-02: #[test] なし関数はテスト関数として抽出しない
- [ ] TC-03: #[tokio::test] 等の scoped_identifier 形式を抽出する
- [ ] TC-04: assertion-free テスト関数を T001 違反として検出する (t001_violation.rs)
- [ ] TC-05: assertion付きテスト関数を T001 pass とする (t001_pass.rs)

#### Step 3: アサーション検出
- [ ] TC-06: assert!, assert_eq!, assert_ne! をカウントする
- [ ] TC-07: debug_assert! もカウントする

#### Step 4: モック検出
- [ ] TC-08: MockXxx::new() パターンを検出する
- [ ] TC-09: mock数 > 閾値 で T002 違反 (t002_violation.rs)
- [ ] TC-10: mock数 <= 閾値 で T002 pass (t002_pass.rs)
- [ ] TC-11: モッククラス名を正しく抽出する (MockXxx -> "Xxx")

#### Step 5: 巨大テスト
- [ ] TC-12: 50行超テスト関数を T003 違反 (t003_violation.rs)
- [ ] TC-13: 50行以下テスト関数を T003 pass (t003_pass.rs)

#### Step 6: ファイルレベルルール
- [ ] TC-14: #[rstest] をパラメタライズドとして検出 (T004)
- [ ] TC-15: パラメタライズド比率が閾値未満で T004 違反 (t004_violation.rs)
- [ ] TC-16: use proptest / use quickcheck を PBT として検出 (T005)
- [ ] TC-17: PBT import なしで T005 違反 (t005_violation.rs)
- [ ] TC-18: T008 は Rust で常に INFO (contract なし) (t008_violation.rs)

#### Step 7: インラインサプレッション
- [ ] TC-19: // exspec-ignore: T001 でサプレッション動作 (suppressed.rs)

#### Step 8: CLI統合
- [ ] TC-20: tests/**/*.rs をRustテストファイルとして検出
- [ ] TC-21: *_test.rs をRustテストファイルとして検出
- [ ] TC-22: src/*.rs をテストファイルとして検出しない
- [ ] TC-23: --lang rust で Rust のみ解析
- [ ] TC-24: SUPPORTED_LANGUAGES に "rust" が含まれる

#### Step 9: E2Eテスト
- [ ] TC-25: T001-T003 pass/violation E2E
- [ ] TC-26: T004-T006, T008 pass/violation E2E
- [ ] TC-27: suppression E2E
- [ ] TC-28: discover_files の Rust フィルタ

### WIP
(none)

### DISCOVERED
- `is_rust_test_file()` の `path.contains("/tests/")` がディレクトリ境界を考慮しない (`tests_data/` 等が誤マッチ)
- `proptest!` マクロ内テスト関数の T001 誤検知リスク (現テストでは顕在化しない)
- cross-extractor code dedup: `count_captures`, `has_any_match`, `extract_suppression_from_previous_line` 等が4言語で重複

### DONE
(none)

## Implementation Notes

### Goal
exspecにRust (cargo test) の言語サポートを追加する。既存のPython/TypeScript/PHPと同じパターンで、tree-sitter-rustによる静的解析を実装する。

### Background
- Phase 4で3言語対応を完了 (269テスト)
- ロードマップ Phase 5 で Rust 対応が予定されている
- 既存の lang-php パターンに倣って実装

### Design Approach

#### テスト関数検出 (plan-review critical #1 反映)
- tree-sitter-rust では `attribute_item` と `function_item` は **兄弟ノード**
- クエリで `attribute_item` を捕捉 → Rust実装側で `next_sibling()` で `function_item` を特定
- PHPの `detect_docblock_test_methods()` と同種の2段階検出

#### scoped_identifier 対応 (plan-review important #3 反映)
- `#[tokio::test]`, `#[async_std::test]` 等は `scoped_identifier` ノード
- クエリに scoped_identifier パターン追加: `name: (identifier) @_attr (#eq? @_attr "test")`

#### Rust固有マッピング
| exspec概念 | Rust対応 |
|-----------|---------|
| テスト関数 | #[test] / #[tokio::test] attribute付き fn |
| アサーション | assert!, assert_eq!, assert_ne!, debug_assert! |
| モック | mockall: MockXxx::new() |
| パラメタライズド | #[rstest] attribute |
| PBT | use proptest / use quickcheck |
| コントラクト | N/A (T008常にINFO) |

#### ABI互換性 (plan-review critical #2 反映)
- tree-sitter-rust 0.23 (ABI 14) を使用。0.24 は ABI 15 で非互換。

## Progress Log

### 2026-03-07 00:37 - KICKOFF
- Cycle doc created from plan
- Plan review PASS (score: 35)
- Critical findings incorporated: AST sibling structure, ABI version, scoped_identifier

### Phase: KICKOFF - Completed at 00:37
**Artifacts**: Cycle doc with PLAN section, Test List (28 items TC-01~TC-28)
**Decisions**: architecture=follow lang-php pattern, test strategy=step-by-step TDD (10 steps)
**Next Phase Input**: Test List items TC-01 ~ TC-28, plan-review critical findings
**Review**: plan-review PASS (score: 35), design-reviewer blocking_score=35

### 2026-03-07 - RED
- 20 new failing tests (14 lang-rust unit + 6 CLI E2E)
- 268 existing tests unaffected
- Files: lang-rust crate + queries + fixtures + CLI integration

### 2026-03-07 - GREEN
- All 309 tests passing (40 new: 21 lang-rust + 19 CLI)
- RustExtractor: attribute_item → next_sibling() 2-stage detection

### 2026-03-07 - REFACTOR
- /simplify: 3 fixes (pub(crate)→fn, --lang help text, block_comment skip)
- Verification Gate: 309 tests PASS, clippy 0 errors, fmt clean
- Discovered: cross-extractor code dedup opportunity (separate cycle)

---

## Next Steps

1. [Done] KICKOFF
2. [Done] PLAN (plan-review PASS)
3. [Done] RED
4. [Done] GREEN
5. [Done] REFACTOR
6. [Done] REVIEW (PASS, score: 42)
7. [Done] COMMIT
