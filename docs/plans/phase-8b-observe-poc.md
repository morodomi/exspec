# Phase 8b: exspec observe PoC

## Vision

exspec observe = 静的 AST-only test-to-code マッピング + 異常系ギャップ検出

既存ツール (Microsoft TIA, SeaLights, Launchable) は全て動的計装 or ML予測。
純粋な静的 AST-only の test-to-code mapping ツールは存在しない。exspec observe が成功すれば新カテゴリを作れる。

**重要な区別**: observe は「構造的可観測性 (structural observability)」であり、カバレッジツールではない。コード行の実行有無ではなく、テストとプロダクションコードの構造的対応関係を静的に推定する。

### Target User

AI エージェント (Claude Code, Codex 等)。「次にどのテストを書くべきか」の判断材料を提供する。
Markdown 出力は人間にも読めるが、主たる消費者は AI。

## PoC Output Image (Layer 1+2 完了時)

```
exspec --observe --lang typescript src/

src/users/users.controller.ts
  routes: GET /users, POST /users, DELETE /users/:id
  tests: users.controller.spec.ts (5 tests)
  gap: @IsEmail on CreateUserDto but no 400-expecting test found
  gap: @UseGuards(AuthGuard) but no 401-expecting test found
  unmapped routes: DELETE /users/:id
```

### MVP Output Image (Layer 3 追加後, v0.2.0+)

```
src/users/users.controller.ts
  GET  /users        3 tests   [normal:2, error:1]
  POST /users        1 test    [normal:1, error:0]  <- @IsEmail validation untested
  DELETE /users/:id  0 tests   <- unmapped
```

## Scope

- 1 language: TypeScript
- 1 framework: NestJS
- PoC validation target: nestjs/nest (dogfooding済み, 2675 tests)
- Output: Markdown (Terminal表示兼用, AI-readable)
- No timebox: 期限なし。ただし go/no-go 判定基準は厳守

## Success / Failure Criteria

| Metric | Success | Failure |
|--------|---------|---------|
| Precision | >= 70% | < 50% |
| Recall | >= 60% | < 40% |

50-70% precision or 40-60% recall = grey zone。Layer 2 (import tracing) で改善を試み、改善しなければ failure。

### Evaluation Corpus (実装前に固定)

nestjs/nest の以下モジュールを ground truth 対象とする (実装前に手動マッピングを完了させる):
- `packages/common` (decorators, pipes, guards の定義元)
- `packages/core` (NestFactory, router, middleware)
- `packages/testing` (テストユーティリティ)

対象の全 public Controller/Provider メソッドを手動リストアップし、対応テストをマッピングした ground truth ファイルを `docs/observe-ground-truth.md` に作成。

## Architecture

```
Production Code                    Test Code
--------------                    ---------
@Controller('users')              describe('UsersController')
  @Get()       findAll()            it('should return users')
  @Post()      create()             it('should create user')
  @Delete(':id') remove()           (no test)

@IsEmail()   (CreateUserDto.email)
@UseGuards() (AuthGuard on create)

         | AST extraction |              | existing extraction |

    RouteMap + DecoratorMap          TestMap (test -> status assertions)
              \                    /
           Mapper (name + import matching)
              |
         ObserveReport (Markdown)
```

### Trait Design

LanguageExtractor trait を汚染しない。observe 用に `ObserveExtractor` trait を lang-typescript 内に新設する。

**Why**: PoC 失敗時に安全に削除可能。既存 4 言語実装に空実装を追加する必要なし。LanguageExtractor への統合は observe が MVP に昇格してから再設計。

```rust
// crates/lang-typescript/src/observe.rs (new)
pub trait ObserveExtractor {
    fn extract_production_functions(&self, source: &str, file_path: &str) -> Vec<ProductionFunction>;
    fn extract_routes(&self, source: &str, file_path: &str) -> Vec<Route>;
    fn extract_decorators(&self, source: &str, file_path: &str) -> Vec<DecoratorInfo>;
}
```

### CLI Design

PoC ではサブコマンド化しない。`--observe` フラグで暫定実装。

**Why**: 現行 main.rs はフラット Cli struct で、Subcommand derive への移行は CLI 全体の破壊的変更。サブコマンド化は observe MVP 昇格と同タイミングで行う。

## Mapping Strategy (TCTracer-inspired)

### PoC scope: Layer 1 + Layer 2

#### Layer 1: ファイル名規約 (naive mapper)

`users.controller.ts` -> `users.controller.spec.ts` (NestJS の標準規約)

#### Layer 2: import 解析

`import { UsersController } from './users.controller'` -> 直接リンク

**Security constraint**: import パス解決時に `canonicalize` + `starts_with(scan_root)` チェック必須。scan root 外へのパス traversal を防止。

