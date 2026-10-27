# REWIND 4.0.0

言語の記法を固め、エディタなどの外部ツールから扱いやすくしたバージョンです。壊れる変更があるので、先に「移行」を読んでください。

## 移行

- 言語 4.0.0 では、`fn`、`let`、`commit`、`revert`、`spawn`、`await` など約 45 語を名前に使えません。使うと `ReservedWord` で拒否され、メッセージが直し方を案内します（`commit` は `committed` など）。一覧は[言語リファレンスの 3.2 節](language-reference.md#32-識別子)にあります。`value`、`capture`、`borrow`、`force`、`live`、`fresh`、`test`、`where`、`effects`、`captures`、`None` は、これまでどおり名前に使えます。
- 言語 4.0.0 では、ソースに `null` と書けません。値がないことは `None` で表します。DB の NULL と JSON の null は、専用の型と関数で扱えます。
- 言語 3.0.0 未満は実行できません（`unsupported language version`）。古いプログラムは REWIND 3 で実行してください。
- 通常の文字列リテラルの中の改行は、ソースの改行コードによらず、どの言語でも LF になります。OS に合わせた改行は、`external { }` の中で `std.text.lineSeparator()` を呼びます。
- `pub effects { ... }` は、これまで `pub` を無視していましたが、拒否するようになりました。
- `binding.revert` を削除しました。`binding.reset` を使ってください。`historyGraph` の `setHead`、`head`、`dropHead` は、引数名が `branch` から `branchName` に変わりました。どちらも `api-diff` では互換性のない変更です。

3.x の言語を指定したプログラムでは、予約語と `null` の 2 点は変わりません。

## 追加された書き方

どれも、これまで拒否していた書き方を受け付けるだけなので、3.x の言語でも使えます。

- 型注釈つきの `let` で短いクロージャを書けます。`let f: fn(Int, Int) -> Int effects {} = |a, b| a + b;` のように、引数の型は注釈から決まります。
- カンマ区切りのすべての箇所で、最後のカンマを書けます。
- raw 文字列を書けます。3 個以上の `"` で開閉し、エスケープも補間も効きません。
- `match` の文でも、`{ }` のアームのあとの `,` を省けます。
- let-else（`let Some(x) = 値 else { ... };`）を、式としてのブロックと、式の `if` の本体にも書けます。
- `match E::A { ... }` と `if x == E::A { ... }` を、括弧で囲まずに書けます。`a < b >= c` は比較の連鎖として読めます。
- `runtime { }` の値に `_` 区切りと 16 進を書けます。

## 追加された library と tool

- `std.text` に `codePoints`、`fromCodePoints`、`lineSeparator` を追加しました。`lineSeparator` は、クロックや乱数と同じ外部の観測として記録され、replay では記録した値を使います。
- `std.binding.reset` を追加しました。`std.guiTabs`、`std.guiCombo`、`std.guiDate`、`std.binding` を、pure な `.rw` で追加しました。
- 形式文法 `docs/grammar/rewind.ebnf` と[説明](grammar/README.md)を追加しました。文法は、実際の構文解析器と突き合わせるテストで検証しています。
- エディタ向けの窓口として、`rewind editor-info`、`rewind tokens`（壊れたソースでも動きます）、LSP の `documentSymbol` と `semanticTokens` を追加しました。作り方は[エディタ作成ガイド](editor-guide.md)にあります。

## 速度と安全性

- VM の命令ごとの固定費を減らしました。100 万回のループは約 10 秒から約 2 秒になりました。構造体のフィールドへの `+=` は、償却 O(1) です。
- 悪意のあるソースへの対策として、1 MiB 以内のソースで、コンパイルに二乗時間がかかる入力（深い入れ子、大量の変数と分岐、巨大なスコープなど）をなくしました。確かめた形はすべて約 14 秒以内に終わります。回帰テストを `tests/hostile_input.rs` と `tests/hostile_runtime.rs` に置いています。
- クロージャを連鎖して作ると二乗時間かかっていたのを、線形にしました。6000 段で約 10 秒かかっていたものが、約 0.2 秒です。
- 認証に失敗した HTTP サーバの要求で、401 の応答が届かないことがあったのを直しました。

詳細は[言語リファレンスの付録 B](language-reference.md#400-の変更)を参照してください。
