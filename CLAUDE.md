# exspec -- Executable Specification Analyzer

## 制約

OSSで公開するため、README.mdなどドキュメントは全て英語。
Claudeは日本語。開発者は日本語しか理解できない。

## Vision

テストは仕様の実行可能な表現である。このツールは、テストが「仕様」として機能しているかを静的解析で高速・言語横断に検証する。

## Why

- AI生成コード時代、テストを「書く」コストは激減するが「テストの質」担保は未成熟
- SonarQube=カバレッジ、mutation testing=実行コスト高、similarity=重複検出
- テストコードの「設計品質」を静的・高速・言語横断で検証するツールは空白地帯
- dev-crewとの統合でLLMコスト0の品質ゲートを実現

## Tech Stack

- **Language**: Rust
- **AST解析**: tree-sitter (ネイティブバインディング)
- **クエリ**: tree-sitter Query (.scm) 外出し -- Rustを再コンパイルせずにロジック調整可能
- **出力**: JSON / SARIF / Terminal / AI Prompt
- **配布**: cargo install exspec

## Project Structure

```
exspec/
├── Cargo.toml
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
│   │   └── queries/*.scm
│   └── cli/                   CLIエントリポイント
├── tests/
│   ├── fixtures/              各言語のサンプルテストコード (SPEC駆動)
│   └── integration/
├── .exspec.toml               dogfooding用設定
└── docs/
    └── SPEC.md                仕様書
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

## Check Rules

### Tier 1: MVP

| ID | Rule | Detection | Level | Default Threshold |
|----|------|-----------|-------|-------------------|
| T001 | assertion-free | テスト関数内にassert/expect/shouldがない | BLOCK | -- |
| T002 | mock-overuse | mock/stub/spy数 > 閾値 + 異クラスmock数 | WARN | mock_max=5, mock_class_max=3 |
| T003 | giant-test | テスト関数行数 > 閾値 | WARN | test_max_lines=50 |
| T004 | no-parameterized | パラメタライズドテスト比率 < 閾値 | INFO | parameterized_min_ratio=0.1 |
| T005 | pbt-missing | PBTライブラリimportなし | INFO | -- |
| T006 | low-assertion-density | assert数/テスト関数数 < 1.0 | WARN | -- |
| T007 | test-source-ratio | テストファイル数/ソースファイル数 | INFO | -- |
| T008 | no-contract | Pydantic/Zod/Pandera等のテスト内使用なし | INFO | -- |

### Tier 2: v0.2

| ID | Rule | Level |
|----|------|-------|
| T101 | how-not-what (実装検証パターン) | WARN |
| T102 | fixture-sprawl (共通helper依存過多) | WARN |
| T103 | missing-error-test (異常系テストなし) | INFO |
| T104 | hardcoded-only (リテラル値のみ) | INFO |
| T105 | deterministic-no-metamorphic | WARN |

### Tier 3: v1.0 (AI検査プロンプト生成)

| ID | Rule | Output |
|----|------|--------|
| T201 | spec-quality | テストが仕様として読めるかのAI検査プロンプト |
| T202 | contract-property-coherence | Contract+Propertyの整合性検査プロンプト |
| T203 | test-duplication | similarity的重複検出 |

## Output Philosophy: 静的解析 + AI連携ハイブリッド

| Layer | Content | Consumer |
|-------|---------|----------|
| Block/Warn/Pass | ルール判定結果 | CI (exit code) |
| Metrics (%) | mock密度、PBT比率、パラメタライズ率等 | Human + AI |
| AI Prompt | Tier 3領域の意味論的チェック用プロンプト | LLM (dev-crew等) |

## CLI

```bash
exspec .                          # 基本実行 (BLOCK=exit 1, WARN/INFO=exit 0)
exspec --strict .                 # WARN以上でexit 1
exspec --lang python .            # 言語指定
exspec --format json .            # JSON出力
exspec --format sarif .           # SARIF (GitHub Code Scanning)
exspec --format ai-prompt .       # AI検査プロンプト出力
exspec init --lang python,typescript  # 設定ファイル生成
```

### Inline Suppression

```python
# exspec-ignore: T002
def test_complex_integration():
    ...
```

## Config (.exspec.toml)

```toml
[general]
lang = ["python", "typescript"]

[rules]
disable = ["T004"]

[thresholds]
mock_max = 5
mock_class_max = 3
test_max_lines = 50
parameterized_min_ratio = 0.1

[paths]
test_patterns = ["tests/**", "**/*_test.*", "**/*.test.*"]
ignore = ["node_modules", ".venv", "vendor"]
```

## Supported Languages

| Phase | Language | Test FW |
|-------|----------|---------|
| MVP | Python | pytest |
| MVP | TypeScript | Jest/Vitest |
| MVP | PHP | PHPUnit/Pest |
| v0.2 | Rust | cargo test |
| v1.0 | Dart | flutter_test (best-effort) |

## Development Approach: SPEC-Driven

```
SPEC.md (ルールごとの入力→期待出力)
  → fixtures/ (違反/準拠サンプル)
    → tests/ (fixture → 期待出力の検証)
      → queries/*.scm (tree-sitterクエリ)
        → crates/ (Rust実装)
```

## Development Phases

| Phase | Content | Deliverable |
|-------|---------|-------------|
| 0 | SPEC.md + 命名 + 壁打ち | 仕様書 (DONE) |
| 1 | Rust + tree-sitter scaffolding | cargo build通る (DONE) |
| 2 | Python + Tier 1 (T001-T003) | 3ルール動作 (DONE) |
| 3A | TypeScript + inline suppression + output polish | Python+TS両対応 (DONE) |
| 3B | T004-T008 + .exspec.toml parsing | 残Tier 1ルール (DONE) |
| 3C | SARIF出力 + metrics本格化 | MVP完成 (DONE) |
| 4 | dev-crew hook統合 + PHP対応 | quality-gate skill + PHP 3言語対応 (DONE) |
| 5 | Tier 2 + Rust対応 | v0.2 |
| 6 | Tier 3 (AI Prompt生成) | v1.0 |
| 7 | OSS公開 + Note記事 + MCP Server | 公開 |

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
- red-workerは変更なし（exspec呼び出しはオーケストレーター側の責務）

## Quick Commands

```bash
cargo test                                      # テスト実行
cargo llvm-cov --html --open                    # カバレッジ (HTML)
cargo llvm-cov --lcov --output-path lcov.info   # カバレッジ (CI用)
cargo clippy -- -D warnings                     # 静的解析
cargo fmt --check                               # フォーマットチェック
cargo fmt                                       # フォーマット適用
```

## TDD Workflow

```
INIT -> PLAN -> RED -> GREEN -> REFACTOR -> REVIEW -> COMMIT
```

| Phase | Action | Skill |
|-------|--------|-------|
| INIT | Cycle doc作成、コンテキスト設定 | dev-crew:init |
| PLAN | 設計・テスト計画 | dev-crew:strategy |
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

## AI Behavior Principles

- テストなしの実装禁止。全ての変更はTDDサイクルを通す
- エラー発見時: 再現テスト作成 -> 修正 -> テスト成功確認
- 「急いでいる」と言われてもTDDを維持
- 不確実な情報で推測しない。確認を求める

## Git Conventions

```
<type>: <subject>

feat | fix | docs | refactor | test | chore
```

コミット前: `cargo test` + `cargo clippy -- -D warnings` + `cargo fmt --check`
