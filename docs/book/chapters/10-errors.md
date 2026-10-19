## 第10章 エラーを扱う

### 二種類の失敗

REWIND の失敗は、扱い方で二つに分かれます。

- **回復できる失敗**: 入力の誤りや、見つからない値など、呼び出し側が選んで処理すべきもの。`Option` や `Result` で返します。
- **致命的な失敗**: プログラムの誤りや、資源の限度超過など。`panic`、`assert` の失敗、リストの範囲外アクセス、ゼロ除算、整数のオーバーフロー、実行予算の超過がこれにあたります。実行が止まり、診断が表示されます。

「ユーザーが入力を間違えた」は前者、「プログラムのつじつまが合わなくなった」は後者です。前者を致命的な失敗にしてしまうと、呼び出し側は再入力を促すこともできません。

### Option: あるか、ないか

`Option<T>` は、`Some(値)` か `None` のどちらかです。値が無いかもしれない場面で使います。`Map.get` や `List.pop` が返すのも `Option` です。

取り出すときは、`match`、`if let`、`let … else` を使います。失敗を無視して取り出す `unwrap` のようなメソッドはありません。「無いかもしれない」ことを、必ず書き手が処理することになります。

標準ライブラリの `std.option` には、無いときの既定値を指定して取り出す `valueOr` があります。

```rewind
import std.option as option;

fn main()->Int {
    let missing: Option<Int> = None;
    Out.println(option.valueOr(missing, 7));
    Out.println(option.valueOr(Some(3), 7));
    return 0;
}
```

```text
7
3
```

### Result: 成功か、失敗か

`Result<T,E>` は、成功の `Ok(値)` か、失敗の `Err(エラー)` のどちらかです。文字列を整数にする `parseInt` のように、失敗しうる処理が返します。

```rewind
fn main()->Int {
    match "12".parseInt() {
        Ok(n) => { Out.println(n + 1); },
        Err(e) => { Out.println(e); }
    }
    match "x".parseInt() {
        Ok(n) => { Out.println(n + 1); },
        Err(e) => { Out.println(e); }
    }
    return 0;
}
```

```text
13
InvalidIntOrOverflow
```

### ? で失敗を呼び出し元へ返す

`Result` を返す関数の中で、`式?` と書くと、`Ok(v)` なら `v` を取り出し、`Err(e)` ならその場で関数から `Err(e)` を返します。失敗を伝えるだけの定型の処理を、一文字で書けます。

```rewind
fn parse(text: String) -> Result<Int, String> effects {} {
    return text.parseInt();
}

fn sum(a: String, b: String) -> Result<Int, String> effects {} {
    let x = parse(a)?;
    let y = parse(b)?;
    return Ok(x + y);
}

fn main()->Int {
    Out.println(sum("1", "2"));
    Out.println(sum("1", "x"));
    match sum("4", "5") {
        Ok(v) => { Out.println(v); },
        Err(e) => { Out.println(e); }
    }
    return 0;
}
```

```text
Ok(3)
Err(InvalidIntOrOverflow)
9
```

`?` は `Option` にも使えます。`Option` を返す関数の中で、`Some(v)` なら `v` を取り出し、`None` ならその関数から `None` を返します。

```rewind
fn firstEven(xs: &List<Int>) -> Option<Int> effects {} {
    for i in 0..xs.len() {
        if xs.get(i) % 2 == 0 { return Some(xs.get(i)); }
    }
    return None;
}

fn doubled(xs: &List<Int>) -> Option<Int> effects {} {
    let n = firstEven(xs)?;
    return Some(n * 2);
}

fn main()->Int {
    let a = [1, 3, 4];
    let b = [1, 3];
    Out.println(doubled(&a));
    Out.println(doubled(&b));
    return 0;
}
```

```text
Some(8)
None
```

`Result` を返す関数の中では、`Option` に `?` を付けられません。`match` か `let … else` で扱ってください。

`?` を使う関数では、`Err` の型が、戻り値の `Err` の型と一致している必要があります。違うときは、`mapErr` で型を変えてから `?` を付けます。

```rewind
fn lookup(key: String) -> Result<Int, String> effects {} {
    if key == "a" { return Ok(1); }
    return Err("missing");
}

fn code(key: String) -> Result<Int, Int> effects {} {
    let v = lookup(key).mapErr(|e: String| -> Int { return e.byteLen(); })?;
    return Ok(v);
}

fn main()->Int {
    Out.println(code("a"));
    Out.println(code("b"));
    return 0;
}
```

```text
Ok(1)
Err(7)
```

`Option` と `Result` には、中身を取り出さずに状態を調べるメソッドと、既定値を指定して取り出すメソッドもあります。`isSome()`、`isNone()`、`isOk()`、`isErr()`、`valueOr(既定値)` です。

```rewind
fn main()->Int {
    let missing: Option<Int> = None;
    let failed: Result<Int, String> = Err("bad");
    Out.println(missing.isNone());
    Out.println(missing.valueOr(7));
    Out.println(failed.isErr());
    Out.println(failed.valueOr(8));
    return 0;
}
```

```text
true
7
true
8
```

`Result` には、他に `map`（`Ok` の中身を変換）、`andThen`（`Ok` の中身から次の `Result` を作る）、`flatten`（入れ子を一つにする）があります。

エラーの型が組み込みの型どうしで食い違っていると、実行前に拒否されます。

```rewind-error does not match
fn code(key: String) -> Result<Int, Int> effects {} {
    let v = key.parseInt()?;      // エラーの型は String、この関数は Int
    return Ok(v);
}

fn main()->Int {
    Out.println(code("1"));
    return 0;
}
```

`record` や標準ライブラリの型が絡む不一致は、実際に `Err` の経路を通ったときの実行時エラーになります。いずれにしても、`mapErr` で型をそろえる書き方を習慣にしておくと安全です。

### panic と assert

`panic` と `assert` は、「ここに来たらプログラムの誤り」という前提を表すためのものです。

| 呼び出し | 働き |
|---|---|
| `panic("メッセージ")` | 実行を止める |
| `assert(条件)` | 偽なら `AssertionFailed` で止まる |
| `assert_eq(a, b)` | 等しくなければ `AssertionFailed` で止まる（左右の値が表示される） |

`assert_eq` の等しさは、`==` と同じ規則です。`List` どうしは、中身が同じでも別々の値なので、そのままでは失敗します。比べたいときは `freeze` します。

利用者の入力のように、起こりうる誤りには、`panic` や `assert` ではなく `Result` を使ってください。なお、`main` の途中で止まったときは、それまでに溜まっていた保留中の出力は外へ出ません。

### テストを書く

`test fn` で書いた関数は、通常の実行では呼ばれず、`rewind test` で実行されます。第22章で詳しく扱いますが、書き方だけ見ておきましょう。

```rewind
fn double(x: Int) -> Int = x * 2;

test fn doubles() {
    assert_eq(double(4), 8);
}

fn main()->Int {
    Out.println(double(21));
    return 0;
}
```

```text
42
```
