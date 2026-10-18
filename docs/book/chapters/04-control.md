## 第4章 分岐と繰り返し

### if と else

条件は `Bool` でなければなりません。数値や文字列を条件に使うことはできません。

```rewind
fn grade(score: Int) -> String effects {} {
    if score >= 80 { return "A"; }
    else if score >= 60 { return "B"; }
    else { return "C"; }
}

fn main()->Int {
    Out.println(grade(90));
    Out.println(grade(65));
    Out.println(grade(10));
    return 0;
}
```

```text
A
B
C
```

この例の `effects {}` は、関数の宣言に付ける「外部とのやりとりをしない」という印です。第5章と第13章で説明するので、いまは「関数にはこう書くことが多い」と思っておいてください。

### 式としての if

REWIND では、`if` も `{ }` のブロックも **値を返せます**。ブロックは、最後の式（末尾に `;` を付けないもの）が値になります。分岐の値の型は、すべてそろっている必要があります。

```rewind
fn main()->Int {
    let n = 5;
    let size = if n > 3 { "big" } else { "small" };
    let doubled = { let t = n * 2; t + 1 };
    Out.println(size);
    Out.println(doubled);
    return 0;
}
```

```text
big
11
```

型がそろっていない分岐は、実行前にエラーになります。

```rewind-error match arm expects
fn main()->Int {
    let x = if true { 1 } else { "a" };
    return 0;
}
```

### while

```rewind
fn main()->Int {
    var i = 0;
    var total = 0;
    while i < 5 {
        i += 1;
        if i == 2 { continue; }   // 次の反復へ
        if i == 5 { break; }      // ループを抜ける
        total += i;
    }
    Out.println(total);
    return 0;
}
```

```text
8
```

`i` が 1、3、4 のときだけ足されるので、合計は 8 です。`break` と `continue` は、いちばん内側のループに働きます。

### for

`for` には二つの使い方があります。

一つは **範囲** です。`a..b` は、`a` 以上 `b` **未満** の整数を表します。`b` は含まれません。`a` が `b` 以上なら、一度も実行されません。

もう一つは、`List` などの **コレクション** の要素を順に取り出す使い方です。`Map` は `keys()` でキーを取り出します。キーは常に昇順です。

```rewind
fn main()->Int {
    for i in 0..3 { Out.println(i); }
    for name in ["a", "b"] { Out.println(name); }

    let ages = Map<String, Int>();
    ages.set("bob", 28);
    ages.set("ann", 31);
    for key in ages.keys() { Out.println(key); }
    return 0;
}
```

```text
0
1
2
a
b
ann
bob
```

### match

`match` は、値の形によって処理を分けます。他の言語の `switch` に似ていますが、値の中身を取り出せる点と、**すべての場合を扱っているかを検査してくれる** 点が違います。

```rewind
fn main()->Int {
    let n = 7;
    match n {
        0..5 => { Out.println("低"); },
        5..10 => { Out.println("中"); },
        _ => { Out.println("高"); }
    }
    let word = match n { 1 => "one", 2 => "two", _ => "many" };
    Out.println(word);
    return 0;
}
```

```text
中
many
```

整数や文字列の `match` では、残りのすべてを受ける `_` が必要です。`match` は式なので、上の `word` のように値を返せます。

値の中身を取り出すパターンは、第7章で `enum` と一緒に学びます。

### if let と let … else

ある形のときだけ処理したい場合は、`match` より `if let` が簡潔です。

```rewind
fn main()->Int {
    let found: Option<Int> = Some(3);
    if let Some(v) = found {
        Out.println(v);
    } else {
        Out.println("なし");
    }
    return 0;
}
```

```text
3
```

`Option<Int>` は「`Int` がある（`Some`）か、ない（`None`）か」を表す型です。詳しくは第10章で扱います。

形が合わなければ関数から抜けたい、という場面には `let … else` が便利です。`else` の中は、`return`、`break`、`continue`、`panic` のどれかで終わらなければなりません。一致したときは、取り出した名前が後ろの文で使えます。

```rewind
fn firstOrZero(m: &Map<String, Int>) -> Int effects {} {
    let Some(v) = m.get("first") else { return 0; };
    return v;
}

fn main()->Int {
    let m = Map<String, Int>();
    Out.println(firstOrZero(&m));
    m.set("first", 9);
    Out.println(firstOrZero(&m));
    return 0;
}
```

```text
0
9
```

引数の `&Map<String, Int>` は「読むだけで借りる」という意味です。第9章で説明します。
