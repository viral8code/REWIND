# REWIND: 作業の取り決め

## 役割分担

作業の性質でモデルを分ける。役割は `.claude/agents/` に定義している。

```
探索・検索タスク  → Haiku 5.5（claude-haiku-5-5）
分析・実装タスク  → Sonnet 5.5（claude-sonnet-5-5）
複雑な設計・判断  → Opus 5.5（claude-opus-5-5）
```

| モデル | 役割（agent） | 担当 |
|---|---|---|
| Haiku 5.5 | `explorer` | コードと docs の探索・検索（読み取りだけ） |
| Haiku 5.5 | `test-runner` | ビルドとテストの実行、失敗の切り分け |
| Sonnet 5.5 | `lib-developer` | 標準ライブラリ（`libraries/std`）の実装、テスト、API baseline、文書 |
| Sonnet 5.5 | `rust-implementer` | 処理系（`src/` の Rust）の実装と修正 |
| Sonnet 5.5 | `doc-writer` | 日本語のドキュメントの執筆・更新、HTML と PDF の再生成 |
| Sonnet 5.5 | `reviewer` | 差分のレビュー（指摘だけを返す） |
| Sonnet 5.5 | `perf-analyst` | 計測と性能の原因の特定 |
| Opus 5.5 | `architect` | 言語の意味や新機能の設計、案の比較と推奨 |
| Opus 5.5 | `security-reviewer` | 暗号・認証・Web・DB の脅威分析 |

- メインのセッションは、作業の分割と指示、成果物の確認（差分の読み、テストの実行結果の確認）、ユーザーとの対話、コミットを担当する。
- 役割を指定せずに小さな機械的な変更を任せるときは、`implementer`（Haiku 5.5）を使ってよい。
- サブエージェントには、対象のファイル、期待する動作、完了条件（実行するコマンドと、通るべきテスト）を具体的に書いて渡す。
- 設計の判断が要る作業は、先に `architect` に設計と実装の指示を作らせ、その指示を実装の役割に渡す。セキュリティに関わる変更は、コミットの前に `security-reviewer` を通す。
- 日本語の文書は、skill の `yomiyasu` で推敲してからコミットする。対象は `README.md`、`libraries/README.md`、`docs/language-reference.md`、`docs/book/chapters/*.md`、`docs/REWIND_v*.md`、`docs/v2-next-status.md` と、`docs/` 配下の説明文。コード例、コマンド、識別子、数値、リンク先は変えない。推敲のあとで文書の例の検査と、HTML・PDF の再生成をやり直す。
- 文書を書く agent（`doc-writer`、`lib-developer`、`rust-implementer`、`perf-analyst`）は、定義の `skills` に `yomiyasu` を持つ。メインが文書を直接書いたときも同じ。`.rw` の `// @api` コメント、コミットメッセージ、サブエージェントとの間のやり取りには使わない。

## 作業の約束

- コミットは `codex/develop` にだけ行い、メッセージに `[skip ci]` を付ける。push、タグ、Release、版の更新、`main` へのマージはしない。
- コミットの末尾に、`Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` を付ける（実装をどのサブエージェントが行った場合も、コミットはメインが確認して行う）。
- 変更の前後で、`cargo test --release --no-fail-fast`（全体）と、`python scripts/check-docs-examples.py docs/language-reference.md docs/book/chapters/*.md`（文書の例）を通す。
- ドキュメントを変えたら、`python scripts/build-book.py` と `python scripts/build-reference-pdf.py` で HTML と PDF を再生成する。
- Linux の確認が必要なときは、WSL（`Ubuntu-24.04`）を使う。ビルドは `CARGO_TARGET_DIR=$HOME/rw-target` に出す。GUI の実機確認には `scripts/gui-drive-windows.ps1`、`scripts/gui-drive-x11.py` を使う。

## コンテキスト

- 自動圧縮（auto compact）の窓は、`.claude/settings.json` の `modelSettings.<モデル>.autoCompactWindow` でモデル別に指定する。Haiku は 100K、Sonnet は 300K、Opus は 500K。環境全体の `CLAUDE_CODE_AUTO_COMPACT_WINDOW` は全モデルに効いてしまうため、書かない。
- サブエージェントの既定のモデルを環境変数 `CLAUDE_CODE_SUBAGENT_MODEL` で固定しない。各 agent の定義（`model:`）が優先されるようにする。
- Haiku への指示は小さく自己完結させ、1 回の実行が約 100K トークンに収まるようにする。
- 大きいファイル（`src/v2.rs`、`libraries/std/tests.rw`、`libraries/api/*.api.json` の全文など）は丸ごと読まず、Grep と行範囲の指定で読む。出力は `| tail -30` などで絞る。
