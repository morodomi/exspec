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
SPEC -> sync-plan -> RED -> GREEN -> REFACTOR -> REVIEW -> COMMIT
```

| Phase | Action | Skill |
|-------|--------|-------|
| SPEC | 設計・テスト計画 (plan mode) | dev-crew:spec |
| sync-plan | Design Review Gate + Cycle doc作成 | dev-crew:orchestrate (Block 1) |
| RED | テスト作成、失敗確認 | dev-crew:red |
| GREEN | 最小限の実装 | dev-crew:green |
| REFACTOR | コード品質改善 | dev-crew:refactor |
| REVIEW | コードレビュー | dev-crew:review |
| COMMIT | Git commit | dev-crew:commit |

Cycle docs: `docs/cycles/YYYYMMDD_HHMM_<topic>.md`

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
├── Cargo.toml
├── ROADMAP.md                 中期ロードマップ
├── crates/
│   ├── core/                  言語非依存の解析エンジン
│   │   ├── extractor.rs       テスト関数抽出
│   │   ├── rules.rs           ルール定義・評価
│   │   ├── metrics.rs         メトリクス計算
│   │   ├── output.rs          出力フォーマッタ
│   │   └── suppress.rs        インラインサプレッション処理
│   ├── lang-python/           Python固有
│   │   └── queries/*.scm
│   ├── lang-typescript/       TypeScript固有
│   │   ├── queries/*.scm
│   │   └── observe.rs         observe PoC (test-to-code mapping)
│   ├── lang-php/              PHP固有
│   │   └── queries/*.scm
│   ├── lang-rust/             Rust固有
│   │   └── queries/*.scm
│   └── cli/                   CLIエントリポイント
├── tests/
│   ├── fixtures/              各言語のサンプルテストコード (SPEC駆動)
│   └── integration/
├── .exspec.toml               dogfooding用設定
└── docs/
```

### queries/*.scm (言語別)

```
queries/
  ├── test_function.scm      テスト関数の抽出
  ├── mock_usage.scm         mock/stub/spy検出
  ├── assertion.scm          assert文検出
  ├── parameterized.scm      パラメタライズ検出
  └── contract.scm           Pydantic/Pandera等検出
```

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
