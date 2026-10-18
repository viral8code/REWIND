## 第18章 文字列・Unicode・正規表現

### 「一文字」の数え方は一つではない

文字列の長さや位置を扱うとき、REWIND は「何を単位に数えるか」を、メソッドごとにはっきり分けています。日本語や絵文字を含む文字列では、この違いが結果に表れます。

| 操作 | 位置・長さの単位 |
|---|---|
| `s.byteLen()`、`s.find(t)`、`s.slice(a, b)` | UTF-8 のバイト |
| `s.charLen()`、`std.text` の `length`、`slice` | Unicode のスカラー値 |
| `s.utf16Len()` | UTF-16 のコード単位 |
| `std.unicode` の `grapheme…` | 書記素クラスタ（人が見る一文字） |
| `std.regex` の範囲 | UTF-8 のバイト位置 |

```rewind
import std.text as text;

fn main()->Int {
    let s = "a😀界";
    Out.println(s.byteLen());
    Out.println(s.charLen());
    Out.println(s.utf16Len());
    Out.println(text.slice(s, 1, 3));    // スカラー値の 1 以上 3 未満
    Out.println(s.slice(1, 5));          // バイト位置の 1 以上 5 未満
    return 0;
}
```

```text
8
3
4
Ok(😀界)
Ok(😀)
```

`"a😀界"` は、バイトで数えると 8（`a` が 1、`😀` が 4、`界` が 3）、スカラー値では 3、UTF-16 では 4 です。バイトの位置で文字の途中を指すと、`slice` は `Err` を返します。

単位を混ぜると、日本語や絵文字で不具合が出ます。たとえば、正規表現が返したバイト位置を、スカラー値の位置を受け取る `std.text.slice` へそのまま渡すと、別の場所を切り出してしまいます。単位を、関数名や変数名、ドキュメントに書き残す習慣をつけましょう。

### 書記素と正規化

目に見える一文字が、複数のスカラー値でできていることがあります。「e」に結合アクセントを重ねた文字や、国旗、家族の絵文字がそうです。人が見る一文字の単位で扱いたいとき（たとえば入力欄の Backspace）には、`std.unicode` の書記素クラスタを使います。

```rewind
import std.unicode as unicode;

fn main()->Int {
    let source = "é🇯🇵👩‍👩‍👧‍👦";
    Out.println(unicode.graphemeCount(&source));
    Out.println(unicode.graphemeSlice(&source, 1, 2));    // 2 つ目の一文字
    let word = "straße";
    Out.println(unicode.upper(&word));
    return 0;
}
```

```text
Ok(3)
Ok(🇯🇵)
Ok(STRASSE)
```

見た目が同じ文字列でも、内部のスカラー値の並びが違うことがあります（「é」は 1 つの文字としても、「e」と結合記号の 2 つとしても表せます）。これを同じとみなして比べたいときは、正規化形（`NFC`、`NFD`、`NFKC`、`NFKD`）を指定して正規化します。`String` や `Map` のキーが、自動で正規化されることはありません。

互換正規化（`NFKC`、`NFKD`）は、見た目や意味の違う文字も同じにしてしまうことがあります。利用者が入力した原文を、そのまま書き換えてよいかどうかは、用途で判断してください。

### 正規表現

`std.regex` は、正規表現を扱います。`compile` で作った `Regex` は、何度でも使い回せます。

```rewind
import std.regex as regex;

fn show(r: Result<String, StdError>) -> Unit effects {output} {
    match r {
        Ok(s) => { Out.println(s); },
        Err(e) => { Out.println(e.code); }
    }
}

fn main()->Int effects {output} {
    match regex.compile("(?P<name>[A-Za-z]+)-([0-9]+)") {
        Err(_) => { Out.println("正規表現が不正です"); },
        Ok(pattern) => {
            let input = "Alice-42 Bob-7";
            match regex.find(&pattern, &input) {
                Ok(Some(found)) => {
                    show(regex.text(&input, regex.span(&found)));       // 全体
                    match regex.group(&found, 1) {                      // 1 番目のグループ
                        Ok(Some(sp)) => { show(regex.text(&input, sp)); },
                        _ => {}
                    }
                    match regex.next(&found) {                          // 続きを探す位置
                        Some(cursor) => {
                            match regex.findFrom(&pattern, &input, cursor) {
                                Ok(Some(second)) => { show(regex.text(&input, regex.span(&second))); },
                                _ => {}
                            }
                        },
                        None => {}
                    }
                },
                _ => { Out.println("見つからない"); }
            }
        }
    }
    return 0;
}
```

```text
Alice-42
Alice
Bob-7
```

`find` は、見つからなければ `Ok(None)` を返します。一致の範囲は、**バイト位置** の組で表され、`regex.text` で文字列に取り出します。続きを探すときは、`regex.next` が返す位置を `findFrom` に渡します。

- 対応する文法は、API の文書に従います。他の言語の正規表現と同じとは限りません。任意の後方参照や先読みを前提にしないでください。
- 正規表現と入力には、容量と処理量の予算があります（第23章）。
- 正規化した文字列で得た位置を、元の文字列で使うと、バイト列が変わっているため間違います。原文を表示する検索では、どの文字列の位置なのかを、はっきりさせてください。
