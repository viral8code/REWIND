# REWIND v1.9.4 — 数値走査・generic scope・履歴予算

## 数値配列

contiguous な FloatArray / IntArray の走査では、256 要素の storage page ごとに木を探索する。各要素で rank の除算と木の探索を繰り返さない。追加の全配列 buffer は作らない。offset を持つ contiguous slice と reshape にも適用する。

sum / dot / mean / variance、map / zip、materialize、値の List 化と bits を利用する統計処理がこの走査を使用する。転置・逆順・broadcast 等の非連続 view は従来の論理順序で走査する。浮動小数点の加算順、補償加算、NaN / Infinity の検査、整数 overflow、共有 storage と COW 更新の契約を保つ。行列積の計算方式と native kernel のキャンセルはこの版の対象に含めない。

ページ境界をまたぐ offset slice、ExactSizeIterator の残数、転置・逆順・broadcast・空 view、materialize と map の結果を検証する。`scripts/benchmark-numeric-scan.py` は 1,048,576 要素の vector に sum と dot を各 40 回実行し、実行時間と Linux peak RSS を比較する。測定には import・初期配列の生成・work admission・結果検査も含む。

同じ Linux x86_64 環境、Rust 1.98.1 の optimized build、3 回の実行では、1.9.3 の中央値 1.302 秒 / 23,632 KiB に対し、1.9.4 は 0.513 秒 / 22,928 KiB だった。native work は 10,000,000,000、execution steps は 10,000,000 を指定した。work の保守的な admission は維持する。これはこの連続 vector workload の測定であり、他の配列形状や OS の速度を保証するものではない。

## import 内の型引数

struct / enum / type alias の型引数は、その定義内で同名の module symbol に優先する。例えば `fn T()` と `struct Box<T> {value:T}` が同じ module にあっても、field の T を function の修飾名へ置換しない。generic function の既存の scope 規則と一致させる。source を取り除いた artifact と trace replay でも検証する。

## 初期履歴予算の CLI 指定

```sh
rewind run main.rw --history-memory 512MiB --history-storage 8GiB --spill-threshold 8MiB
```

サイズは非負整数の byte 数、または KiB / MiB / GiB suffix で指定する。memory / storage は正数、spill threshold は 0 も指定できる。既定はそれぞれ 512 MiB / 8 GiB / 8 MiB。サイズの overflow と未知の単位を拒否する。

これは Runtime の履歴・計算値の admission と spill に関する予算で、OS の RSS 上限ではない。`--memory-mib` は Linux の address-space 上限として独立して働く。言語の `runtime { ... }` は実行中に履歴予算を変更できる。execution / native / task の累積 work quota は履歴予算とは独立しており、revert で復活しない。

trace に初期予算を記録し、replay は同じ予算を復元する。明示的な CLI 予算が記録と異なる場合は ReplayMismatch で終了する。この field がない trace の初期予算は既定値として扱う。従来通り trace の compiler version は一致が必要。

## 継続事項

累積 task quota metadata、DOM trace と host file snapshot の全体 materialization、native kernel の公平性とキャンセル、GUI・アルゴリズム・v2.0 統合の受入条件は継続する。この版の高速化だけでこれらを完了扱いにしない。
