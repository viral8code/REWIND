## 第5章 関数

### 関数を宣言する

関数には、名前、引数と型、戻り値の型を書きます。戻り値は `return` で返します。

```rewind
fn factorial(n: Int) -> Int effects {} {
    if n <= 1 { return 1; }
    return n * factorial(n - 1);
}

fn main()->Int {
    Out.println(factorial(5));
    return 0;
}
```

```text
120
```

関数は、ファイルのどこに書いても、どこからでも呼べます。呼ぶ側より後ろに宣言してもかまいません。関数の中に別の関数を宣言することはできません。

値を返す関数は、すべての経路で `return` に到達する必要があります。足りないと実行前に拒否されます。

本体が一つの式だけの関数は、`=` で短く書けます。

```rewind
fn double(x: Int) -> Int = x * 2;
fn greet(name: String) -> String = "hi ${name}";

fn main()->Int {
    Out.println(double(21));
    Out.println(greet("REWIND"));
    return 0;
}
```

```text
42
hi REWIND
```

### effects の宣言

関数のシグネチャに付いている `effects {}` は、その関数が **外の世界に対して何をするか** の宣言です。何もしない関数は `effects {}` と書きます。たとえば出力する関数は `effects {output}` と書きます。

```rewind
fn greet(name: String) -> Unit effects {output} {
    Out.println("こんにちは、${name}さん");
}

fn main()->Int effects {output} {
    greet("REWIND");
    return 0;
}
```

```text
こんにちは、REWINDさん
```

なぜこんな宣言が要るのでしょうか。理由は、巻き戻しにあります。「外部とやりとりしない関数」は、同じ引数なら同じ結果になるので、巻き戻して実行し直しても安全です。この区別を言語が追跡してくれるので、あとの章で出てくる「戻せるもの」と「戻せないもの」の境目を、型検査で守れます。詳しくは第13章で説明します。

なお、`main` だけは `effects` を省略でき、省略すると本体が必要とする効果が自動で求められます。

### 名前付きの引数と既定値

引数には、名前を付けて渡せます。名前を付けると、順序は自由です。引数には **既定値**（リテラルか `None`）を付けられ、その引数は呼ぶときに省略できます。

```rewind
fn box(width: Int, height: Int, label: String = "box") -> String effects {} {
    return "${label}: ${width * height}";
}

fn main()->Int {
    Out.println(box(3, 4));
    Out.println(box(height: 2, width: 5, label: "wide"));
    return 0;
}
```

```text
box: 12
wide: 10
```

既定値のある引数の後ろには、既定値のない引数を置けません。

### クロージャ

名前の無い関数は、`|引数: 型| -> 戻り値の型 { 本体 }` で作ります。型は省略できず、本体は必ず `{ }` で囲んで `return` で値を返します。外側の変数を読み書きできます。

```rewind
fn main()->Int {
    let add = |a: Int, b: Int| -> Int { return a + b; };
    Out.println(add(1, 2));

    var count = 0;
    let bump = || -> Unit { count += 1; };
    bump();
    bump();
    Out.println(count);
    return 0;
}
```

```text
3
2
```

#### 短い形

関数型が分かっている場所へ **直接渡す** ときは、型と `return` を省いて、`|引数| 式` と短く書けます。引数の型は、渡す先から決まります。

```rewind
fn main()->Int {
    let xs = [1, 2, 3, 4];
    let evens = xs.iter().filter(|n| n % 2 == 0).map(|n| n * 10).collect();
    Out.println(evens.len());
    Out.println(xs.iter().fold(0, |a, b| a + b));
    return 0;
}
```

```text
2
10
```

型は、関数の引数、`Iterator` の `map`・`filter`・`fold`、`Result` の `map`・`andThen`・`mapErr`、`return`、代入のほか、型注釈つきの `let` でも決まります。注釈の関数型から、引数の型を決められるためです。

```rewind
fn main()->Int {
    let f: fn(Int, Int) -> Int effects {} = |a, b| a + b;
    Out.println(f(1, 2));
    return 0;
}
```

```text
3
```

`let add = |a, b| a + b;` のように、注釈のない `let` で変数に束縛するときは、型を決める手がかりが無いので、`cannot infer the type of closure parameter` と拒否されます。そのときは、注釈をつけるか、前に説明した、型を書く形を使ってください。短い形は、`language` 3.0.0 以上でだけ使えます。

### 関数を引数に渡す

関数を受け取る引数には、関数型 `fn(引数の型) -> 戻り値の型 effects { ... }` を書きます。名前付きの関数も、クロージャも渡せます。

```rewind
fn apply(f: fn(Int) -> Int effects {}, x: Int) -> Int effects {} {
    return f(x);
}

fn inc(n: Int) -> Int effects {} { return n + 1; }

fn main()->Int {
    Out.println(apply(inc, 4));
    Out.println(apply(|n: Int| -> Int { return n * 2; }, 4));
    return 0;
}
```

```text
5
8
```

渡す関数の効果は、受け取る側の型で許された範囲に収まっている必要があります。この例では `effects {}`、つまり「何もしない関数」だけを受け取ります。

### 再帰の深さには上限がある

関数の呼び出しの深さには上限があり、約 1,000 段を超えると、`ExecutionBudgetExceeded` で止まります。深い再帰を書くときは、ループに書き換えられないか考えてください。この上限は、第23章で扱う「実行予算」の一部です。
