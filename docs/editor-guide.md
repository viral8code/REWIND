# REWIND 専用エディタの作り方（簡易ガイド）

REWIND の専用エディタを、別のプロジェクトとして作るための資料です。エディタ側で決めることと、REWIND 側が出す窓口を、順に説明します。

## 1. 役割分担

エディタの見た目と操作はエディタ側の仕事です。言語の事実（語、記号、型、診断）を機械が読める形で出すのは、REWIND 側の仕事です。エディタが自前で言語を解釈し直さないようにすると、コンパイラとエディタの判断が食い違いません。C# と Visual Studio も、同じ分け方をしています。

| 側 | 担当 |
|---|---|
| REWIND（コンパイラ） | 語の一覧、トークンの分類、アウトライン、診断、補完、定義へ移動、整形、名前の変更 |
| エディタ | 表示、入力、タブとペイン、色の選び方、REWIND 固有の画面（履歴、コミットとリバートの図、リプレイ） |

## 2. おすすめの構成

Tauri（Rust の裏側 + Web の表示）と、Monaco か CodeMirror 6 の組み合わせをおすすめします。

- 補完のポップアップ、波線、ホバー、複数カーソル、検索置換は、Monaco に最初からあります。
- 日本語入力（変換中の文字、候補の位置、サロゲートペア）は、ブラウザの編集部品が解決済みです。自前の編集部品では、ここに時間がかかります。
- 重い処理は Rust 側が受け持ちます。編集部品は、Monaco 自体が大きなファイルに耐えます。
- 履歴や分岐の図のように、REWIND 固有の画面は、Web の技術で作りやすいです。

最速を優先して Rust だけで作る案（iced、Slint など）もあります。ただし、編集部品と IME を自作する負担が最も大きくなります。

## 3. REWIND が出す窓口

境界はプロセスの外側です。REWIND の `rewind` コマンドを、エディタが子プロセスとして呼びます。コンパイラは、ライブラリ（crate）としては公開していません。

### 3.1 言語の事実: `rewind editor-info`

語、記号、コメントや文字列の書き方、対応する言語の版、診断と LSP の呼び方を、JSON で返します。構文の色付けの定義（TextMate 文法など）を作るときの元データにしてください。

```bash
rewind editor-info
```

主な項目は次のとおりです。

| 項目 | 内容 |
|---|---|
| `keywords` | 言語 4.0.0 の予約語。名前には使えない |
| `contextualKeywords` | 決まった位置でだけ意味を持つ語。ほかの位置では名前として使える |
| `literalWords` | `true`、`false`、`None` |
| `removedWords` | 4.0.0 で書けなくなった語（`null`） |
| `operators`、`punctuation` | 演算子と区切りの記号 |
| `comments`、`strings`、`numbers` | コメント、文字列（raw 文字列を含む）、数の書き方 |
| `tokenKinds`、`symbolKinds` | 次の節のトークンの種類と、アウトラインの種類 |
| `languages` | この版が実行できる言語の版 |

### 3.2 トークンとアウトライン: `rewind tokens`

ソースのトークンを種類つきで返します。標準入力から読むには、ファイル名の代わりに `-` を渡します。

```bash
rewind tokens main.rw
```

```bash
echo 'fn main() -> Int { return 0; }' | rewind tokens -
```

返す JSON は、`tokens`（トークンの列）と `symbols`（宣言の一覧）を持ちます。

```json
{
  "format": 1,
  "tokens": [
    {"kind": "keyword", "line": 1, "column": 1, "endLine": 1, "endColumn": 3}
  ],
  "symbols": [
    {"name": "main", "kind": "function", "line": 1, "column": 4, "endLine": 1, "endColumn": 8}
  ]
}
```

- 種類は `keyword`、`literal`、`name`、`type_name`（大文字で始まる名前）、`number`、`string`、`raw_string`、`comment`、`operator`、`punctuation`、`unknown` です。
- 閉じていない文字列やコメントは、トークンに `error` が付きます。入力の途中でも、残りは分類されます。コンパイラの字句解析は最初の誤りで止まりますが、この分類は止まりません。
- 位置は 1 始まりで、列は文字の数です。終わりの位置は、そのトークンの次の文字を指します。
- 宣言は `function`、`method`（impl と trait の中の関数）、`struct`、`enum`、`trait`、`impl`、`const`、`type` です。コンパイルが通らないファイルでも出ます。
- ソースは 1 MiB までで、UTF-8 だけを受け付けます。

### 3.3 診断: `--diagnostic-format json`

```bash
rewind --diagnostic-format json check main.rw
```

誤りがあると、標準エラーに JSON を 1 行で出し、終了コードは 65 です。`diagnostic` に `code`、`message`、`source`、`line`、`column` が入ります。`hints`（直し方の案）と `related`（関連する位置）が付くこともあります。

- 診断は最初の 1 件だけです。直すたびに再実行して、次の誤りを見ます。
- 診断には範囲の終わりがありません。エディタは、その位置にある語（トークン）の長さを使って、波線を引いてください。
- 4.0.0 の予約語を名前に使うと、`ReservedWord` になります。

### 3.4 言語サーバ: `rewind lsp`

標準入力と標準出力で、Language Server Protocol を話します。

```bash
rewind lsp --root DIR
```

対応する要求は、診断、ホバー、補完、定義へ移動、参照、名前の変更、シグネチャヘルプ、整形、コードアクション、アウトライン（`textDocument/documentSymbol`）、構文の色付け（`textDocument/semanticTokens/full`）です。

- 色付けのトークンの種類は、`initialize` の応答の `legend` に入っています。順に `keyword`、`comment`、`string`、`number`、`operator`、`type`、`variable` です。
- LSP の列は、仕様どおり UTF-16 のコード単位です。`rewind tokens` と診断の列は文字の数なので、混ぜて使わないでください。
- Monaco は LSP を直接話しません。Tauri の裏側（Rust）で `rewind lsp` を子プロセスとして起動し、メッセージを仲介してください。

## 4. 作る順番

1. `rewind tokens` と `rewind editor-info` で、構文の色付けを作る。Monaco では、`editor-info` の語から、モナーク形式のトークナイザを組み立てられます。
2. ファイルを保存するたびに `--diagnostic-format json check` を呼び、波線を出す。
3. `rewind lsp` をつなぎ、補完、ホバー、定義へ移動、アウトラインを出す。
4. REWIND 固有の画面（履歴の分岐、コミットとリバートの図、リプレイ）を作る。ここがエディタを別に作る意味です。

## 5. 守ってほしい点

- 色付けでは、`contextualKeywords` の語を予約語として色づけしないでください。名前としても使えます。
- 言語は版で変わります。`rewind.toml` の `language` を読んで、`removedWords` や予約語の警告を出すかどうかを決めてください。3.x の言語では、予約語を名前に使えます。
- 文法の正式な定義は [docs/grammar/rewind.ebnf](grammar/rewind.ebnf) です。自前で構文解析をするときの元にできます。実装の癖の一覧は [docs/grammar/README.md](grammar/README.md) にあります。
- `rewind` の出力形式（`format` の値）が変わるときは、互換性のない変更として、版の更新のときに案内します。
