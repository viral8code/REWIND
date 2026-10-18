## 第21章 数値配列と線形代数

### List と数値配列

`List<Float>` は、何でも入れられる汎用のコレクションです。大量の数値を計算するには、`std.numeric` の **数値配列** を使います。数値配列は、形（shape）を持つ、型の決まった連続したメモリの配列で、行列や多次元の配列を表せます。計算が速く、メモリも少なくて済みます。

数値配列には、`Float` の配列（`…Float` の関数）と、`Int` の配列（`…Int` の関数）があります。

| 関数 | 働き |
|---|---|
| `zerosFloat(&形)` | 0 で満たした配列を作る |
| `fromFloat(&形, &値)` | `List<Float>` の値から作る |
| `getFloat(&配列, &添字)` | 一つの要素を読む |
| `withFloat(&配列, &添字, 値)` | 一つの要素を変えた **新しい** 配列を返す |
| `valuesFloat(&配列)` | `List<Float>` に取り出す |
| `reshapeFloat`、`transposeFloat`、`sliceFloat` | 形の変更、転置、部分の取り出し |
| `sum`、`mean`、`variance`、`dot`、`matmul` | 集計と行列の積 |
| `solve`、`qr`、`leastSquares`、`eigenSymmetric` | 線形代数 |

「形」は、各次元の大きさを並べた `List<Int>` です。`[2, 2]` なら 2 行 2 列の行列、`[3]` なら長さ 3 の列です。

### 連立方程式を解く

次の例は、`0x + 2y = 4`、`1x + 3y = 7` を解きます。さらに、行列の要素を書き換えて、チェックポイントへ戻すと、行列も戻ることを確かめます。

```rewind
import std.numeric as numeric;

fn take<T,E>(value: Result<T,E>) -> T effects {} {
    match move value {
        Ok(item) => { return move item; },
        Err(_) => { panic("数値演算に失敗しました"); }
    }
}

fn main()->Int {
    let shape = [2, 2];
    let values = [0.0, 2.0, 1.0, 3.0];
    var matrix = take(numeric.fromFloat(&shape, &values));
    let rhsShape = [2];
    let rhsValues = [4.0, 7.0];
    let rhs = take(numeric.fromFloat(&rhsShape, &rhsValues));
    let answer = take(numeric.solve(&matrix, &rhs, 0.00000000000001));
    let first = [0];
    let second = [1];
    Out.println(take(numeric.getFloat(&answer, &first)));
    Out.println(take(numeric.getFloat(&answer, &second)));

    let cell = [0, 0];
    commit saved;
    matrix = take(numeric.withFloat(&matrix, &cell, 99.0));
    Out.println(take(numeric.getFloat(&matrix, &cell)));
    publish;                 // 99 は確定済みなので、revert の後も残る
    revert saved;
    Out.println(take(numeric.getFloat(&matrix, &cell)));
    return 0;
}
```

```text
1
2
99
0
```

この例には、覚えておきたい点が三つあります。

1. **配列は更新ではなく、置き換えで変える**: `withFloat` は元の配列を変えず、要素を変えた新しい配列を返します。`matrix = …` と代入し直します。
2. **`revert` は配列も戻す**: `saved` へ戻すと、`matrix` は 99 を書き込む前の状態に戻ります。巨大な配列でも、変えていない部分は共有されるので、保存の費用は小さく済みます。
3. **`commit` の後の変数は、`revert` で消える**: `cell` を `commit` の前に作ったのは、そのためです。`commit` の後で作った変数は、`revert` の後には存在しません。

`solve` の最後の引数は、特異に近い行列を拒否するための許容値です。

### 計算が重いとき

数値配列の計算は、ネイティブコードで実行されます。そのため、実行ステップではなく、**ネイティブ処理量** の予算（第23章）を消費します。同期的に実行される重い計算の途中では、他のタスクへ切り替わりません。GUI と重い計算を組み合わせるときは、`numericAsync` のような、協調して進む版の関数を選んでください。

数値計算には、他にも、疎行列（`std.sparse`）、高速フーリエ変換（`std.fft`）、確率分布（`std.distributions`）、自動微分（`std.autodiff`）や最適化（`std.optimize`）があります。関数の一覧と費用の契約は、[libraries/README.md](../../libraries/README.md) を参照してください。
