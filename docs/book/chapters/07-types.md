## 第7章 自分の型を定義する

REWIND で自分の型を作る方法は、`struct`、`record`、`enum` の三つです。最初に、`struct` と `record` の違いを押さえましょう。

| | `struct` | `record` |
|---|---|---|
| フィールドの書き換え | できる | できない |
| コピー | できない（持ち主が一人） | 自由にできる |
| `==` | 同じ一つの値か | **中身が等しいか** |
| 向いているもの | 状態を持つ、変化していくもの | 設定、座標、メッセージなど、値そのもの |

### record: 変わらないデータ

`record` は、作った後に変えられないデータです。値は、フィールドの宣言順に引数を渡して作ります。

```rewind
record Point { x: Int, y: Int }

fn main()->Int {
    let p = Point(3, 4);
    let q = p;                    // コピーしてよい
    Out.println(p.x + q.y);
    Out.println(p == Point(3, 4));
    return 0;
}
```

```text
7
true
```

フィールドを書き換えようとすると、実行前にエラーになります。

```rewind-error record fields are immutable
record Point { x: Int, y: Int }

fn main()->Int {
    let p = Point(3, 4);
    p.x = 1;
    return 0;
}
```

変更したいときは、新しい値を作ります。`record` は小さくて安いので、作り直しを恐れる必要はありません。

`record` のフィールドの型は、コピーして共有できる型（`Int`、`String`、他の `record` など）に限られます。`List` のような変更できる値を持たせたいときは、`freeze` した `Frozen<List<T>>` にします。

```rewind
record Config { names: Frozen<List<String>>, limit: Int }

fn main()->Int {
    let c = Config(freeze(["a", "b"]), 3);
    Out.println(c.names.len());
    return 0;
}
```

```text
2
```

`freeze` は第9章で説明します。

### struct: 変えられるデータ

`struct` は、フィールドを後から書き換えられます。

```rewind
struct Counter { value: Int }

fn main()->Int {
    let c = Counter(1);
    c.value = 2;
    c.value += 5;
    Out.println(c.value);
    return 0;
}
```

```text
7
```

`let c` と書いていても、フィールドは更新できます。`let` が禁じるのは `c` に別の値を代入し直すことだけで、`c` の中身の更新とは別の話です。`struct` は `List` と同じく **持ち主が一人の値** で、コピーできません。第9章で、その扱い方を学びます。

> **メモ**: どちらを選ぶか迷ったときは、`record` から始めてください。変化していく状態が必要になったときに `struct` を選べば十分です。

### enum: 選択肢を表す

`enum` は、「いくつかの形のうちのどれか」を表す型です。それぞれの形を **variant** と呼びます。variant は、値を持たないもの、位置で値を持つもの、名前付きのフィールドを持つものを、混ぜて宣言できます。値は `型名::variant` で作ります。

```rewind
enum Message {
    Quit,
    Write(String),
    Move{x: Int, y: Int}
}

fn describe(m: Message) -> String effects {} {
    match m {
        Message::Quit => { return "quit"; },
        Message::Write(text) => { return "write " + text; },
        Message::Move{x: a, y: b} => { return "move ${a},${b}"; }
    }
}

fn main()->Int {
    Out.println(describe(Message::Quit));
    Out.println(describe(Message::Write("hi")));
    Out.println(describe(Message::Move{x: 1, y: 2}));
    return 0;
}
```

```text
quit
write hi
move 1,2
```

### match で網羅を検査する

`enum` に対する `match` は、**すべての variant を扱わなければなりません**。1 つでも漏れていると、実行前に拒否されます。あとから variant を足したとき、扱いを直し忘れた場所がすべてエラーになるので、見落としを防げます。

```rewind-error non-exhaustive match
enum Light { Red, Yellow, Green }

fn name(l: Light) -> String effects {} {
    match l {
        Light::Red => { return "赤"; },
        Light::Yellow => { return "黄"; }
    }
}

fn main()->Int { return 0; }
```

パターンには、次のものが使えます。

| パターン | 例 | 一致するもの |
|---|---|---|
| リテラル | `1`、`"a"`、`true` | 等しい値 |
| 範囲 | `0..5` | 0 以上 5 未満の整数 |
| 名前 | `v` | 何にでも一致し、`v` に束縛する |
| `_` | `_` | 何にでも一致し、捨てる |
| variant | `Some(x)`、`Shape::Rect(w, h)` | その variant。中身は入れ子のパターン |
| 名前付きフィールド | `Message::Move{x: a, y: b}` | フィールドを持つ variant |
| `record`、`struct` | `Point{x: a, y: b}` | 各フィールド |
| タプル | `(1, s)` | 要素ごとのパターン |

アームには `if 条件` の **ガード** を付けられます。ガード付きのアームは、網羅の根拠としては数えられません。

```rewind
enum Shape {
    Circle(Int),
    Rect(Int, Int),
    Empty
}

fn area(s: Shape) -> Int effects {} {
    match s {
        Shape::Circle(r) => { return 3 * r * r; },
        Shape::Rect(w, h) if w == h => { return w * w; },   // 正方形
        Shape::Rect(w, h) => { return w * h; },
        Shape::Empty => { return 0; }
    }
}

fn main()->Int {
    Out.println(area(Shape::Circle(2)));
    Out.println(area(Shape::Rect(3, 3)));
    Out.println(area(Shape::Rect(2, 5)));
    return 0;
}
```

```text
12
9
10
```

### 型にメソッドを足したいとき

`struct` や `record` に、直接メソッドを書く構文はありません（`impl Point { ... }` のような書き方は使えません）。メソッドは、次の章で学ぶ `trait` を通して定義します。値を作る関数や、値を操作する関数は、普通の関数として書けば十分です。

### 公開と非公開

型・関数・定数に `pub` を付けると、他の module から使えます。`struct` や `record` のフィールドに `private` を付けると、その module の外からは読み書き・構築・分解ができなくなります。値の中身を隠して、決まった関数を通してだけ扱わせたいときに使います。詳しくは第14章です。
