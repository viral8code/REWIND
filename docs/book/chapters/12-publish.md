## 第12章 公開と外部領域

### 三つの層

実行中のものは、次の三つの層に分けて考えると整理できます。

| 層 | 内容 | `revert` で |
|---|---|---|
| 計算状態 | 変数、`List` や `Map` の中身、タスク、入力の読み取り位置、乱数の状態 | 戻る |
| 保留中の入出力 | `Out.println` や `File.writeText` の結果で、まだ外に出ていないもの | 取り消される |
| 確定済みの結果 | `publish` で外へ出たもの | 戻らない |

`Out.println("hi")` を実行しても、すぐ画面に出るわけではありません。いったん保留中になり、「確定してよい」と宣言されたときに初めて外へ出ます。この宣言が `publish;` です。

### publish: 外へ確定する

`publish;` は、保留中の入出力を外へ出します。公開した後の `revert` が戻せるのは、公開より **後の** 変化だけです。公開済みの結果は戻らず、再び送られることもありません。

```rewind
fn main()->Int effects {output} {
    var score = 10;
    commit first;
    score = 99;
    Out.println(score);
    publish;                 // 99 が外へ出る
    revert first;
    Out.println(score);      // score は 10 に戻った。この出力は保留中
    return 0;                // main が戻るときに公開される
}
```

```text
99
10
```

`99` は取り消せず、表示されたままです。

### main が戻ったときの自動公開

`main` が戻ると、保留中の出力は **自動で公開** されます。普通のプログラムでは、`publish;` を一度も書く必要はありません。`panic` や失敗で止まったときは公開されません。途中までの出力が外へ漏れることなく、「成功したときだけ出力が出る」という性質が得られます。

`publish;` を明示するのは、次のようなときです。

- 入力を待つ前に、プロンプトを見せたい（`In.readLine()` の前）。
- 長い処理の途中で、結果を外へ確定しておきたい。
- 取り消したくない結果を確定してから、先へ進みたい。

公開は、複数のファイルやストリームを **一度にまとめて** 適用する保証がありません。適用の途中で失敗すると `PublishPartiallyApplied` になり、どこまで適用されたかが診断に出ます。

### begin: 最初の状態へ戻る

プログラムの実行前に、`begin` という名前のチェックポイントが自動で作られます。`revert begin;` は、ここまでに作った変数やチェックポイントも含めて、すべてを開始時の状態に戻します。

```rewind
fn main()->Int {
    var x = 1;
    commit a;
    x = 2;
    Out.println("one");
    publish;                // one は確定済み
    x = 3;
    Out.println("two");     // 保留中
    revert begin;           // x も a も消え、two も消える
    Out.println("after");
    return 0;
}
```

```text
one
after
```

`begin` は予約名で、自分で `commit` や `drop` はできません。変数や関数の名前にも使えません。

### branch: 試すだけ試す

`branch 名前 { ... }` は、ブロックの中を試しに実行し、**終わると状態を実行前に戻します**。「もしこうしたら、どうなるか」を、状態を汚さずに確かめたいときに使います。ブロックの中の保留中の出力も捨てられますが、`publish` した結果は残ります。

```rewind
fn total(cart: &List<Int>) -> Int effects {} {
    var sum = 0;
    for i in 0..cart.len() { sum += cart.get(i); }
    return sum;
}

fn main()->Int {
    let cart = [300, 500];
    branch preview {
        cart.add(1200);                                 // もし高い品を足したら?
        Out.println("試算: ${total(&cart)} 円");
        publish;                                        // 試算の結果だけ外へ出す
    }
    Out.println("実際: ${total(&cart)} 円");
    return 0;
}
```

```text
試算: 2000 円
実際: 800 円
```

ブロックの終わりで `cart` は元に戻っています。

### 外部領域: 取り直せない値を扱う

時刻やネットワークの応答のように、外の世界から得た値は、巻き戻した後に取り直すと別の値になりえます。そうなると、「同じ状態へ戻って、もう一度実行する」という前提が崩れてしまいます。

REWIND は、そのような操作を `external { ... }` という **外部領域** に閉じ込めます。領域の中の操作の結果は **記録** され、巻き戻して実行し直したときは、時計やネットワークを再び見に行かず、記録された結果が使われます。

```rewind
import std.external as host;

fn main()->Int effects {external, clock, fileRead, fileWrite, output} {
    var stamp = 0;
    commit before;
    external {
        match host.millis() {
            Ok(n) => { stamp = n; },
            Err(_) => {}
        }
    }
    File.writeText("first.txt", stamp.format());
    publish;                       // 一度目の値をファイルに確定する
    revert before;                 // stamp は 0 に戻る
    external {
        match host.millis() {
            Ok(n) => { stamp = n; },
            Err(_) => {}
        }
    }
    match File.readText("first.txt") {
        Ok(text) => { Out.println(text == stamp.format()); },
        Err(_) => { Out.println("読めない"); }
    }
    return 0;
}
```

```text
true
```

二度目の `host.millis()` は、時計を読み直さず、一度目に記録した値を返します。そのため、巻き戻す前後で値が一致します。

領域には三つの形があります。

| 形 | 意味 |
|---|---|
| `external { ... }` | 結果を記録し、巻き戻した後は記録を再利用する |
| `external fresh { ... }` | いつも新しく実行し、その結果を記録する |
| `external live { ... }` | 物理的な操作を実行する。巻き戻しや再生では再利用しない。`live` 効果の許可が必要 |

領域の中では、外部操作だけを行います。`await`、`publish`、`commit`、`revert`、`return`、`break`、`continue` は使えません（`ExternalBoundary`）。通信を領域の中で始め、領域を出てから `await` で結果を受け取る、という使い方になります。

データベースへの書き込みのように、すでに外の世界へ及んだ作用は、`revert` で取り消せません。外へ出る操作は「戻せない」ことを前提に設計してください。
