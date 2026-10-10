# 数理・科学計算・simulation

既存numeric、統計、FFT、AD等の先へ広げる自由構想です。library名だけでなく、計算を調べたり説明したりする体験も考えます。

## VSCI-01：ComplexとRationalの数値体系

複素数と正確な有理数を、配列・式・serializationまで一貫して扱う。近似値が必要になる境界を明示し、途中までは正確に計算する用途にも使いたい。

## VSCI-02：区間演算と保証された誤差

値を一点ではなく上下限として計算し、丸めや入力誤差を含めて結果の範囲を得る。条件分岐が確実かどうかを判断し、「この精度では決められない」結果も表したい。

## VSCI-03：精度を指定する浮動小数点

計算ごとに必要な桁数を指定し、精度を変えた結果を比較する。桁落ちや誤差の増幅を表示して、Float64だけでは不足する計算の原因を追いたい。

## VSCI-04：symbolic expressionと数式変形

式木を扱い、展開、因数分解、微分、簡約、代入を行う。記号計算の結果をnumeric kernelへ接続し、変形した根拠や前提条件も見たい。

## VSCI-05：ODEと適応stepの探索

微分方程式を解き、step幅の選択、棄却、error推定を時間軸で調べる。途中からparameterを変えるbranchを作り、異なるtrajectoryを比較したい。

## VSCI-06：PDEとmesh-based計算

grid/mesh、境界条件、fieldを使って偏微分方程式を解く。mesh refinementと結果可視化をつなぎ、memoryやsolver収束を同じreportで調べたい。

## VSCI-07：有限要素と構造解析

element、材料、拘束、荷重をmodelとして組み立てる。解析条件をbranchで比べ、変位や応力の結果から元elementへ戻れる表示にしたい。

## VSCI-08：線形・整数・制約付き最適化

目的関数と制約を宣言し、解・不可解・未収束を区別する。制約の衝突や支配的な条件を説明し、条件を変えた場合の解の違いを見たい。

## VSCI-09：SAT/SMTと制約探索

型付き条件から充足例や反例を得る。small programや設定の検証に使い、解をREWINDの値へ戻してtestへ組み込めると嬉しい。

## VSCI-10：積分・root探索・誤差の説明

数値積分や非線形root探索で、区間、iteration、収束の理由を表示する。成功値だけでなく、評価回数、残差、危険な区間を取得したい。

## VSCI-11：Monte Carloと確率過程

sampling、Markov chain、粒子法等を再現可能なseedと設定で実行する。途中stateをbranchで比較し、収束診断や有効sample数を結果と並べたい。

## VSCI-12：感度・Hessian・高階微分

既存ADから、parameter感度、高階微分、Jacobian/Hessianの作用へ広げる。巨大matrixを全部作る方式と、必要なvectorへの作用だけを計算する方式を選びたい。

## VSCI-13：不確実性の伝播

入力の分布や範囲から、出力の不確実性を評価する。測定値、model error、数値誤差を区別し、どの入力が結果の揺れへ寄与するかを見たい。

## VSCI-14：signal解析の統合pipeline

FFTに加え、window、filter、wavelet、spectrogram、event検出をつなぐ。raw signalから特徴量までの変換と単位を保持し、異なる設定をbranchで比べたい。

## VSCI-15：離散event simulation

queue、資源、待機、到着、サービス時間をmodel化する。simulation上の時計を前後に調べ、実際のOS時計や外部設備の動作と分けて実験したい。

## VSCI-16：実験計画と統計report

parameter sweep、bootstrap、検定、多重比較、効果量をまとめる。単一のp値だけで結論を出す体験にせず、条件・前提・sample・不確実性をreportへ残したい。
