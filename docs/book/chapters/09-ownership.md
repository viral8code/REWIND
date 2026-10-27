## 第9章 所有権と借用

### なぜ所有権があるのか

`List` や `struct` のように中身を変えられる値は、二か所から同時に持てると困ったことになります。片方で変更したつもりが、もう片方から見えてしまうからです。REWIND は、変更できる値を **暗黙にはコピーも共有もしません**。「誰がその値を持っているか」を型検査が追跡し、危ない書き方を実行前に拒否します。

巻き戻しとの関係もあります。保存した状態へ戻るときに、同じ値を指す名前が複数あると、「どの名前から見た状態が戻るのか」があいまいになります。持ち主を一人に決めておけば、戻る対象は常にはっきりします。

### 二種類の値

REWIND の値は、次の二種類に分かれます。

| 種類 | 例 | 扱い |
|---|---|---|
| **共有値** | `Int`、`Float`、`Bool`、`String`、`Bytes`、`record`、`Frozen<T>`、タプルなど | 自由にコピーして共有できる |
| **変更できる値** | `List`、`Map`、`struct`、イテレータ、ファイルなどの資源 | 持ち主が一人。コピーできない |

`let` か `var` かは、この区別とは関係ありません。`let xs = [1];` でも `xs.add(2)` は書けます。`let` が禁じるのは、`xs` に別の値を代入し直すことだけです。

共有値は、普通にコピーして使えます。難しいのは、変更できる値の扱いです。選べる方法は三つあります。

1. 所有権を渡す: `move`
2. 一時的に貸す: `&`（読むだけ）、`&mut`（変更する）
3. 変更できない複製を作って共有する: `freeze`

### move: 持ち主を替える

`move 式` は、値の持ち主を新しい側に替えます。渡した後の元の名前を使うと、エラーです。

```rewind
fn count(xs: List<Int>) -> Int effects {} {
    return xs.len();
}

fn main()->Int {
    let xs = [1, 2, 3];
    Out.println(count(move xs));
    return 0;
}
```

```text
3
```

```rewind-error UseAfterMove
fn count(xs: List<Int>) -> Int effects {} { return xs.len(); }

fn main()->Int {
    let xs = [1, 2, 3];
    let n = count(move xs);
    Out.println(xs.len());   // 渡した後は使えない
    return 0;
}
```

変更できる値を `move` なしで関数へ渡すと、`passing a mutable owner requires move or freeze` と拒否されます。「渡すと自分は使えなくなる」ことを、書き手に意識させるためです。

### 借用: 貸すだけで、持ち主は変わらない

関数に所有権を渡したくないときは、借用を使います。引数の型を `&T`（読むだけ）または `&mut T`（変更する）と宣言し、呼ぶ側も同じ記号を付けます。

```rewind
fn total(values: &List<Int>) -> Int effects {} {
    var sum = 0;
    for i in 0..values.len() { sum += values.get(i); }
    return sum;
}

fn append(values: &mut List<Int>, n: Int) -> Unit effects {} {
    values.add(n);
}

fn main()->Int {
    let xs = List<Int>();
    append(&mut xs, 7);
    append(&mut xs, 8);
    Out.println(total(&xs));
    return 0;
}
```

```text
15
```

借用の規則は、次の通りです。

- `&` の借用は、同時にいくつでも持てます。借用している間、持ち主は変更できません。
- `&mut` の借用は、同時に一つだけです。
- `&` の借用からは、値を変更できません。
- 借用を、関数の戻り値にしたり、タスクへ渡したりすることはできません。

```rewind-error cannot mutate through a shared borrow
fn bad(xs: &List<Int>) -> Unit effects {} {
    xs.add(1);
}

fn main()->Int { return 0; }
```

```rewind-error conflicting borrows
fn f(a: &mut List<Int>, b: &mut List<Int>) -> Unit effects {} {}

fn main()->Int {
    let xs = [1];
    f(&mut xs, &mut xs);
    return 0;
}
```

`xs.add(1)` や `xs.len()` のようなメソッド呼び出しでは、`&` や `&mut` は自動で付くので、書く必要はありません。

### freeze と thaw: 変えない複製を共有する

`freeze(値)` は、その時点の内容を **変更できない共有用の複製** にします。型は `Frozen<T>` です。`Frozen` は共有値なので、コピーしても、`record` のフィールドに入れてもかまいません。読み取り（`len`、`get`、`for`）はできますが、変更するとエラーです。

`thaw(frozen)` は、元の `Frozen` と独立した、変更できる値を作ります。

```rewind
fn sum(xs: Frozen<List<Int>>) -> Int effects {} {
    var total = 0;
    for x in xs { total += x; }
    return total;
}

fn main()->Int {
    let snapshot = freeze([1, 2, 3]);
    Out.println(sum(snapshot));
    Out.println(sum(snapshot));      // 何度でも渡せる

    let copy = thaw(snapshot);
    copy.add(4);
    Out.println(copy.len());
    Out.println(snapshot.len());     // 元は変わらない
    return 0;
}
```

```text
6
6
4
3
```

```rewind-error cannot mutate Frozen
fn main()->Int {
    let s = freeze([1]);
    s.add(2);
    return 0;
}
```

`freeze` は、複製を作る分の費用がかかります。関数値や、ファイル・接続のような資源、循環する値は `freeze` できません。

### 使い分けの目安

- 関数が値を **見るだけ**: `&`
- 関数が値を **更新する**: `&mut`
- 関数が値を **受け取って持ち続ける**（保存、別のタスクへ渡すなど）: `move`
- 同じ値を **複数の場所で読みたい**: `freeze`
