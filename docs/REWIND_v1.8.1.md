# REWIND v1.8.1 — 線形代数・統計・分布乱数

v1.8.0 の型付き数値配列へ QR、最小二乗、対称固有値計算と追加統計を接続し、`std.distributions` を追加する。数値配列と集計・乱数状態は VM 内にあり、commit / revert、freeze、Task、source-free artifact / replay の契約に従う。

## 分解・解法

`std.numeric.qr(array,tolerance)->Result<QrResult,StdError>` は列 pivot 付き Householder QR。Q は economy size、R は上三角形 / 台形で、`A[:,permutation] = Q × R`。形状 m×n、p=min(m,n) に対し Q は m×p、R は p×n。結果の `q` / `r` は FloatArray、`permutation` は Frozen<List<Int>>、`rank` は Int。rank は残差列 norm と tolerance × 最初の最大列 norm を比較して判定する。rank deficiency 自体は QR のエラーにせず、結果で返す。

`leastSquares(matrix,right,tolerance)->Result<FloatArray,StdError>` は m>=n、rank 1 の右辺長 m、full column rank を要求する。pivot 付き QR を使って解き、係数を元の列順に戻す。正規方程式や逆行列へ置き換えない。rank deficiency は NumericSingular。matrix と Q / R を使う bounded な一時領域は予算に含める。

`eigenSymmetric(matrix,tolerance,maxSweeps)->Result<EigenResult,StdError>` は有限の正方対称行列に cyclic Jacobi を使う。最大絶対要素で scale し、対称性と off-diagonal の停止条件に相対 tolerance を用いる。許容範囲内の非対称差は対称な平均にする。maxSweeps は0..10000。停止条件に到達できない場合は NumericConvergence で、未収束の値を成功として返さない。

`values` は昇順の FloatArray、`vectors` は対応する固有ベクトルを列に持つ FloatArray、`sweeps` は実行した sweep 数。固有ベクトルの符号や重複固有値の基底は一意ではない。一般の非対称行列や complex spectrum をこの API で扱うことはしない。

全解法は NaN / Inf を拒否する。tolerance は有限の非負値。内部 scale により不要な overflow を避け、表現範囲を超えた結果は NumericOverflow。精度確認には QR の再構成、Q の直交性、最小二乗の residual、`A V - V diag(values)` を用い、丸めた表示文字列の完全一致で判断しない。

## 統計

- `covariance(left,right,ddof)` は同じ長さの rank 1 配列に online paired moments を使う。len<=ddof は NumericEmpty。
- `correlation(left,right)` は Pearson の相関係数。定数入力など分散が0の場合は NumericDomain。
- `quantile(array,probability)` は全 logical 要素の明示 copy を順序付け、probability×(len-1) の隣接値を線形補間する。probability は0..1、空配列は NumericEmpty。
- `histogram(array,edges)` は有限で厳密に増加する rank 1 の境界を要求する。区間は `[left,right)`、最後の右端だけ含める。結果の `counts:IntArray` と `underflow:Int` / `overflow:Int` で範囲外の値も明示する。

NaN / Inf は欠損値として勝手に捨てず、NumericNonFinite とする。quantile は一時領域 O(n)、仕事量 O(n log n)。histogram は boundary の copy と bin count を保持し、仕事量 O(samples log edges + edges)。

`onlineMoments()` はサンプルを保持しない mutable な OnlineMoments を作る。`pushMoment(&mut owner,value)`、`mergeMoments(&mut target,&source)`、`onlineCount` / `onlineMean` / `onlineVariance(ddof)` で更新・結合・照会できる。count は整数から Float への exact conversion を維持するため2^53まで。型付き Result のエラーは書込み前に検査し、以前の集計を保持する。fatal な VM の予算中断を transaction の自動取消しとして扱わない。所有者は通常の VM の checkpoint / revert に対応する。

## 分布乱数

`std.distributions` は `random` 効果を宣言する独立 module。純粋な `std.numeric` にこの効果を加えない。

| API | 契約 |
| --- | --- |
| uniform()->Float | checkpointed generator の53 bitを使う `[0,1)` |
| normal(mean,deviation)->Result<Float,StdError> | Box-Muller、有限の mean、有限で非負の deviation、2 draw |
| exponential(rate)->Result<Float,StdError> | 有限で正の rate、1 draw、非負の結果 |
| bernoulli(probability)->Result<Bool,StdError> | 有限の0..1、1 draw |

乱数の状態を VM の外へ置かず、隠れた normal の spare cache を作らない。同じ checkpoint へ戻した同じ呼出しは同じ draw を使う。generator は暗号用途の乱数ではない。引数の domain / finite validation は draw 前に行う。生成した Float が表現できない場合は NumericNonFinite。数学関数の最下位 bit に関する OS 間の約束は v1.8.0 と同じ。

## 予算・受入・後続

QR / leastSquares は O(mn min(m,n))、eigen は最大 sweep を含む O(maxSweeps n³) の仕事量を先に計上する。空の m×n でも permutation の n 要素をゼロ費用にしない。Q / R / reflection、ソート、bin、descriptor と VM List の容量も出力前に予算確認する。

受入には tall / wide QR、pivot の列順、rank deficiency、正規方程式を作らない最小二乗、空・zero・scale の異なる行列、固有値の並びと直交性・residual、収束拒否、統計の参照値、histogram の endpoint / outlier、分布の checkpoint / replay と deterministic sample moments、online merge / revert を含める。Linux / Windows の回帰・実 DB fixture・展開済み SDK が成功してから main 統合と公開を行う。

本版で1.8工程全体を完了とはしない。次の patch は BigInt / Decimal と DB / JSON 変換、その後に日時 / IANA / DST、Unicode / regex と増分 JSON / CSV を進める。[到達計画](v2-design.md)は維持する。
