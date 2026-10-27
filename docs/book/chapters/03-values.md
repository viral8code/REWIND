## 第3章 値・変数・基本の型

### 変数を作る

名前に値を結び付けることを **束縛** と呼びます。`let` で作った名前は、あとから別の値に付け替えられません。付け替えたいときは `var` を使います。

```rewind
fn main()->Int {
    let base = 10;          // 付け替えられない
    var score = base;       // 付け替えられる
    score += 5;
    score *= 2;
    Out.println(score);
    return 0;
}
```

```text
30
```

迷ったら `let` を使い、必要になったときだけ `var` に変えるのがお勧めです。「この名前はずっと同じ値だ」と分かると、プログラムが読みやすくなります。

`let` の名前に再び代入しようとすると、実行前にエラーになります。

```rewind-error cannot assign to let binding
fn main()->Int {
    let x = 1;
    x = 2;
    return 0;
}
```

型は、値から自動的に決まります。書きたいときは `名前: 型` で明示できます。

```rewind
fn main()->Int {
    let count: Int = 3;
    let ratio: Float = 0.5;
    Out.println(count);
    Out.println(ratio);
    return 0;
}
```

```text
3
0.5
```

変数の名前には、`let` や `if`、`commit` のようなキーワードを使えません。名前に使えない語の一覧は、言語リファレンスの 3.2 節にあります。たとえば `commit` という変数が欲しくなったら、`committed` のように別の名前にします。

### 基本の型

| 型 | 意味 | 例 |
|---|---|---|
| `Int` | 符号付き 64 ビット整数 | `42`、`-7`、`1_000_000`、`0xff` |
| `Float` | 64 ビットの浮動小数点数 | `3.14`、`1.5e3` |
| `Bool` | 真偽 | `true`、`false` |
| `String` | UTF-8 の文字列 | `"こんにちは"` |
| `Bytes` | バイト列 | `Bytes("abc")` |
| `Unit` | 「値が無い」ことを表す型 | `()` |

数値のリテラルでは、桁の区切りに `_` を入れたり、`0x`（16 進）、`0b`（2 進）、`0o`（8 進）を使ったりできます。小数点の前後には数字が必要で、`.5` や `5.` とは書けません。

### 暗黙の型変換はない

REWIND には、型を自動で変換する仕組みがありません。`Int` と `Float` を混ぜた計算は、エラーになります。変換は、目的を書いて明示します。

```rewind
fn main()->Int {
    let n: Int = 3;
    let x: Float = 1.5;
    // n + x はエラー。まず Float に変換する
    match n.toFloatChecked() {
        Ok(f) => { Out.println(f + x); },
        Err(_) => { Out.println("変換できません"); }
    }
    Out.println(x.toIntChecked());   // 1.5 は整数にできないので失敗する
    return 0;
}
```

```text
4.5
Err(InexactOrOverflow)
```

`toFloatChecked` や `toIntChecked` は、変換が正確にできたときだけ `Ok` を返します。失敗の可能性を `Result` という型で表すのは、REWIND の基本的な方針です。`Ok` と `Err`、`match` の使い方は、第10章で詳しく説明します。

### 整数の計算は検査される

`Int` の `+ - * / %` は、結果が 64 ビットの範囲を超えたり、0 で割ったりすると、**実行が止まります**。値が黙って丸められたり、折り返したりはしません。

```rewind-error IntegerOverflow
fn main()->Int {
    let max = 9223372036854775807;
    Out.println(max + 1);
    return 0;
}
```

除算は 0 に向かって切り捨て、剰余の符号は割られる数に合わせます。

```rewind
fn main()->Int {
    Out.println(7 / 2);
    Out.println(-7 / 2);
    Out.println(-7 % 3);
    Out.println(7 % -3);
    return 0;
}
```

```text
3
-3
-1
1
```

より大きな整数が必要なときは `std.bigint` を使います（第20章）。

`Float` は IEEE 754 の規則に従います。0 で割っても止まらず、無限大や NaN（数でない値）になります。

```rewind
fn main()->Int {
    Out.println(1.0 / 0.0);
    let nan = 0.0 / 0.0;
    Out.println(nan == nan);    // NaN はそれ自身とも等しくない
    Out.println(0.1 + 0.2);
    return 0;
}
```

```text
inf
false
0.30000000000000004
```

### 数値のメソッド

数値には、よく使う計算がメソッドとして用意されています。

| 型 | メソッド |
|---|---|
| `Int` | `abs()`、`min(x)`、`max(x)`、`pow(n)` |
| `Float` | `abs()`、`sqrt()`、`floor()`、`ceil()`、`round()`、`min(x)`、`max(x)` |

```rewind
fn main()->Int {
    Out.println((-5).abs());
    Out.println(3.max(8));
    Out.println(2.pow(10));
    Out.println(2.5.floor());
    Out.println(16.0.sqrt());
    return 0;
}
```

```text
5
8
1024
2
4
```

`Int` の `abs()` と `pow(n)` は、結果が 64 ビットの範囲を超えると止まります。`Float` の `floor()` などは、`Float` を返します（`Int` にしたいときは、第3章で見た `toIntChecked` を使います）。

### 文字列

文字列は `"` で囲みます。改行や `\n` などのエスケープが使えます。文字列どうしは `+` で連結できます。

文字列の中に値を埋め込むには、`${式}` を使います。`${` という文字そのものを書きたいときは `$${` とします。

```rewind
fn main()->Int {
    let name = "REWIND";
    let n = 3;
    Out.println("${name}: ${n + 1} 件");
    Out.println("価格は $${n}");
    var s = "a";
    s += "b" + "c";
    Out.println(s);
    return 0;
}
```

```text
REWIND: 4 件
価格は ${n}
abc
```

数値などを文字列にしたいときは `.format()` も使えます（`3.format()` は `"3"` です）。

### 比較と等価

`<`、`<=`、`>`、`>=` は `Int`、`Float`、`String` に使えます。文字列は、UTF-8 のバイト列としての辞書順で比べます。論理演算は `&&`、`||`、`!` で、`&&` と `||` は左の結果で決まれば右を評価しません。

`==` は、値によって意味が変わります。数値、文字列、タプル、後の章で学ぶ `record` や `enum` は、**中身が等しいか** を調べます。`List` や `Map` のように変更できる値は、**同じ一つの値であるか** を調べるので、中身が同じでも別々に作ったものは等しくなりません。中身を比べたいときは `freeze` します（第9章）。

```rewind
fn main()->Int {
    let a = [1, 2];
    let b = [1, 2];
    Out.println(a == b);                    // 別々の List
    Out.println(freeze(a) == freeze(b));    // 中身で比べる
    Out.println((1, "x") == (1, "x"));
    Out.println(Some(3) == Some(3));
    return 0;
}
```

```text
false
true
true
true
```

### 演算子の優先順位

数学と同じように、`*`、`/`、`%` は `+`、`-` より先に計算されます。比較は算術よりも後、`&&` は `||` よりも先です。迷うときは `( )` で順序を明示してください。

ビット演算子（`&`、`|`、`^`、`<<`、`>>`）はありません。ビット操作が必要なときは、標準ライブラリの `std.bits` を使います。

### 型に別名を付ける

`type` で、既存の型に別名を付けられます。別名は新しい型を作らないので、元の型と自由に代入し合えます。意味を読み手に伝えるのに便利です。

```rewind
type Score = Int;

fn main()->Int {
    let s: Score = 3;
    Out.println(s + 1);
    return 0;
}
```

```text
4
```
