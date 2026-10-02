# REWIND v1.8.0 — 型付き数値配列

1.8 工程の最初の公開単位として、`std.numeric` と native の FloatArray / IntArray を追加する。DB、HTTP、GUI の外部作用とは異なり、配列は VM の値であり、commit / revert、freeze / thaw、Task の受渡し、compiled artifact / replay に従う。

## 値と更新

FloatArray は IEEE 754 binary64、IntArray は符号付き64 bit。要素を VM の Value に包まず、256要素（2 KiB）の native page と永続二分木に保持する。root の複製は O(1)、一要素の変更は一つの page と O(log pages) の経路を複製する。最後の参照がなくなった native page は Rust の Arc により解放される。VM heap に格納した値への参照は既存の tracing GC が管理する。

配列は所有された値で、`withFloat` / `withInt` は更新した新しい配列を返す。代入して使う。

```javascript
import std.numeric as numeric;
fn take<T,E>(value:Result<T,E>)->T effects {} {
    match move value {Ok(item)=>{return move item;},Err(_)=>{panic("numeric operation failed");}}
}
let shape = List<Int>(); shape.push(2);
var array = take(numeric.zerosFloat(&shape));
let index = List<Int>(); index.push(0);
commit saved;
array = take(numeric.withFloat(&array, &index, 99.0));
Out.println(take(numeric.getFloat(&array, &index)));
publish;
revert saved;
Out.println(take(numeric.getFloat(&array, &index)));
publish;
```

行列計算を含む [例](../examples/numeric/main.rw) も参照できる。公開出力は `99`、`0`。配列の直接 constructor と内部の pointer / storage ID は公開しない。

## shape と view

最大 rank は8、shape の各 dimension と要素数は最大16,777,216。rank 0 は1要素の scalar array。0 dimension は空配列。index は0始まりの非負整数、dimension ごとの境界を検査する。

- `fromFloat` / `fromInt` は shape と List の要素数の完全一致を要求する。型の暗黙変換は行わない。
- `shapeFloat` / `shapeInt`、`stridesFloat` / `stridesInt` は descriptor を返す。stride の単位は要素。
- `reshape*` は row-major の連続 view に対し、要素数を変えずに行う。
- `transpose*` は全 axis の permutation、`slice*` は axis / start / length / step を明示する。negative step に対応する。negative index の省略規約は設けない。
- `broadcast*` は明示操作。繰返しを含む view は read-only とし、`with*` は NumericReadOnly を返す。更新する場合は `materialize*` で独立した連続配列を作る。
- view は生成時の storage version を所有する。元の配列を後で更新しても、view の内容は変わらない。
- `values*` は VM List への明示的な変換であり、全要素の Value を作る。大規模演算では配列のまま native kernel を使う。

## 演算と誤差

`zipFloat` / `zipInt` は同じ shape の add / sub / mul / div、Int は rem にも対応する。自動 broadcast は行わない。Int の範囲外と除算失敗は NumericOverflow。Float の演算・統計・solve は NaN / Inf を拒否し、domain / overflow / singular は StdError の code で返す。格納と読み出しは NaN payload、negative zero を含む bit pattern を保持する。

`math(operation,value)` と `mapFloat(operation,array)` は sqrt、exp / expM1、log / log1p、sin / cos / tan、asin / acos / atan、sinh / cosh / tanh、abs、floor / ceil / trunc、round / roundEven を提供する。round は中間をゼロから遠ざけ、roundEven は ties-to-even。libm の最下位 bit が異なる環境間で、計算結果の bitwise な同一性を約束しない。

`sum` と `dot` は Neumaier の補償和、`mean` と `variance(ddof)` は Welford の online 集計。空の sum / dot は0、mean と len <= ddof の variance は NumericEmpty。`matmul` は rank 2、`solve(matrix,right,tolerance)` は正方行列と rank 1 の右辺に対する部分 pivot LU。逆行列を作らない。pivot の絶対値が tolerance × 行列の最大絶対値以下の場合は NumericSingular。数値比較には入力の条件と絶対・相対誤差、residual を用いる。

## 費用・保存・検証

shape / index / 型を検査し、native work と一時領域・出力のメモリ予算を確保してから演算する。matmul は O(mnk)、LU solve は O(n³) の仕事量を事前計上する。kernel は Rust の loop で実行し、要素ごとの REWIND callback や JSON 往復を使わない。page 内は連続であり、全配列が一つの巨大な物理領域に入るという保証はしない。strided view の読み出しと木の参照費用も発生する。

診断は全要素の文字列を作らず、descriptor と cached SHA-256 を使う。replay の state digest は値の変化を検出する。内部 wire の serde は backing storage、shape、stride、offset、writable と数値 bits を保存し、deserialize 中に rank / 長さ / 境界 / writable view の重なりを検査する。native payload の保持量も VM のメモリ予算に含める。現段階では異なる VM root 間の共有領域を保守的に重複計上する場合があり、1.9 の memory audit で改善する。

検証にはページ共有、逆 stride、broadcast、overflow / domain / singular、補償和、LU の residual、wire 改ざん拒否、freeze / Task、revert、source-free run / replay、native work の事前拒否を含める。公開は Linux / Windows の回帰と展開済み SDK の検証後に行う。

## 1.8 工程の残り

本版を1.8工程全体の完了とはしない。QR / least squares / 固有値、共分散 / 分位点 / histogram / correlation / 分布乱数、BigInt / Decimal と DB / JSON の型変換、日時 / IANA / DST、Unicode / regex、増分 JSON / CSV を後続の1.8 patchで実装・検証する。元の [到達条件](v2-design.md) は維持する。
