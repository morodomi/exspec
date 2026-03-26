---
feature: closure-route-url-path-matching
cycle: 20260326_1540
phase: RED
complexity: standard
test_count: 4
risk_level: low
codex_session_id: ""
created: 2026-03-26 15:40
updated: 2026-03-26 15:40
---

# Closure route coverage via URL path matching

## Scope Definition

### In Scope
- [ ] テストファイルから HTTP client URL を tree-sitter で抽出 (PHP / Python / TS)
- [ ] Route path と照合: 静的パスは完全一致、動的パス (`{id}`) は正規表現変換して照合
- [ ] 既存の file-based / class-based match に URL path-based match を追加
- [ ] `route_path_to_regex` ヘルパー: `{param}` → `[^/]+` 変換
- [ ] `has_url_match` ヘルパー: テストソース内の HTTP client 呼び出し URL を照合
- [ ] Unit テスト + Integration テスト

### Out of Scope
- 認証ヘッダや query string の照合 (Reason: スコープ外、パスのみで十分)
- TS `fetch('/path')` 以外の複雑なパターン (Reason: 初回実装は基本パターンのみ)

### Files to Change (target: 10 or less)
- `crates/cli/src/main.rs` (edit) -- URL path-based route coverage match 追加
- テストファイル (edit/new) -- TC-01〜TC-04 の unit / integration テスト

## Environment

### Scope
- Layer: Backend
- Plugin: rust
- Risk: 10 (PASS)

### Runtime
- Language: Rust (edition 2021)

### Dependencies (key packages)
- tree-sitter: workspace
- regex: workspace

### Risk Interview (BLOCK only)
(該当なし)

## Context & Dependencies

### Reference Documents
- [docs/cycles/20260326_1305_php-laravel-route-extraction.md] - Laravel route 抽出の実装参照
- [docs/cycles/20260326_1510_route-coverage-status-gap-reasons.md] - route coverage status (covered/gap/unmappable) の仕様

### Dependent Features
- Route extraction (#211): `crates/cli/src/main.rs` L400-440

### Related Issues/PRs
- Issue #211: route coverage に status (covered/gap/unmappable) を追加

## Test List

### TODO
- [ ] TC-01: Given closure route `GET /csrf-token` and test with `$this->get('/csrf-token')`, When coverage mapped, Then route status = "covered"
- [ ] TC-02: Given closure route `GET /users/{id}` and test with `$this->get('/users/1')`, When coverage mapped, Then route status = "covered" (dynamic path)
- [ ] TC-03: Given closure route `GET /about` and NO test hitting that path, When coverage mapped, Then route status = "unmappable" (unchanged)
- [ ] TC-04: Given Flask route `POST /api/auth/verify` and test with `client.post('/api/auth/verify')`, When coverage mapped, Then route status = "covered"

### WIP
(none)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal
closure route (ルーティングにクロージャを使う Laravel / Flask の route) のカバレッジ判定を改善する。テスト内の HTTP client 呼び出し URL と route path を照合することで、これまで "unmappable" だった closure route を "covered" と判定できるようにする。

### Background
#211 で route coverage に status (covered/gap/unmappable) を追加。sr108 で 21 closure routes が "unmappable"。しかし実際にはテストファイルが `$this->get('/csrf-token')` のような直接 URL パスでテストしている。テスト内の HTTP client 呼び出しから URL を抽出し、route path と照合すれば closure route もカバレッジ判定可能。

実データ:
- sr108: `route()` helper 513件、直接 URL path 10件以上
- nagano-toyota: `client.post('/api/auth/verify')` パターン
- u-group: 同上

### Design Approach
1. テストファイルから HTTP client URL を tree-sitter で抽出:
   - PHP: `$this->get('/path')`, `$this->post('/path')`, `->getJson('/path')`, `->postJson('/path')`
   - Python: `client.get('/path')`, `client.post('/path')`
   - TS: `request(app).get('/path')`, `fetch('/path')` (supertest パターン)

2. Route path と照合: 静的パス (`/csrf-token`) は完全一致。動的パス (`/users/{id}`) は正規表現変換 (`{id}` → `[^/]+`) して照合。

3. coverage mapping に統合: 既存の file-based + class-based match に加えて、URL path-based match を追加。

実装場所: CLI の route coverage 照合ロジック (`crates/cli/src/main.rs` L400-440)

```rust
// URL path-based match (closure routes)
if entry.test_files.is_empty() {
    let route_regex = route_path_to_regex(&entry.path);
    for (test_file, source) in &test_sources {
        if has_url_match(source, &route_regex, &entry.http_method) {
            entry.test_files.push(test_file.clone());
        }
    }
}
```

ヘルパー関数:
- `fn route_path_to_regex(path: &str) -> Regex` -- `{param}` → `[^/]+` に変換
- `fn has_url_match(source: &str, regex: &Regex, method: &str) -> bool` -- テストソース内で HTTP client 呼び出しの URL が regex にマッチするか

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
cargo run -- observe --lang php --format json ~/Documents/NewsService/sr108 | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'unmappable: {d[\"summary\"][\"routes_unmappable\"]}')"
# unmappable should decrease from 21
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-26 15:40 - INIT
- Cycle doc created
- Scope definition ready

---

## Next Steps

1. [Done] INIT <- Current
2. [Next] RED
3. [ ] GREEN
4. [ ] REFACTOR
5. [ ] REVIEW
6. [ ] COMMIT
