# exspec -- Executable Specification Analyzer

## Start Here

| 何を知りたいか | どこを見るか |
|---------------|-------------|
| プロジェクトの方向性と判断理由 | [ROADMAP.md](ROADMAP.md) |
| 設計思想 | [docs/philosophy.md](docs/philosophy.md) |
| 既知の制約 | [docs/known-constraints.md](docs/known-constraints.md) |
| 設定とエスケープハッチ | [docs/configuration.md](docs/configuration.md) |
| 言語固有の挙動 | [docs/languages/](docs/languages/) |
| 実プロジェクトでの検証結果 | [docs/dogfooding-results.md](docs/dogfooding-results.md) |
| ルール仕様 (入力→期待出力) | [docs/SPEC.md](docs/SPEC.md) |
| ユーザー向け概要 | [README.md](README.md) |
| 現在のステータス | [docs/STATUS.md](docs/STATUS.md) |

## Tech Stack

- **Language**: Rust
- **AST解析**: tree-sitter (ネイティブバインディング)
- **クエリ**: tree-sitter Query (.scm) 外出し -- Rustを再コンパイルせずにロジック調整可能
- **出力**: JSON / SARIF / Terminal / AI Prompt
- **配布**: cargo install exspec

## Quick Commands

```bash
cargo test                                      # テスト実行
cargo llvm-cov --html --open                    # カバレッジ (HTML)
cargo llvm-cov --lcov --output-path lcov.info   # カバレッジ (CI用)
cargo clippy -- -D warnings                     # 静的解析
cargo fmt --check                               # フォーマットチェック
cargo fmt                                       # フォーマット適用
cargo run -- --lang rust .                      # self-dogfooding (BLOCK 0件を確認)
```

## TDD Workflow

```
spec -> sync-plan -> plan-review -> RED -> GREEN -> REFACTOR -> REVIEW -> COMMIT
```

| Phase | Action | Skill | 詳細 |
|-------|--------|-------|------|
| SPEC | 設計・テスト計画 | dev-crew:spec | plan mode内で実行。要件ヒアリング → リスク評価 → Test List作成 → planファイルに記録 |
| (plan review) | 設計レビュー | dev-crew:review --plan | plan mode内でspec後に実施。approve → compact → orchestrate起動 |
| sync-plan | Design Review Gate + Cycle doc作成 | dev-crew:orchestrate (Block 1) | architect が設計レビュー後、Cycle doc を `docs/cycles/` に生成 |
| RED | テスト作成、失敗確認 | dev-crew:red | Given/When/Then形式。テスト失敗を確認後、self-dogfooding実施 |
| GREEN | 最小限の実装 | dev-crew:green | テストを通す最小コード。過剰実装禁止 |
| REFACTOR | コード品質改善 | dev-crew:refactor | チェックリスト駆動。Verification Gateで品質確認 |
| REVIEW | コードレビュー | dev-crew:review | 正確性・パフォーマンス・セキュリティの並列レビュー |
| COMMIT | Git commit | dev-crew:commit | テスト通過 + clippy + fmt + self-dogfooding確認後にコミット |

**orchestrate** (`dev-crew:orchestrate`): PdMとしてsync-plan→RED→GREEN→REFACTOR→REVIEW→COMMITを自律管理。PASS/WARN→自動進行、BLOCK→再試行→ユーザー報告。

Cycle docs: `docs/cycles/YYYYMMDD_HHMM_<topic>.md`

### Post-Approve Action

planファイル末尾の `## Post-Approve Action` セクションがcompact後のauto-orchestrateトリガーになる。specで自動付与される。

## Quality Standards

| Metric | Target |
|--------|--------|
| Coverage | 90%+ (min 80%) |
| Static analysis (clippy) | 0 errors |
| Format (rustfmt) | 差分なし |
| exspec (self-dogfooding) | BLOCK 0件 |

### Self-Dogfooding

exspec自身のテストに対して `cargo run -- --lang rust .` を実行し、BLOCKが0件であることを確認する。
RED phase完了時またはコミット前に必ず実施すること。

## Project Structure

```
exspec/
├── Cargo.toml                 Workspace root (6 crates)
├── ROADMAP.md                 中期ロードマップ
├── crates/
│   ├── core/                  言語非依存の解析エンジン
│   │   ├── config.rs          .exspec.toml 設定パーサー
│   │   ├── extractor.rs       テスト関数抽出
│   │   ├── rules.rs           ルール定義・評価
│   │   ├── metrics.rs         メトリクス計算
│   │   ├── output.rs          出力フォーマッタ
│   │   ├── hints.rs           ランタイムヒント (custom_patterns案内等)
│   │   ├── query_utils.rs     tree-sitterクエリユーティリティ
│   │   └── suppress.rs        インラインサプレッション処理
│   ├── lang-python/           Python固有 (pytest)
│   │   └── queries/*.scm      (14 queries)
│   ├── lang-typescript/       TypeScript固有 (Jest/Vitest)
│   │   ├── queries/*.scm      (15 queries, incl. production_function, decorator, import_mapping)
│   │   └── observe.rs         observe PoC (production function/route抽出, test-to-code mapping)
│   ├── lang-php/              PHP固有 (PHPUnit/Pest)
│   │   └── queries/*.scm      (13 queries)
│   ├── lang-rust/             Rust固有 (cargo test)
│   │   └── queries/*.scm      (12 queries)
│   └── cli/                   CLIエントリポイント
├── tests/
│   └── fixtures/              各言語のサンプルテストコード (SPEC駆動)
│       ├── python/
│       ├── typescript/
│       ├── php/
│       ├── rust/
│       └── config/
├── .exspec.toml               dogfooding用設定
└── docs/
```

### queries/*.scm (言語別共通パターン)

```
queries/
  ├── test_function.scm        テスト関数の抽出
  ├── assertion.scm            assert文検出
  ├── mock_usage.scm           mock/stub/spy検出
  ├── mock_assignment.scm      mock代入検出
  ├── parameterized.scm        パラメタライズ検出
  ├── how_not_what.scm         実装詳細アクセス検出
  ├── private_in_assertion.scm private属性アクセス検出
  ├── relational_assertion.scm リレーショナルアサーション検出
  ├── error_test.scm           エラーテスト検出
  ├── wait_and_see.scm         sleep/wait検出
  ├── skip_test.scm            skip/only検出 (Python/PHP)
  ├── import_contract.scm      契約ライブラリimport検出
  └── import_pbt.scm           PBTライブラリimport検出
```

TypeScript追加: `production_function.scm`, `decorator.scm`, `import_mapping.scm` (observe PoC用)

## Development Approach: SPEC-Driven

```
SPEC.md (ルールごとの入力→期待出力)
  → fixtures/ (違反/準拠サンプル)
    → tests/ (fixture → 期待出力の検証)
      → queries/*.scm (tree-sitterクエリ)
        → crates/ (Rust実装)
```

## dev-crew Integration

RED Phase Stage 3完了後にquality-gate skillを呼び出し:

```
RED Phase → テスト作成完了
  → Quality Gate: quality-gate skill (exspec --format json)
    ├── exit 0 → Verification Gate → GREEN Phase
    └── exit 1 → red-workerにフィードバック → 最大2回リトライ
```

- exspec未インストール時はスキップ（WARNログ）
- `--strict`は使わない（BLOCKのみexit 1）

## Git Conventions

```
<type>: <subject>

feat | fix | docs | refactor | test | chore
```

コミット前: `cargo test` + `cargo clippy -- -D warnings` + `cargo fmt --check` + `cargo run -- --lang rust .`
