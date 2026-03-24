---
feature: observe: Rust L2 import resolution bugs
cycle: 20260324_1530
phase: COMMIT
complexity: trivial
test_count: 8
risk_level: low
codex_session_id: ""
created: 2026-03-24 15:30
updated: 2026-03-24 15:30
---

# #179 observe: Rust L2 import resolution bugs (self:: prefix + single-segment module)

## Scope Definition

### In Scope
- [ ] `parse_use_path`: single-segment module import early return fix
- [ ] `extract_pub_use_re_exports`: strip `self::` prefix from full_text
- [ ] `extract_re_exports_from_text`: strip `self::` prefix from use_path (cfg macro blocks)
- [ ] Unit tests for all 3 fixes (RS-IMP-09, RS-IMP-10, RS-BARREL-SELF-01~03, RS-BARREL-CFG-SELF-01)
- [ ] Integration tests (RS-L2-SELF-BARREL-E2E, RS-L2-SINGLE-SEG-E2E)

### Out of Scope
- Other observe recall improvements beyond these 2 bugs (deferred to future phases)
- L1 matching changes (unrelated)
- `super::` prefix handling (barrel re-exports で実用上使われないため対象外)

### Files to Change (target: 10 or less)
- `crates/lang-rust/src/observe.rs` (edit)
- `crates/lang-rust/tests/observe_tests.rs` (edit — add unit + integration tests)

## Environment

### Scope
- Layer: Backend (Rust)
- Plugin: N/A (cargo test + clippy + fmt + self-dogfood)
- Risk: 15 (PASS)

### Runtime
- Language: Rust (stable)

### Dependencies (key packages)
- tree-sitter: workspace version
- exspec-lang-rust: workspace crate

### Risk Interview (BLOCK only)
(N/A — Risk 15, PASS)

## Context & Dependencies

### Reference Documents
- [ROADMAP.md] - v0.4.5 "Now": P2 Rust observe recall improvement
- [CONSTITUTION.md] - Static AST analysis only, Precision >= 98% / Recall >= 90% ship criteria

### Dependent Features
- Phase 16 (Rust observe L2): `crates/lang-rust/src/observe.rs`

### Related Issues/PRs
- Issue #179: Rust L2 import resolution bugs (self:: prefix + single-segment module)

## Test List

### TODO
(none)

### WIP
- [x] RS-IMP-09: `parse_use_path` single-segment module import — FAIL (expected)
- [x] RS-IMP-10: `parse_use_path` single-segment with crate_name — FAIL (expected)
- [x] RS-BARREL-SELF-01: `extract_barrel_re_exports` strips `self::` from wildcard — FAIL (expected)
- [x] RS-BARREL-SELF-02: `extract_barrel_re_exports` strips `self::` from symbol — FAIL (expected)
- [x] RS-BARREL-SELF-03: `extract_barrel_re_exports` strips `self::` from use list — FAIL (expected)
- [x] RS-BARREL-CFG-SELF-01: `extract_re_exports_from_text` strips `self::` in cfg macro — FAIL (expected)
- [x] RS-L2-SELF-BARREL-E2E: Integration — L2 resolves through self:: barrel — FAIL (expected)
- [x] RS-L2-SINGLE-SEG-E2E: Integration — L2 resolves single-segment module import — FAIL (expected)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal
Rust observe L2 recall is 38.2% (104/272 test files in tokio). Two bugs cause 90/110 FN in `tokio/tests/`. Fix both to raise recall to ~71%.

### Background
Bug A: `parse_use_path("fs")` silently drops single-segment paths because the final branch requires `parts.len() >= 2`. `use tokio::fs` hits this after crate prefix stripping.

Bug B: `pub use self::file::File` produces from_specifier `"./self/file"` instead of `"./file"` because `self::` is not stripped before path splitting. Affects both `extract_pub_use_re_exports` (tree-sitter AST path) and `extract_re_exports_from_text` (cfg macro text path).

### Design Approach
- Bug A fix: Add early return in `parse_use_path` before the `parts.len() >= 2` check — if path contains no `::` and is non-empty, register it as a module import with empty symbols.
- Bug B fix: Strip `self::` prefix at the start of `extract_pub_use_re_exports` and `extract_re_exports_from_text` use_path handling, using `strip_prefix("self::").unwrap_or(full_text)`.

## Verification

```bash
cargo test -p exspec-lang-rust
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-24 15:30 - INIT
- Cycle doc created
- Scope definition ready

### 2026-03-24 15:35 - PLAN-REVIEW
- Design review: WARN (35/100)
- Bug A/B のコードパス独立性確認済み
- `super::` prefix を Out of Scope に明記
- Phase completed

### 2026-03-24 - RED
- 8テスト追加: `crates/lang-rust/src/observe.rs` の `#[cfg(test)] mod tests` ブロック末尾
- 実行結果: 154 passed; 8 failed (全て期待通りの失敗)
- self-dogfooding: BLOCK 0件
- RED state verified

### 2026-03-24 - GREEN
- 3箇所修正: parse_use_path (single-seg), extract_pub_use_re_exports (self::), extract_re_exports_from_text (self::)
- 実行結果: 162 passed; 0 failed
- Phase completed

### 2026-03-24 - REFACTOR
- チェックリスト7項目全て確認: 改善不要
- Verification Gate: PASS (162 passed, clippy 0, fmt clean, BLOCK 0)
- Phase completed

### 2026-03-24 - REVIEW
- Security: PASS (3/100)
- Correctness: WARN (35/100) -> 2件修正: `pub use self::*` handling, brace trimming safety
- Lint-as-Code: PASS (162 passed, clippy 0, fmt clean, BLOCK 0)
- Phase completed

---

## Next Steps

1. [Done] INIT
2. [Done] PLAN-REVIEW
3. [Done] RED
4. [Done] GREEN
5. [Done] REFACTOR
6. [Done] REVIEW
7. [Next] COMMIT
