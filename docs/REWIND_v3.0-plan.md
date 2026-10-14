# REWIND v3.0：書きやすさと検査の厳格化

状態：実装中（`codex/develop`）。言語バージョン`3.0.0`は追加済みですが、crate版（`Cargo.toml`）は2.0.0のままです。Release、版更新、標準ライブラリの言語切替は、版上げ時にまとめて行います。

## 方針

- `language = "3.0.0"`は新しいバージョンとして追加し、2.0.0以前の挙動は変えません。
- 構文はJava/C#に近い書き心地へ寄せ、所有権・借用・`Frozen`・checkpointのモデルは維持します。
- 実行前の検査を厳しくし、検査を通ったプログラムが型や所有権が原因で実行時に落ちないことを目標にします。
- すべての機能は`tests/language_v300.rs`で確認します。

## 実装済み

| 機能 | 書き方 | 備考 |
|---|---|---|
| トップレベル文の禁止 | 宣言（`fn`、`struct`、`enum`、`record`、`const`、`import`）だけを書く | 実行コードは関数へ。`runtime {…}`は許可 |
| `fn main`起点 | `fn main()->Int effects {…} { … }` | `rewind run`でmainが無いと実行前に失敗 |
| ブロック式 | `let x = { let a = 1; a + 1 };` | 最後の式（`;`なし）が値 |
| `if`式 | `let s = if n > 0 { "pos" } else if n < 0 { "neg" } else { "zero" };` | `else`省略はUnitのブロック。内部はBoolの`match` |
| `match`アームのブロック | `Err(e) => { return Err(e); }` | `return`/`break`/`?`/複数文を書ける。アーム型は`Never`を許容して統一 |
| `let … else` | `let Some(v) = m.get("a") else { return …; };` | else節は`return`/`break`/`continue`/`panic`で終わる必要がある。以降の文がマッチ側になる |
| `if let` | `if let Some(v) = x { … } else { … }` | 文と式の両方 |
| コレクションリテラル | `[1, 2, 3]`、`{"a": 1}` | 要素型は最初の要素で決まり、型の違う要素は拒否。空は`List<T>()`。`{k: v}`は`{`の次が`:`で判別 |
| 添字 | `xs[i]`、`xs[i] = v`、`xs[i] += 1`、`m[k]` | `get`/`set`への糖衣。`m[k]`は`Option<V>`（`get`と同じ） |
| タプル要素 | `pair.0`、`t.1.0` | 内部は`_0`、`_1`フィールド |
| 文字列補間 | `"${name}: ${n + 1}"` | 各式を`.format()`して連結。`$${`で`${`。`${}`内に文字列リテラルは書けない |
| 式本体の関数 | `fn double(x:Int)->Int = x*2;` | 戻り型が必須 |
| 総称enumの明示 | `Result<…>`ではなく利用者のenum `RowMapError<E>::TooManyRows(n)` | 型引数を推論できない変種で使う |
| `while true`の網羅性 | `break`が無い`while true`の後に`return`は不要 | 2.0.0以前でも許可（緩和のみ） |

`.format()`は`Int`、`Float`、`Bool`、`String`で使えます。

## 互換性の注意

- ブロック式、`if`式、`[ ]`と`{k: v}`のリテラルは内部でブロック式になるため、`language < 3.0.0`では型検査で拒否されます（`block and if expressions require language 3.0.0`）。
- 文字列補間`${…}`は3.0.0でだけ展開されます。旧バージョンでは通常の文字列のままです。
- `.0`、添字、`let … else`、`if let`、式本体の関数は、旧バージョンでも読み込めますが、3.0.0の機能として扱ってください。
- `libraries/std`は2.0.0のまま、3.0.0の検査で91件すべて合格することを確認済みです。

## 実装で直した既存の不具合

- Send境界を持つ総称recordを受け付けない（`Frozen<T>`を含む`Snapshot<T:Send>`）。
- 文字列の`<`、`<=`、`>`、`>=`が実行時に使えない（UTF-8バイト順）。
- 明示した型引数（`Name<E>(…)`）が推論できない型引数の補完に使われない。
- `Enum<T>::Variant`の名前解決。
- 総称関数内のクロージャの型引数が効果解析で解決されない。クロージャが定義元モジュールの可視性で検査されない。
- モジュール別名付きの型（`snapshot.Snapshot<T>`）と`Map<String, mod.Type>()`の名前解決。
- 空のコレクションなど、要素型が実行時に分からない値を総称コンテナ型へ渡せない。

## 未実装・検討中

- 暗黙の`publish`：`main`が正常に戻った時点と、ブロックする入力待ちの直前で自動publishし、明示`publish;`も残す案。パニック時はpublishしない。
- 型推論つきの短いクロージャ（`|a, b| a == b`）。
- `Frozen`の読み取りで`thaw`を不要にする。
- `.unwrap()`、`.expect("msg")`。
- 借用由来のコレクション（`keys()`など）の自動コピー。
- `import std.text;`の別名省略（旧バージョンの「名前を直接取り込む」挙動と衝突するため保留）。
- 言語リファレンスとbookへの反映、標準ライブラリの3.0.0への切替（版上げ時）。

## 検証

```sh
cargo test --test language_v300
./target/release/rewind test libraries/std/tests.rw --root libraries/std
```

標準ライブラリの全テストは1件約3〜5秒（releaseビルド）です。単独で確認するときは`--filter 名前`を使います。