**Scope limitation**: `import_mapping.scm` の責務は `(module_specifier, symbol_name)` ペアの抽出のみ。パス解決は Rust 側の専用関数に分離。barrel export (index.ts re-export) の再帰解決は PoC スコープ外。

### Post-PoC: Layer 3 (呼び出し解析)

テスト内の `controller.findAll()` -> `UsersController.findAll` へのメソッドレベルマッピング。変数の型解決が必要で難易度が高い。PoC 成功後の v0.2.1 以降で実装。

## Error-path Gap Detection

2つの解析の掛け合わせ:

### 1. プロダクション側: decorator 検出 (tree-sitter query)

| Decorator | 意味 |
|-----------|------|
| `@IsEmail()`, `@IsNotEmpty()` 等 (class-validator) | validation あり |
| `@UseGuards(AuthGuard)` | 認証ガードあり |
| `@UsePipes(ValidationPipe)` | validation pipe あり |

### 2. テスト側: status code assertion 検出 (新規 query)

テストが期待する HTTP status code を検出:

```typescript
// 検出対象パターン
expect(response.status).toBe(400);
expect(response.statusCode).toBe(422);
.expect(400)    // supertest
.expect(401)
.expect(403)
```

**Gap 条件**: decorator が存在するのに、マッピングされたテスト群の中に対応する error status を期待するテストがない。

## Task Breakdown

| # | タスク | 依存 | サイズ |
|---|--------|------|--------|
| 0 | Ground truth 作成 (nestjs/nest 手動マッピング) | なし | M |
| 1 | Production function extractor (TypeScript) | なし | M |
| 2 | NestJS route/decorator extractor (queries) | なし | M |
| 3a | Test-to-code mapper: ファイル名規約 (Layer 1) | 1 | M |
| 3b | Test-to-code mapper: import tracing (Layer 2) | 3a | M |
| 4a | Test status code assertion extractor (query) | なし | S |
| 4b | 異常系ギャップ analyzer | 2, 3a, 4a | M |
| 5 | `exspec --observe` CLI + Markdown 出力 | 3a, 4b | M |
| 6 | NestJS 精度検証 (ground truth 比較) | 0, 5 | M |

### Go/No-Go Gate

Task 6 完了時に precision/recall を計測:
- Success (precision >= 70%, recall >= 60%) → observe を v0.2.0 としてリリース準備
- Grey zone → Layer 2 で改善を試みる
- Failure (precision < 50% or recall < 40%) → observe コード削除。Phase 8c fallback (Go + Tier3) へ

### Post-PoC Versioning (go の場合)

| Version | 内容 |
|---------|------|
| v0.2.0 | Layer 1+2 + gap detection (PoC コードをリリース) |
| v0.2.1 | Layer 3 (method-level mapping) |
| v0.2.2 | 2nd language or multi-framework |

## Technical Notes

### 既存インフラの再利用

- tree-sitter TypeScript grammar: 既存
- Query caching (OnceLock): 既存
- テストファイル発見 + ソースファイル発見: 既存 (T007用)
- ObserveExtractor: 新規 trait (lang-typescript 内に閉じる)

### 新規 tree-sitter クエリ

| Query file | 責務 |
|-----------|------|
| `production_function.scm` | export された関数/メソッド抽出 |
| `decorator.scm` | NestJS decorator (@Controller, @Get, @Post, @UseGuards, etc.) 抽出 |
| `import_mapping.scm` | import 文から (module_specifier, symbol_name) ペア抽出のみ。パス解決は Rust 側 |
| `status_assertion.scm` | テスト内の HTTP status code assertion 検出 (expect(status).toBe(4xx), .expect(4xx)) |

### 設計判断

| 判断 | Why |
|------|-----|
| NestJS 固有で OK | PoC は特定フレームワークで精度を検証する目的。汎用化は MVP フェーズ |
| Markdown 出力のみ | JSON/SARIF は MVP 以降。AI agent も人間も Markdown で十分 |
| 同一ディレクトリスコープ | cross-module 解析は PoC スコープ外 |
| ObserveExtractor は lang-typescript 内 | 既存 trait を汚染しない。失敗時に安全に削除可能 |
| CLI はフラグ暫定 | サブコマンド化は MVP 昇格時 |
| 「構造的可観測性」と表現 | coverage tool との差別化。コード行の実行有無ではなく構造的対応関係 |

### Security

- import パス解決: `canonicalize` + `starts_with(scan_root)` ガード必須
- file size guard: 既存方針を継承 (LOW risk, accept)
- Markdown output injection: 既存方針を継承 (local CLI, LOW risk)

## Review Log

- 2026-03-14: design-reviewer PASS (42), security-reviewer PASS (12) -> Aggregate PASS (27)
- 2026-03-14: Codex review WARN -> Layer 3 を PoC スコープ外に、recall 指標追加、Task 4 分割、評価コーパス固定
- Key decisions: PoC go/no-go 判定後に versioning。Layer 3 は post-PoC (v0.2.1)
