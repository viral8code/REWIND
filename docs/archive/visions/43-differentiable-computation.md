# 自動微分・感度・計算履歴

既存数理・AI構想の発展案。forward/reverse modeを、mutable model、checkpoint、native計算の契約と共に扱う方法を考える。

## VDIFF-01：微分の入出力を指定する

どの引数を微分し、どの出力からgradientを返すかを型付きpathで指定する。parameter record全体を毎回flat配列へ手書き変換せず使いたい。

離散値、識別子、physical handle等は微分対象の能力を持たない。元の型へ対応するtangent/cotangent型を明示する。

## VDIFF-02：JVPとVJPを別の操作にする

方向を与えて出力の変化を得るJVPと、出力の重みから入力のgradientを得るVJPをlibraryで使い分ける。

shapeと空間の対応を型で確認する。full Jacobianを無条件に構築せず、必要な積だけを計算したい。

## VDIFF-03：mutable計算のfunctionalization

localなarray更新をversion付きの操作へ変換し、微分に必要な旧値を追う。利用者が自然に書いたloopから計算graphを得たい。

alias、borrow、更新順を検査する。physical mutationやcheckpoint外counterを、数値modelの純粋更新と同じ扱いにしない。

## VDIFF-04：branchの微分contract

実行したbranchに沿う局所的な微分、非微分点、定義したsubgradient等を結果に持たせる。

branchをrevertできることだけで、選択そのものが微分可能になるわけではない。離散探索と連続parameterの感度を別操作として組み合わせる。

## VDIFF-05：tapeの明示owner

reverse modeのtapeをownerとして返し、gradient計算またはdropまで必要な値を保持する。

tape、利用者のcheckpoint、memoizationが同じpageを保持している場合の費用を分けて表示する。tapeを消しても利用者のsnapshotは残す。

## VDIFF-06：微分用の再計算plan

forward時の中間値を全部保持するか、選んだ境界から再計算するかをplanにする。

再計算区間はcode、input、random stream、arithmetic contextが一致する純粋計算へ限定する。physical inputを取得し直す計画にはしない。

## VDIFF-07：custom derivativeの契約

native関数やdomain primitiveにJVP/VJPを登録する。shape、dtype、effect、error条件を元の関数と対応させる。

有限差分等の検査を補助として生成する。数値検査が通ったことと、数学的な正しさの証明を区別して記録する。

## VDIFF-08：implicit differentiation

収束したrootや最適解の条件から感度を求める操作を考える。長いiterationを全部tapeへ保持せず計算したい。

収束、非特異性、制約の有効集合等の仮定を結果へ残す。条件が不足した場合は感度未確定や別の推定結果として返す。

## VDIFF-09：random計算のgradient estimator

reparameterization、score-function等の方式を、対象分布が持つ能力に応じて選ぶ。

同じ乱数traceを共有する比較と、推定量の性質を分ける。variance、bias、使用した補助項を計算reportで確認したい。

## VDIFF-10：高階微分の能力

gradient関数を再び微分し、Hessian-vector product等へ使う。custom derivativeやbackendが対応する階数を能力として表す。

tapeを持つ返値と通常の数値返値を区別する。未対応primitiveで、無意味な零gradientへ黙って変換しない。

## VDIFF-11：simulationの感度

worldの連続parameterや入力波形に対する感度を計算する。branch比較では、何がgradientで、何が離散の反事実かを表示する。

実行時間、実際の装置操作、取得済み観測を微分可能な値として扱わない。観測を使った純粋modelに対して計算する。

## VDIFF-12：gradientの版と採用

gradientへparameter版、data版、loss code版を付け、optimizerへの採用時に照合する。

backgroundで得た古いgradientを使う場合は、明示的なalgorithm policyにする。単に数値shapeが同じだから現在parameterへ自動適用する構成にはしない。
