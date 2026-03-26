---
feature: t003-language-specific-thresholds
cycle: 20260326_1047
phase: RED (tests written)
complexity: standard
test_count: 6
risk_level: low
codex_session_id: ""
created: 2026-03-26 10:47
updated: 2026-03-26 10:47
---

# T003 言語別デフォルトしきい値 (language-specific default thresholds for giant-test rule)

## Scope Definition

### In Scope
- [ ] `crates/core/src/rules.rs` に `test_max_lines_is_explicit: bool` フィールド追加
- [ ] `crates/core/src/rules.rs` に `fn default_test_max_lines(file: &str) -> usize` 追加
- [ ] `crates/core/src/rules.rs` の T003 評価ロジック修正 (言語別しきい値を使用)
- [ ] `crates/core/src/config.rs` の `From<ExspecConfig> for Config` に `test_max_lines_is_explicit` 設定追加
- [ ] `docs/SPEC.md` — T003 セクションに言語別デフォルト表を追加
- [ ] `docs/configuration.md` — `test_max_lines` の説明を更新

### Out of Scope
- ルールのセマンティクス変更 (T003 自体の判定ロジックは維持)
- 他ルールのしきい値変更
- 言語別デフォルトの UI / CLI での表示改善 (別サイクル)

### Files to Change (target: 10 or less)
- `crates/core/src/rules.rs` (edit) — フィールド追加・関数追加・T003 評価ロジック修正
- `crates/core/src/config.rs` (edit) — `test_max_lines_is_explicit` 設定
- `docs/SPEC.md` (edit) — T003 セクション更新
- `docs/configuration.md` (edit) — `test_max_lines` 説明更新

## Environment

### Scope
- Layer: core (rules / config)
- Plugin: all (Python / TypeScript / PHP / Rust)
- Risk: low (PASS)

### Runtime
- Language: Rust (edition 2021)

### Dependencies (key packages)
- tree-sitter: workspace (変更なし)
- crates/core: rules.rs, config.rs

### Risk Interview (BLOCK only)
N/A — Risk low (PASS)

## Context & Dependencies

### Reference Documents
- [CONSTITUTION.md] — 原則・存在意義
- [ROADMAP.md] — T003 FP 低減方針
- [docs/configuration.md] — `test_max_lines` 設定の現状説明
- [docs/SPEC.md] — T003 ルール仕様

### Dependent Features
- 内部dogfooding: PHP/Laravel で T003 WARN 211件 (test_max_lines=50 超過)
- 業界標準: PHPMD=100, clippy=100 (code lines only)

### Related Issues/PRs
- 内部dogfooding結果: PHP Recall 向上後に T003 WARN 211件が顕在化

## Test List

### TODO
- [x] TC-01: Given `.php` file with test function, When default_test_max_lines called, Then returns 100
- [x] TC-02: Given `.rs` file with test function, When default_test_max_lines called, Then returns 100
- [x] TC-03: Given `.ts` / `.tsx` file with test function, When default_test_max_lines called, Then returns 75
- [x] TC-04: Given `.py` file with test function, When default_test_max_lines called, Then returns 50
- [x] TC-05: Given `.exspec.toml` with explicit `test_max_lines = 60`, When T003 evaluated for PHP file with 80-line test, Then WARN triggered (explicit config overrides language default)
- [x] TC-06: Given no explicit `test_max_lines` in config, When T003 evaluated for PHP file with 80-line test, Then no WARN (80 < 100 = PHP default)

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal

T003 (`giant-test`) の `test_max_lines` デフォルト値を言語別に設定し、PHP/Laravel の構造的に長い Feature Test に対する FP を低減する。`.exspec.toml` で明示的に設定した場合は全言語でその値を優先する (既存動作を維持)。

### Background

内部dogfoodingで PHP/Laravel プロジェクトに対して T003 (giant-test) が 211件 WARN を出した。全てが `test_max_lines=50` 超過。調査の結果、PHP の Feature Test は Arrange (factory/mock) セクションが構造的に長く、50行超の多くは Compositional 違反ではなく FP。業界標準 (PHPMD=100, clippy=100 code lines) と比較しても exspec の 50 は言語によって厳しすぎる。

| 言語 | 現在 | 変更後 | 根拠 |
|------|------|--------|------|
| Python | 50 | 50 | pylint 50 statements と同等 |
| TypeScript | 50 | 75 | ESLint 50 だが Feature Test の構造考慮 |
| PHP | 50 | 100 | PHPMD デフォルトと一致 |
| Rust | 50 | 100 | clippy 100 (code lines only, exspec は全行なので同等以上) |

### Design Approach

`Config` を言語非依存のまま維持し、T003 評価時に `func.file` 拡張子から言語を判定して言語別デフォルトを適用する。

1. **`Config` に `test_max_lines_is_explicit: bool` を追加** — ユーザーが `.exspec.toml` で明示的に設定したかを記録
2. **`rules.rs` に `fn default_test_max_lines(file: &str) -> usize` を追加** — 拡張子から言語別デフォルトを返す
3. **T003 評価ロジック**: `config.test_max_lines_is_explicit` なら `config.test_max_lines` を使用、そうでなければ `default_test_max_lines(func.file)` を使用
4. **`config.rs`**: `From<ExspecConfig> for Config` で `test_max_lines_is_explicit: ec.thresholds.test_max_lines.is_some()`
5. **メッセージ**: 実際に使ったしきい値を WARN メッセージに表示

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
# PHP dogfood validation (manual)
cargo run -- --lang php --format json ~/Documents/Netman/clair-gpt  # WARN ~39 (was 211)
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-26 10:47 - INIT
### 2026-03-26 - RED

- Created 6 tests (TC-01 to TC-06) in `crates/core/src/rules.rs`
- Added `make_func_with_file` helper for file-specific test functions
- RED state verified: `cargo test` fails with 7 compile errors (missing `default_test_max_lines` fn and `test_max_lines_is_explicit` field)
- Self-dogfooding: BLOCK 0 confirmed

- Cycle doc created from plan file `indexed-moseying-bachman.md`
- Scope definition ready

---

## Next Steps

1. [Done] INIT
2. [Done] PLAN
3. [ ] RED
4. [ ] GREEN
5. [ ] REFACTOR
6. [ ] REVIEW
7. [ ] COMMIT
