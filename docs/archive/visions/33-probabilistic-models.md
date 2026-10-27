# 確率model・推論・実験の構成

probabilistic programmingの構想を、trace、推論方式、説明へ広げる。乱数を使う計算を単にrevertできるだけでなく、どの仮説をどう調べたかを値として扱いたい。

## VPROB-01：分布を合成できる値

Distribution<T>にsample、logDensity、support、変換等を持たせる。有限・連続・混合分布を、対応する能力から合成する。

密度が計算できない分布や、normalizing constantが不明な分布も別contractで扱う。すべてに同じ操作があるように見せない。

## VPROB-02：addressを持つrandom choice

model内の乱数選択へ安定したaddressを付け、traceの該当選択だけを変更する。別branchへ進んだ場合も、再利用できる選択を識別したい。

sourceの行番号だけへ依存せず、loop内のindexやmodel上のkeyを使える設計を考える。address衝突を診断できると便利。

## VPROB-03：観測と潜在変数の分離

observeとsampleを区別し、観測版をmodelの実行に添える。観測を変更したrunと、同じ観測で探索を変えたrunを比べられる。

観測へ使った前処理も版として残す。あとから得た情報を過去の推論へ黙って混ぜない。

## VPROB-04：traceの部分再実行

random choiceを一部変更した時、その依存先だけを再計算したい。純粋な共通prefixや変更の影響がないsubtreeを共有する。

条件分岐でchoice集合が変わる場合は、新しいchoiceと消えたchoiceを記録する。traceの再利用は、推論algorithmが要求する確率補正にも対応する。

## VPROB-05：推論backendのcontract

importance sampling、MCMC、SMC、variational inference等を同じmodelへ適用できる形を考える。

必要な微分、密度、有限support等の能力をbackendが要求する。backendを交換しただけで、同じ推定保証や収束性が得られるとはしない。

## VPROB-06：support違反と数値失敗

確率が零の候補、無効parameter、数値overflow、実行budgetの終了を別結果として返す。

推論上の棄却と処理系の故障を混ぜず、候補の頻度や診断へ反映する。fatal failureを零確率として隠さない。

## VPROB-07：posteriorと診断を持つ返値

sample列だけでなく、weight、chain、effective sample size、収束の診断、使用した仮定をInferenceResultへ持たせたい。

診断を満たさない場合も結果を閲覧できる一方、推定値へ保証が付いたように表示しない。libraryは診断の定義と限界を文書化する。

## VPROB-08：推論外のmetrics catalog

試行履歴、採択率、時間、失敗理由をcheckpoint外へ集め、候補を戻しても説明を残す。

metricsが推論を変える場合は、単なる観測ではなくadaptive policyとして記録する。集計の置き場所だけで推論の正しさが保たれるとはしない。

## VPROB-09：checkpoint可能な推論state

chain state、粒子、乱数state、optimizer stateを型付きdataとして保存する。中断再開、parameter比較、診断の再計算へ使う。

resumeは必要なcode・backend・input版を照合する。native device handleの復元とは分け、dataから新しい計算資源を構築する。

## VPROB-10：反事実を作る介入

同じ観測を条件にした推論と、model内の因果構造へ介入する計算を別操作にする。選んだ変数を変更した場合の仮説を比較したい。

因果の解釈はmodelが提供する仮定に依存する。単なる相関modelへ介入APIを付けて、因果関係を自動保証する構想ではない。

## VPROB-11：random streamの階層分割

model、chain、worker、mini-batch等に明示keyを付けて乱数streamを分割する。並列実行の順序を変えても、対応するrunを再現しやすくする。

使用する生成器と分割方法の版も保存する。統計的な性質と暗号用途の性質は別のcontractとして選ぶ。

## VPROB-12：実験を選ぶmodel

新しい観測を取る案を、費用・期待情報量・制約と共に計算する。候補間で推論を共有し、どの観測が仮説を分けるかを説明したい。

現実の測定はIntentとして別に実行する。測定後に得られた値を保存し、推論を戻して測定済みという事実を消さない。
