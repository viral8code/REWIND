## 第6章 List・Map・イテレータ

### List

`List<T>` は、要素の型が `T` の、長さを変えられる列です。添字は 0 から始まります。`[ ]` で作れます。空の `List` は、型を明示して `List<Int>()` と書きます。

```rewind
fn main()->Int {
    let values = List<Int>();
    values.add(3);
    values.add(5);
    values.add(7);
    values.set(1, 9);
    Out.println(values.len());
    Out.println(values[1]);        // values.get(1) と同じ
    Out.println(values.pop());     // 末尾を取り出す。空なら None
    var total = 0;
    for v in values { total += v; }
    Out.println(total);
    return 0;
}
```

```text
3
9
Some(7)
12
```

添字が範囲の外を指すと、`IndexOutOfBounds` で止まります。`None` が返るわけではありません。入力から得た添字を使うときは、先に `len()` と比べてください。要素の型が合っていることと、添字が正しいことは別の問題です。

要素の型は最初の要素で決まり、型の違う要素を混ぜると拒否されます。入れ子にもできます（`[[1, 2], [3]]`）。

### Map

`Map<K,V>` は、キーと値の対応です。値は `get` で取り出し、結果は `Option` です。キーが無ければ `None` になります。

```rewind
fn main()->Int {
    let ages = {"ann": 31, "bob": 28};
    Out.println(ages.get("ann"));
    Out.println(ages.get("zed"));
    ages.set("zed", 40);
    ages.remove("bob");
    Out.println(ages.len());
    for key in ages.keys() { Out.println(key); }   // キーは昇順
    return 0;
}
```

```text
Some(31)
None
2
ann
zed
```

`ages["ann"]` と書いても `ages.get("ann")` と同じです。`Map` のキーは、必ず並び順が決まります。そのため、どの環境で動かしても反復の順序が変わらず、結果が再現できます。

### 変更できる値には持ち主がいる

`List` や `Map` は、中身を後から変えられる値です。REWIND では、このような値をむやみにコピーせず、「いま誰が持っているか」を言語が追跡します。たとえば次のようには書けません。

```rewind-error mutable owner alias
fn main()->Int {
    let a = [1];
    let b = a;      // 同じ List を二か所から持てない
    return 0;
}
```

これが、第9章で説明する **所有権** の考え方です。いまは「List はこう代入できない」とだけ覚えておけば、先へ進めます。

### イテレータ

`xs.iter()` は、`List` の **その時点の内容** を順に取り出すイテレータを作ります。元の `List` を後から変更しても、作ったイテレータには影響しません。

| メソッド | 働き |
|---|---|
| `map(f)` | 各要素を `f` で変換する |
| `filter(f)` | `f` が `true` を返す要素だけ残す |
| `fold(初期値, f)` | 畳み込む |
| `take(n)` | 先頭の `n` 個だけにする |
| `enumerate()` | `(添字, 要素)` のタプルにする |
| `zip(他)` | 二つの列を組にする |
| `collect()` | `List` に集める |
| `next()` | 次の要素を `Option` で返す |

```rewind
fn main()->Int {
    let xs = [1, 2, 3, 4];
    let evens = xs.iter()
        .filter(|n: Int| -> Bool { return n % 2 == 0; })
        .map(|n: Int| -> Int { return n * 10; })
        .collect();
    Out.println(evens.len());
    Out.println(evens.get(1));
    Out.println(xs.iter().fold(0, |a: Int, b: Int| -> Int { return a + b; }));
    return 0;
}
```

```text
2
40
10
```

イテレータに渡す関数は、すぐに実行される同期的なものでなければなりません。タスクを作る関数や `publish` を伴う関数は渡せません。

### タプル

`(a, b, c)` は、型の違う値をひとまとめにするタプルです。要素は `.0`、`.1` で読みます。`let` で分解して受け取ることもできます。複数の値を返す関数によく使います。

```rewind
fn minmax(a: Int, b: Int) -> (Int, Int) effects {} {
    if a < b { return (a, b); }
    return (b, a);
}

fn main()->Int {
    let (low, high) = minmax(9, 4);
    let pair = minmax(1, 2);
    Out.println(low);
    Out.println(high);
    Out.println(pair.1);
    return 0;
}
```

```text
4
9
2
```
