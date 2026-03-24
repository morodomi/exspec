---
feature: observe: Rust file_exports_any_symbol cfg macro fallback + multi-line pub use
cycle: 20260324_2350
phase: DONE
complexity: trivial
test_count: 7
risk_level: low
codex_session_id: ""
created: 2026-03-24 23:50
updated: 2026-03-24 23:50
---

# #181 observe: Rust file_exports_any_symbol cfg macro fallback + multi-line pub use

## Scope Definition

### In Scope
- [ ] `file_exports_any_symbol`: text fallback for pub items inside cfg macro token_tree
- [ ] `join_multiline_pub_use`: helper function to join multi-line pub use blocks
- [ ] `extract_re_exports_from_text`: use `join_multiline_pub_use` before line-by-line loop
- [ ] Unit tests (RS-EXPORT-CFG-01~03, RS-MULTILINE-USE-01~02)
- [ ] Integration tests (RS-L2-CFG-EXPORT-E2E, RS-L2-CFG-MULTILINE-E2E)

### Out of Scope
- `pub(crate)` visibility handling (not exported as public API)
- `super::` / `self::` prefix handling (separate issue)
- Multi-level nested cfg macro resolution (deferred)
- Other observe recall improvements beyond these 2 function changes

### Files to Change (target: 10 or less)
- `crates/lang-rust/src/observe.rs` (edit)
- `crates/lang-rust/tests/observe_tests.rs` (edit — add unit + integration tests)

## Environment

### Scope
- Layer: Backend (Rust)
- Plugin: N/A (cargo test + clippy + fmt + self-dogfood)
- Risk: 20 (PASS)

### Runtime
- Language: Rust (stable)

### Dependencies (key packages)
- tree-sitter: workspace version
- exspec-lang-rust: workspace crate

### Risk Interview (BLOCK only)
(N/A — Risk 20, PASS)

## Context & Dependencies

### Reference Documents
- [ROADMAP.md] - v0.4.5 "Now": P1 Rust cfg macro multi-hop barrel resolution
- [CONSTITUTION.md] - Static AST analysis only, Precision >= 98% / Recall >= 90% ship criteria

### Dependent Features
- Phase 16 (Rust observe L2): `crates/lang-rust/src/observe.rs`
- Issue #178: pub use/pub mod extraction from cfg macro blocks (precedent for text fallback approach)

### Related Issues/PRs
- Issue #181: Rust file_exports_any_symbol misses pub items inside cfg macros

## Test List

### TODO
- [ ] RS-EXPORT-CFG-01: `file_exports_any_symbol` finds pub struct inside cfg macro
- [ ] RS-EXPORT-CFG-02: `file_exports_any_symbol` returns false for non-exported symbol
- [ ] RS-EXPORT-CFG-03: `file_exports_any_symbol` does not match pub(crate)
- [ ] RS-MULTILINE-USE-01: `join_multiline_pub_use` joins multi-line pub use
- [ ] RS-MULTILINE-USE-02: `extract_re_exports_from_text` parses joined multi-line pub use
- [ ] RS-L2-CFG-EXPORT-E2E: L2 resolves through cfg-wrapped production file
- [ ] RS-L2-CFG-MULTILINE-E2E: L2 resolves through multi-line cfg pub use

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal
Post-#179, Rust observe recall = 62.9% (171/272 in tokio). Two remaining FN buckets: (1) 43 test files import symbols defined inside cfg macros (e.g., `cfg_net! { pub struct TcpListener { ... } }`) — tree-sitter parses cfg macro bodies as opaque `token_tree`, so `file_exports_any_symbol()` returns `false` and L2 filters them out. (2) ~8 more FN from multi-line `pub use util::{\n  ...\n};` in cfg blocks not parsed by the line-by-line loop in `extract_re_exports_from_text`.

### Background
- `file_exports_any_symbol` uses tree-sitter query to find `pub` items at top level. Items inside cfg macro bodies appear as `token_tree` nodes which are opaque — the query never matches.
- `extract_re_exports_from_text` processes text line-by-line. Multi-line `pub use module::{...};` spanning multiple lines is not handled.

### Design Approach
- **Change 1**: After the tree-sitter query loop returns `false`, add text-based fallback. For each requested symbol, check `"pub {keyword} {symbol}"` patterns (struct/fn/type/enum/trait/const/static). Intentionally simple — triggered only when tree-sitter query fails, symbol name must match exactly, and `pub(crate)` won't match.
- **Change 2**: Add `join_multiline_pub_use(text)` helper that accumulates lines between `pub use module::{` (no closing `}`) and the closing `};`. Call it before the existing line-by-line loop in `extract_re_exports_from_text`.

## Verification

```bash
cargo test -p exspec-lang-rust
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
```

Dogfooding target: tokio observe recall >= 75%

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-24 23:50 - INIT
- Cycle doc created
- Plan: /Users/morodomi/.claude/plans/humble-twirling-rabin.md
- Scope: 2 function changes in observe.rs + 7 tests

### 2026-03-25 - RED
- 7 tests added: 5 FAILED (expected), 2 PASSED (negative cases)
- Phase completed

### 2026-03-25 - GREEN
- 3 fixes: file_exports_any_symbol text fallback, join_multiline_pub_use, extract_re_exports_from_text wiring
- Additional: extract_single_re_export_stmt split for multi-statement lines
- 169 passed; 0 failed
- Phase completed

### 2026-03-25 - REFACTOR
- Verification Gate: PASS (169 passed, clippy 0, fmt clean, BLOCK 0)
- Phase completed

### 2026-03-25 - REVIEW
- Security: PASS (8/100)
- Correctness: PASS (42/100) -> 3件修正: comment-skip in fallback, brace depth counter, dead code removal
- Phase completed
