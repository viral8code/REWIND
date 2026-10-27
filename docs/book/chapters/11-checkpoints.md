## 第11章 チェックポイント: commit と revert

ここからが、REWIND でいちばん特徴的な部分です。

### 保存して、戻る

`commit 名前;` は、いまの計算状態に名前を付けて保存します。この保存点を **チェックポイント** と呼びます。`revert 名前;` は、その保存点の状態に戻します。

次の例は、送金の途中で残高不足に気づいたとき、書き換えを取り消します。

```rewind
fn main()->Int {
    let balances = [100, 50];          // 口座 0 と口座 1
    commit start;                      // 送金前の状態を保存する

    balances.set(0, balances.get(0) - 130);
    balances.set(1, balances.get(1) + 130);

    if balances.get(0) < 0 {
        revert start;                  // 残高不足。書き換えをなかったことにする
        Out.println("残高不足のため取り消しました");
    }
    Out.println(balances.get(0));
    Out.println(balances.get(1));
    return 0;
}
```

```text
残高不足のため取り消しました
100
50
```

自分で「元の値」を取っておいて書き戻す必要はありません。何をどれだけ書き換えていても、`revert` が保存時の状態にまとめて戻します。

### 何が戻るのか

`revert` で戻るのは、プログラムの **計算状態** と、まだ外に出ていない **保留中の出力** です。

| 戻る | 戻らない |
|---|---|
| 変数の値、`List` や `Map` の中身 | `publish` で外に出した出力 |
| 実行中のタスクの状態 | すでに送信した通信、確定したデータベースの書き込み |
| 保留中の出力（`Out.println` など） | |

次の例で、保留中の出力が取り消される様子を確かめましょう。

```rewind
fn main()->Int {
    let items = [1, 2];
    var n = 10;
    commit saved;

    items.add(3);
    n = 99;
    Out.println(n);          // 保留中のまま、revert で消える

    revert saved;
    Out.println(items.len());
    Out.println(n);
    drop saved;
    return 0;
}
```

```text
2
10
```

`99` は、出力されません。「何を戻せて何を戻せないか」の境目は、次の第12章でじっくり見ます。

### revert は、実行位置を戻さない

見落としやすい点です。`revert` が戻すのは **状態だけ** で、プログラムの実行位置は動きません。`revert` の次の文から、そのまま実行が続きます。上の例でも、`revert saved;` の次の `Out.println(items.len());` が実行されています。

実行位置まで戻したいときは、`resume 名前;` を使います。`resume` は、状態と実行位置を、`commit 名前;` の直後へ戻します。

### resume の注意: 終わらないループ

`resume` は状態も戻すので、「状態だけを脱出条件にしたループ」は決して終わりません。

```rewind-error ExecutionBudgetExceeded
fn main()->Int {
    var n = 0;
    commit start;
    n += 1;                 // resume で n は 0 に戻る
    if n < 3 { resume start; }
    return 0;
}
```

この例では、`n` が `resume` のたびに 0 へ戻るため、`n < 3` はいつまでも真です。実行は、第23章で説明する「実行予算」を使い切って止まります。

脱出条件には、**巻き戻らないもの** を使います。公開済みの結果、消費した実行予算、外部領域で記録された観測（第12章）などです。そのため、日常的なコードでは `resume` より `revert` を使う場面の方がずっと多くなります。

### 保存点の名前と寿命

チェックポイントの名前は、変数や関数の名前とは別の名前空間にあります。次の規則があります。

- 同じ名前で二度 `commit` することはできません。
- 存在しない名前に `revert` や `drop` するとエラーです。
- `begin` は予約名です（第12章）。
- 不要になった保存点は `drop 名前;` で捨てられます。保存点は、保存した状態の分だけメモリを使います。使い終わったら捨てると、メモリを解放できます。

`revert` で戻れるのは、**同じ関数の中の、まだ有効なスコープにある保存点** です。呼び出し元の関数が作った保存点や、すでに終わった内側のスコープの保存点へ戻ろうとすると、`InvalidContinuation` になります。

```rewind-error InvalidContinuation
fn undo() -> Unit effects {} {
    revert outer;           // 呼び出し元で作った保存点には戻れない
}

fn main()->Int {
    commit outer;
    undo();
    return 0;
}
```

保存点より後に作った変数は、`revert` で消えます。

### 考え方のまとめ

- `commit` は、**外へ確定する操作ではありません**。あとで戻るための保存点を作るだけです。
- `revert` は、状態と保留中の出力を戻します。実行位置は動きません。
- 外へ出す（確定する）ためには、別の操作 `publish` を使います。次の章で学びます。
