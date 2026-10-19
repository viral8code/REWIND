# REWIND: 作業の取り決め

## 役割分担

- **実装は Haiku 5.5（`claude-haiku-5-5`）が担当する。** コードの変更、テストの追加、ドキュメントの更新といった実装作業は、サブエージェント（Agent ツール、`model: haiku`）に任せる。
- Sonnet 5.5 は、調査、設計、作業の分割と指示、成果物の確認（差分の読み、テストの実行結果の確認）、コミット前のレビューを担当する。
- サブエージェントには、対象のファイル、期待する動作、完了条件（実行するコマンドと、通るべきテスト）を具体的に書いて渡す。`.claude/settings.json` の `CLAUDE_CODE_SUBAGENT_MODEL` で、サブエージェントの既定のモデルを Haiku 5.5 にしている。

## 作業の約束

- コミットは `codex/develop` にだけ行い、メッセージに `[skip ci]` を付ける。push、タグ、Release、版の更新、`main` へのマージはしない。
- コミットの末尾に、`Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` を付ける（実装を Haiku 5.5 が行った場合も、コミットは Sonnet 5.5 が確認して行う）。
- 変更の前後で、`cargo test --release --no-fail-fast`（全体）と、`python scripts/check-docs-examples.py docs/language-reference.md docs/book/chapters/*.md`（文書の例）を通す。
- ドキュメントを変えたら、`python scripts/build-book.py` と `python scripts/build-reference-pdf.py` で HTML と PDF を再生成する。
- Linux の確認が必要なときは、WSL（`Ubuntu-24.04`）を使う。ビルドは `CARGO_TARGET_DIR=$HOME/rw-target` に出す。GUI の実機確認には `scripts/gui-drive-windows.ps1`、`scripts/gui-drive-x11.py` を使う。

## コンテキスト

- 自動圧縮（auto compact）の 100K トークンは **Haiku（実装を担当するサブエージェント）にだけ**適用する。メインの Sonnet のセッションには適用しない。
- `.claude/settings.json` の `modelSettings.claude-haiku-5-5.autoCompactWindow` でモデル別に 100K を指定している。環境全体の `CLAUDE_CODE_AUTO_COMPACT_WINDOW` は Sonnet にも効いてしまうため、書かない。
- Haiku への指示は小さく自己完結させ、1 回の実行が約 100K トークンに収まるようにする。
