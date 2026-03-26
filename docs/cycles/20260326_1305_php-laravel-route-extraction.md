---
feature: php-laravel-route-extraction
cycle: 20260326_1305
phase: RED-DONE
complexity: standard
test_count: 6
risk_level: low
codex_session_id: ""
created: 2026-03-26 13:05
updated: 2026-03-26 13:05
---

# PHP Laravel Route Extraction for Observe

## Scope Definition

### In Scope
- [ ] `crates/lang-php/queries/laravel_route.scm` (new) -- tree-sitter query for `Route::get/post/...`
- [ ] `crates/lang-php/src/observe.rs` (edit) -- `extract_routes()` + Route struct + prefix resolution
- [ ] `crates/cli/src/main.rs` (edit) -- PHP observe の route extraction 呼び出し
- [ ] Controller array syntax: `Route::get('/path', [Ctrl::class, 'method'])`
- [ ] Closure/Arrow syntax: `Route::get('/path', fn () => ...)`
- [ ] FQCN inline: `Route::post('/path', [\App\...\Ctrl::class, 'method'])`
- [ ] prefix + group ネスト解決 (depth 1-3)
- [ ] middleware chain (middleware は無視し group のみ処理)

### Out of Scope
- `Route::resource` / `Route::apiResource` (Reason: P2 defer)
- Symfony / Slim / CakePHP (Reason: 需要が出てから対応)
- runtime 実行 (Reason: CONSTITUTION "no runtime")

### Files to Change (target: 10 or less)
- `crates/lang-php/queries/laravel_route.scm` (new)
- `crates/lang-php/src/observe.rs` (edit)
- `crates/cli/src/main.rs` (edit)

## Environment

### Scope
- Layer: Backend
- Plugin: php
- Risk: 10 (PASS)

### Runtime
- Language: Rust (tree-sitter static analysis)

### Dependencies (key packages)
- tree-sitter: workspace version
- tree-sitter-php: workspace version

### Risk Interview (BLOCK only)
(N/A -- Risk PASS)

## Context & Dependencies

### Reference Documents
- [CONSTITUTION.md](../../CONSTITUTION.md) - "no runtime" 原則
- [docs/languages/](../languages/) - PHP 言語固有挙動
- [crates/lang-typescript/src/observe.rs] - Route struct パターンの参照実装

### Dependent Features
- PHP observe L1/L2: `crates/lang-php/src/observe.rs`
- CLI observe dispatch: `crates/cli/src/main.rs`

### Related Issues/PRs
- (none)

## Test List

### TODO
- [ ] TC-06: Given sr108/routes/web.php, When extract_routes, Then >= 80 routes extracted (integration test)

### WIP
- [x] TC-01: Given `Route::get('/users', [UserController::class, 'index'])`, When extract_routes, Then Route{ GET, /users, index, UserController }
- [x] TC-02: Given `Route::post('/users', fn () => ...)`, When extract_routes, Then Route{ POST, /users, "", "" } (closure handler)
- [x] TC-03: Given `Route::prefix('admin')->group(function () { Route::get('/users', ...) })`, When extract_routes, Then Route{ GET, admin/users, ... }
- [x] TC-04: Given nested prefix `Route::prefix('api')->group(fn() => Route::prefix('v1')->group(fn() => Route::get('/users', ...)))`, When extract_routes, Then Route{ GET, api/v1/users, ... }
- [x] TC-05: Given `Route::middleware('auth')->group(function () { Route::get('/dashboard', ...) })`, When extract_routes, Then Route{ GET, /dashboard, ... } (middleware group は path に影響しない)

### DISCOVERED
(none)

### DONE
(none)

## Implementation Notes

### Goal
PHP observe で route extraction を実装し、Laravel dogfooding プロジェクトの route coverage を 0% から改善する。

### Background
PHP observe で routes_total=0 が続いている。3つの Laravel dogfooding プロジェクトで合計 264 Route 定義があるが、route coverage が 0%。Laravel の route 定義は `routes/*.php` に `Route::get('/path', [Controller::class, 'method'])` 形式で記述される。`Route::prefix()->group()` によるネスト (depth 1-3) がある。

`php artisan route:list` は runtime 実行 (Laravel ブート必要) のため、exspec の静的解析原則に反する。tree-sitter で静的解析する。

### Design Approach
TS observe と同じパターン: Route struct + `extract_routes()` を lang-php に実装。core は変更しない。

```rust
// crates/lang-php/src/observe.rs
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub http_method: String,
    pub path: String,
    pub handler_name: String,    // Controller method
    pub class_name: String,      // Controller class
    pub file: String,
    pub line: usize,
}

impl PhpExtractor {
    pub fn extract_routes(&self, source: &str, file_path: &str) -> Vec<Route> { ... }
}
```

**prefix 解決アルゴリズム:**
1. tree-sitter で全 `Route::get/post/...` call を検出
2. 各 call の byte range を記録
3. tree-sitter で全 `Route::prefix('...')` call を検出し、その `group(function() { ... })` の byte range を記録
4. 各 route call に対して、それを含む prefix group を逆順 (内→外) に辿り、prefix を累積

**対応パターン優先度:**

| パターン | 例 | 優先度 |
|---------|---|--------|
| Controller array | `Route::get('/path', [Ctrl::class, 'method'])` | P1 |
| Closure/Arrow | `Route::get('/path', fn () => ...)` | P1 |
| FQCN inline | `Route::post('/path', [\App\...\Ctrl::class, 'method'])` | P1 |
| prefix + group | `Route::prefix('admin')->group(fn () => ...)` | P1 |
| middleware chain | `Route::middleware('auth')->group(...)` | P2 (skip middleware, handle group) |
| resource | `Route::resource('posts', PostCtrl::class)` | P2 (defer) |

## Verification

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo run -- --lang rust .
cargo run -- observe --lang php --format json ~/Documents/NewsService/sr108  # routes > 0
```

Evidence: (orchestrate が自動記入)

## Progress Log

### 2026-03-26 13:05 - INIT
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
