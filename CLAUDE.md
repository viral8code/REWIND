# REWIND: 作業の取り決め

## 役割分担

- 日本語の文書は、skill の `yomiyasu` で推敲してからコミットする。対象は `README.md`、`libraries/README.md`、`docs/language-reference.md`、`docs/book/chapters/*.md`、`docs/REWIND_v*.md`、`docs/v2-next-status.md` と、`docs/` 配下の説明文。コード例、コマンド、識別子、数値、リンク先は変えない。推敲のあとで文書の例の検査と、HTML・PDF の再生成をやり直す。

## 作業の約束

- コミットは `codex/develop` にだけ行い、基本はメッセージに `[skip ci]` を付ける。push、タグ、Release、版の更新、`main` へのマージはしない。
- コミットの末尾に、`Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` を付ける。
- 変更の前後で、`cargo test --release --no-fail-fast`（全体）と、`python scripts/check-docs-examples.py docs/language-reference.md docs/book/chapters/*.md`（文書の例）を通す。
- ドキュメントを変えたら、`python scripts/build-book.py` と `python scripts/build-reference-pdf.py` で HTML と PDF を再生成する。
- Linux の確認が必要なときは、WSL（`Ubuntu-24.04`）を使う。ビルドは `CARGO_TARGET_DIR=$HOME/rw-target` に出す。GUI の実機確認には `scripts/gui-drive-windows.ps1`、`scripts/gui-drive-x11.py` を使う。
