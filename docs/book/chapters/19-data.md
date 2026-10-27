## 第19章 JSON と CSV

設定ファイルや外部とのやりとりでは、JSON や CSV のような形式のデータを読み書きします。この章では、標準ライブラリの `std.json` と `std.csv` の使い方と、外から来るデータを扱うときの考え方を説明します。

### JSON を読む

`json.parse` は、文字列を JSON の値に変換します。結果は `Result` で、JSON として正しくない文字列は `Err` になります。JSON の値は `Json` 型で、`json.get` でオブジェクトのキーを、`json.integer`、`json.text`、`json.boolean`、`json.floating`、`json.array` で値を取り出します。取り出すメソッドは、型が違うときや、キーが無いときに `None` を返します。

次の例は、プレイヤーの得点を集計します。`score` が無い項目は 0 点として数えます。

```rewind
import std.json as json;

fn scoreOf(item: Json) -> Int effects {} {
    match json.get(item, "score") {
        Some(s) => {
            match json.integer(s) {
                Some(n) => { return n; },
                None => { return 0; }
            }
        },
        None => { return 0; }
    }
}

fn main()->Int {
    let text = "{\"title\":\"結果\",\"players\":[{\"score\":42},{\"score\":8},{\"name\":\"x\"}]}";
    match json.parse(text) {
        Err(e) => { Out.println("JSON が不正: " + e.code); },
        Ok(root) => {
            match json.get(root, "players") {
                None => { Out.println("players がない"); },
                Some(list) => {
                    match json.array(list) {
                        None => { Out.println("players が配列ではない"); },
                        Some(items) => {
                            var total = 0;
                            for item in items { total += scoreOf(item); }
                            Out.println("${items.len()} 人、合計 ${total}");
                        }
                    }
                }
            }
        }
    }
    match json.parse("{oops") {
        Ok(_) => { Out.println("?"); },
        Err(e) => { Out.println(e.code); }
    }
    return 0;
}
```

```text
3 人、合計 50
Syntax
```

`match` が深く入れ子になっているのは、「ある」「型が合う」「配列である」という確認を、一つずつ明示しているからです。長くなるときは、上の `scoreOf` のように小さな関数へ分けるか、第10章の `?` が使える `Result` を返す関数にまとめてください。

> **ポイント**: JSON として正しいことと、アプリケーションが期待する形をしていることは、別の問題です。`parse` が成功したあとで、必要なキーがあるか、型が合っているかを、必ず検査してください。`json.integer` は、`Int` の値だけを返し、数字に見える文字列を自動で変換しません。

`json.stringify` は、`Json` の値を文字列に戻します。オブジェクトのキーは、並び順が決まった状態で出力されます。

### 大きな JSON を逐次的に読む

数十 MB の JSON を、いったん丸ごと読み込むと、その分のメモリを使います。`std.jsonStream` は、文字列を少しずつ与えながら、「オブジェクトの開始」「キー」「値」といったイベントとして順に取り出します。必要な部分だけを処理して捨てていけば、メモリの量を抑えられます。数値の token を、どの正確な型に変換するかは、利用側が別に決めます。

### CSV を読む

`csv.parse` は、バイト列を表として読みます。引用符で囲まれたセルに含まれる区切りや改行も、正しく扱われます。`csv.rows` が行数、`csv.get(&表, 行, 列)` がセルを返します。範囲外は `Err` です。

```rewind
import std.csv as csv;

fn main()->Int {
    let source = "name,score\nAlice,42\n\"Bob, Jr.\",8\n";
    match csv.parse(Bytes(source)) {
        Err(e) => { Out.println(e.code); },
        Ok(table) => {
            var total = 0;
            for row in 1..csv.rows(&table) {          // 0 行目は見出し
                match csv.get(&table, row, 0) {
                    Ok(name) => { Out.println(name); },
                    Err(_) => {}
                }
                match csv.get(&table, row, 1) {
                    Ok(cell) => {
                        match cell.parseInt() {
                            Ok(n) => { total += n; },
                            Err(_) => {}
                        }
                    },
                    Err(_) => {}
                }
            }
            Out.println(total);
            match csv.get(&table, 9, 0) {
                Ok(_) => {},
                Err(e) => { Out.println(e.code); }
            }
        }
    }
    return 0;
}
```

```text
Alice
Bob, Jr.
50
Range
```

カンマで単純に `split` するだけの処理は、一般的な CSV の読み込みではありません。引用符の中のカンマや改行、`""` による引用符のエスケープに対応できないからです。CSV は必ず `std.csv`（大きなデータには増分処理の `std.csvStream`）で読んでください。

### 正確な値を保存する

`BigInt` や `Decimal` を `Float` に変換してから JSON の数値として保存すると、精度を失うことがあります。そのような値は、それぞれの型が用意している文字列表現で、往復させてください。日時も、タイムゾーンと精度を明示して保存します。次の章で、これらの型を学びます。

`std.config`、`std.path`、`std.args` は、設定、パス、引数の処理を補助します。パスを組み立てたことが、`File` が持つ「root の外には触れない」という制限を、取り払うわけではありません。パスの検査と、ファイルアクセスの権限は、別の仕組みです。
