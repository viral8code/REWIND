## 第8章 trait とジェネリクス

### trait: 型が持つ約束

`trait` は、「この型はこういう操作ができる」という約束です。メソッドの宣言を並べ、`impl トレイト for 型` で、それぞれの型について実装します。

メソッドの最初の引数は `self` で、その型は次のどれかです。

- `&Self`: 読むだけで借りる
- `&mut Self`: 変更するために借りる
- `Self`: 所有権を受け取る

`trait` のメソッドには、**既定の実装** を付けられます。実装する側は、既定のあるメソッドを省略できます。

```rewind
trait Shape {
    fn area(self: &Self) -> Int effects {};
    fn describe(self: &Self) -> String effects {} {          // 既定の実装
        return "面積 " + self.area().format();
    }
}

record Square { side: Int }

impl Shape for Square {
    fn area(self: &Square) -> Int effects {} { return self.side * self.side; }
}

fn main()->Int {
    let sq = Square(3);
    Out.println(sq.area());
    Out.println(sq.describe());
    return 0;
}
```

```text
9
面積 9
```

メソッドは `値.メソッド()` の形で呼びます。`Int` のような組み込みの型にも実装できます。

### ジェネリクス

型や関数は、**型引数** を取れます。`<T>` で宣言し、中身の型に関わらず使える部品を作れます。

```rewind
struct Stack<T> { items: List<T> }

fn main()->Int {
    let s = Stack<Int>(List<Int>());
    s.items.add(1);
    s.items.add(2);
    Out.println(s.items.len());
    return 0;
}
```

```text
2
```

型引数が引数などから決まるときは、省略できます。決まらないときは `List<Int>()` のように明示します。

### 型引数に制約を付ける

`T` が何でもよいとは限りません。たとえば二つの値のうち大きい方を返したいなら、`T` は大小を比べられる型でなければなりません。このような要求を、`T: 制約` で書きます。

| 制約 | 意味 |
|---|---|
| `Share` | コピーして共有できる型 |
| `Send` | タスクへ渡せる型 |
| `Ord` | 大小を比べられる型（`Map` のキーや並べ替えに使える） |
| `Eq` | 等しいかどうか比べられる型 |
| `Hash` | ハッシュ値を計算できる型 |
| `Display` | 文字列として表示できる型 |

制約は、`T: Ord + Share` のように `+` でつなぐか、`where` 節にまとめて書きます。

```rewind
fn largest<T>(a: T, b: T) -> T effects {} where T: Ord + Share {
    if a.cmp(b) >= 0 { return a; }
    return b;
}

fn main()->Int {
    Out.println(largest(3, 9));
    Out.println(largest("a", "b"));
    return 0;
}
```

```text
9
b
```

`Int` と `String` は、どちらも `Ord` を満たしているので、同じ関数に渡せます。

### 自作の型を比べる

独自の型を `Map` のキーにしたり、並べ替えたりするには、その型に `Ord` を実装します。`cmp` は、小さければ負、等しければ 0、大きければ正の整数を返します。

```rewind
record Version { major: Int }

impl Ord for Version {
    fn cmp(self: Self, other: Self) -> Int effects {} {
        if self.major < other.major { return -1; }
        if self.major > other.major { return 1; }
        return 0;
    }
}

fn main()->Int {
    let m = Map<Version, String>();
    m.set(Version(2), "two");
    m.set(Version(1), "one");
    Out.println(m.len());
    Out.println(Version(2).cmp(Version(1)));
    return 0;
}
```

```text
2
1
```

### 型引数付きの enum

`enum` も、型引数を取れます。標準の `Option<T>` や `Result<T,E>` は、このようにして作られている型です。自分で作ることもできます。

```rewind
enum Tree<T> { Leaf, Node(T) }

fn main()->Int {
    let t: Tree<Int> = Tree::Node(5);
    match t {
        Tree::Leaf => { Out.println("葉"); },
        Tree::Node(v) => { Out.println(v); }
    }
    return 0;
}
```

```text
5
```
