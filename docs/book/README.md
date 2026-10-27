# REWIND 入門

REWIND 4.2.0 の入門書です。変数や関数といった基本から、所有権、チェックポイントと巻き戻し、効果、並行処理、入出力、データ形式、数値計算、テスト、実行予算、データベース・通信・GUI まで、25 章と付録で説明します。

読むには、次のどちらかを開いてください。

- [index.html](index.html): 1 つのファイルで完結する HTML。ブラウザーで開けます。
- [REWIND-4.2.0-book.pdf](REWIND-4.2.0-book.pdf): 目次としおりの付いた PDF。

言語の正確な定義が必要なときは、[言語リファレンス](../language-reference.md)（[PDF 版](../language-reference.pdf)）を使ってください。標準ライブラリの各関数は [libraries/README.md](../../libraries/README.md) にあります。この本は、以前の 2.0.0 版にあった全 API の一覧を含みません。

## 構成

- `chapters/*.md`: 本文の原稿（Markdown）。章ごとに 1 ファイルで、`26-appendix.md` が付録です。
- `fixtures/*.json`: GUI の例を、画面なしで実行するための入力イベント。
- `index.html`、`REWIND-4.2.0-book.pdf`: 原稿から生成した成果物です。直接編集しないでください。

## 例の検証と再生成

本文のコード例は、すべて実際に実行して、出力を確かめます。コードブロックの種類は次のとおりです。

| 書き方 | 意味 |
|---|---|
| ` ```rewind ` | 完全なプログラム。終了コード 0 で終わること。直後の ` ```text ` が期待する標準出力 |
| ` ```rewind-error 名前 ` | 失敗すること。標準エラーに `名前` を含むこと |
| ` ```rewind-exit N ` | 終了コード `N` で終わること |
| ` ```rewind stdin="…" args=a,b allow=効果 gui=ファイル http=1 steps=N ` | 標準入力、引数、許可する効果、GUI の入力、HTTP の試験サーバー、実行ステップの上限を指定する |

```sh
cargo build --release
python scripts/check-docs-examples.py docs/book/chapters/*.md
python -m pip install mistune==3.2.0
python scripts/build-book.py
```

`check-docs-examples.py` は、`target/release/rewind` を使います。`--rewind PATH` で別の実行ファイルを指定できます。HTML は Python だけで作れます。PDF の生成には Microsoft Edge または Chrome を使います（`--browser PATH` で指定、`--no-pdf` で省略）。

`language-reference.md` の例も、同じ書き方です。リポジトリのテスト `language_reference_examples_execute_as_documented` が実行します。
