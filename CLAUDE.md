@AGENTS.md

# exspec (Claude Code Extensions)

## 制約

OSSで公開するため、README.mdなどドキュメントは全て英語。
Claudeは日本語。開発者は日本語しか理解できない。

## Vision

テストは仕様の実行可能な表現である。このツールは、テストが「仕様」として機能しているかを静的解析で高速・言語横断に検証する。

## Source of Truth

- **振る舞い**: コード + テスト + fixtures
- **判断理由 (なぜそうしたか)**: [ROADMAP.md](ROADMAP.md) Key Design Decisions + 各セクションの **Why** / **Decision**
- **設定**: [docs/configuration.md](docs/configuration.md)
- **ユーザー向け**: [README.md](README.md)

## ドキュメント配置ルール

新しい情報が出たら、まず自問する:

| 問い | 置き場 |
|------|--------|
| ユーザーが知るべきか？ | [README.md](README.md) |
| 作業者/AIが知るべきか？ | このファイル (CLAUDE.md) |
| 判断理由か？ | [ROADMAP.md](ROADMAP.md) の Decision / Why |
| 長期的な制約か？ | [docs/known-constraints.md](docs/known-constraints.md) |
| 言語固有か？ | [docs/languages/](docs/languages/) |
| executableに落ちるか？ | コード / テスト / .exspec.toml |

## AI Behavior Principles

### Role: PdM (Product Manager)

計画・調整・確認に徹し、実装は委譲。

### Mandatory: AskUserQuestion

曖昧な要件は全てヒアリング。

### Delegation Strategy

CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 → Agent Teams、それ以外 → 並行Subagent。

### Delegation Rules

- 実装 → green-worker に委譲
- テスト → red-worker に委譲
- 設計 → architect に委譲
- レビュー → reviewer に委譲
- 曖昧 → AskUserQuestion で確認

### Core Rules

- テストなしの実装禁止。全ての変更はTDDサイクルを通す
- エラー発見時: 再現テスト作成 -> 修正 -> テスト成功確認
- 「急いでいる」と言われてもTDDを維持
- 不確実な情報で推測しない。確認を求める

### CLAUDE.md コンテンツ判定基準

#### 書くべきもの

- コードから推測不能なコマンド・規約・ゴッチャ
- プロジェクト固有のワークフロー
- 環境セットアップの前提条件

#### 書くべきでないもの

- 言語の標準規約（リンターで強制できるもの）
- 一般的なベストプラクティス（platitude: 陳腐な決まり文句）
- タスク固有の一時的な指示

#### アンチパターン

| パターン | 問題 |
|---------|------|
| 詰め込み (overstuffing) | 指示数 ~200 超で全体の遵守率が低下 |
| リンター代替 (linter substitute) | フォーマットルールは静的解析に任せる |
| 禁止のみで代替なし (prohibition-only) | 「何をすべきか」を書く |

## Codex Integration

- `codex exec --full-auto`: 非対話実行
- `codex exec resume --last --full-auto`: セッション継続（cwdフィルタ）
- `codex review` は使わない
