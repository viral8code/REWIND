# 数値計算の精度・形・結果の契約

数値型やsolverの構想を、計算の途中とbackendの選択まで広げる。型があることに加えて、どの誤差・形・費用を約束して計算したかを読みたい。

## VNUM-01：明示的なarithmetic context

精度、丸め方式、overflow、underflow、NaNの扱いを値として渡す。純粋計算は同じcontextを使って再実行できる。

threadごとの暗黙な設定に頼らず、cache keyや計算reportへcontextの版を含める。呼出し先libraryが対応しないpolicyも確認したい。

## VNUM-02：整数overflowの操作族

checked、wrapping、saturating、widening等を型または明示APIで選ぶ。低levelのalgorithmと一般的な数値計算が、同じ記号の暗黙差で食い違わない設計を考える。

既存演算の意味を変更する計画ではなく、意図が読める追加表現の候補。compile時の定数計算でも同じpolicyを使う。

## VNUM-03：approximate equalityの値

絶対誤差、相対誤差、ULP、scale等の比較policyを値にし、test・solver・mergeへ渡す。

近いことは一般に推移的ではないため、Mapの同値関係やhash keyにそのまま使わない。近接判定と分類用の量子化を分ける。

## VNUM-04：結果を含むaccumulator

合計値だけでなく、補償項・誤差見積り・要素数を持つaccumulatorを考える。大きなstreamを分割して集計した経緯も保てる。

merge順で変わる方式と、再現可能な方式を選べるようにする。並列化時の追加memoryや費用を計算planへ表示したい。

## VNUM-05：shapeと空間を持つmatrix

行と列に空間の名前を付け、同じ長さでも意味の違う座標を混ぜないmatrix。座標変換や物理量の対応をsignatureから読める。

runtimeのshape検査と、静的に得られるshapeの証拠を組み合わせる。dynamicな入力を扱うたびに型全体を生成する必要はない設計を探る。

## VNUM-06：疎構造のsymbolic plan

sparse matrixの非zero位置に依存する解析をplanとして分離し、値だけが変わる計算で再利用する。

patternの版とordering、backendをplanに含める。古いplanを別patternへ使えないようにし、変更が小さい場合の更新費用も見たい。

## VNUM-07：solverの結果をvariantにする

Converged、IterationLimit、Diverged、Singular、Cancelled等を返し、それぞれに利用できる近似値と診断を持たせる。

未収束でも近似値を使う判断を利用者が選べる。成功flag一つで、保証された解と途中の値を同じ扱いにしない。

## VNUM-08：条件数と残差の説明

結果の値に、残差、条件数の推定、入力scale、停止基準を添えたい。桁数が多いことと、結果が安定していることを区別して読める。

説明値の推定費用や信頼範囲も示す。高費用の診断を通常計算へ無条件に追加する構想にはしない。

## VNUM-09：必要時だけexactへ昇格

幾何predicateや比較で近似結果が不確かな場合だけ、高精度やexact arithmeticへ昇格するpolicy。

cacheには使用した精度と証拠を残す。高速経路で判定できた結果と、未確定の近似を区別して共有したい。

## VNUM-10：丸めを指定した区間伝播

区間の上下端を安全な向きに丸め、計算が囲い込みを保つcontractを持つ。scalar、matrix、solverの間でpolicyを共有したい。

未対応のnative演算を使った場合は囲い込みの保証を失った状態として返す。見た目が区間型だから保証があるとはしない。

## VNUM-11：近似を遅延させるexpression

exactな式treeや高精度の値を保持し、表示・比較・外部転送時に必要な精度へ評価する。早い丸めで失う情報を後から選べる。

expressionの共有とmemoizationで重複計算を抑える。式の大きさや評価費用に上限を持たせ、単純な値にも毎回式treeを強制しない。

## VNUM-12：精度と速度の比較plan

backend、精度、algorithmを変えた候補を同じ入力版で比較する。時間・memory・誤差・決定性の違いを並べて選べる。

選択結果は計算policyとして保存する。以後の入力へ適用する際、比較したshapeや条件の範囲外なら再評価できるようにしたい。
