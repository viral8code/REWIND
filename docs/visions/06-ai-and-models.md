# AI・model・学習・評価

現行のAD/optimizer/training/model保存を土台にした自由構想です。新しい機能名よりも、modelを作って調べ、比較し、渡せる体験を考えます。

## VAI-01：名前を持つtensor axis

batch、time、channel、feature等のaxis名で操作を指定する。transposeやbroadcastの意図を型・表示から読み、同じshapeでも意味が違う誤用を見つけたい。

## VAI-02：dtypeとquantizationの明示操作

Float32/64、低精度、整数quantizationを用途ごとに選ぶ。scale、zero point、誤差、overflowを持つ変換として扱い、暗黙に精度が落ちる場所を減らしたい。

## VAI-03：model componentの合成

layer、parameter、buffer、training/evaluation状態をcomponentとして組む。同じcomponentを共有する場合と複製する場合を明示し、graphとparameterの所有関係を見たい。

## VAI-04：custom gradientの登録

新しいoperationにforward/backwardやJVP/VJPを定義できる。numerical gradientとの照合を行い、誤ったgradientがどのoperationから来たかを追いたい。

## VAI-05：gradientの健康状態を調べる

爆発、消失、NaN、極端なparameter updateを可視化する。失敗iterationへ戻り、入力・activation・gradientを因果関係付きで比較したい。

## VAI-06：再計算を選ぶtraining memory管理

保存する中間値と再計算する中間値を選べる。学習用のactivation節約と、利用者が保持するREWIND checkpointを分け、memoryと追加計算の交換条件を見たい。

## VAI-07：datasetとlabelのversion管理

sample、label、split、annotation変更をstable identityで追う。model結果の差がコード・parameter・データのどれから来たかをreportで比較したい。

## VAI-08：学習と推論で同じ前処理

normalization、欠損補完、tokenization、feature選択を一つのpipelineへ定義する。学習で決めた統計値を推論へ持ち込み、data leakageや処理の食い違いを減らしたい。

## VAI-09：再現可能なaugmentation

画像・音声・textの変換を、sample ID、seed、epochから再現する。失敗sampleの変換過程を記録して、modelが何を見たかを後から確認したい。

## VAI-10：model formatとの相互運用

ONNX等を読み書きし、他のruntimeとの結果を比較する。対応operation、shape、dtype、unsupported部分を明示し、読み込めたというだけで完全互換としない体験にしたい。

## VAI-11：複数device・複数workerの学習

data/model parallel、gradient同期、worker failureを扱う。local stateとremoteで確定した進捗を区別し、再開時のdataset位置やoptimizer stateを揃えたい。

## VAI-12：強化学習とtrajectory探索

simulation環境のstate、action、reward、policyを扱い、trajectoryをbranchで比較する。VM内simulationを戻す場合と、実環境へactionを送った場合は別の境界として表示したい。

## VAI-13：localな言語model推論

tokenizer、KV cache、streaming token、sampling policyを扱う。分岐した候補文のcache共有やmemory費用を見ながら、modelの推論をapplicationへ組み込みたい。

## VAI-14：検索・生成の評価pipeline

取得文書、prompt、model version、生成結果、評価指標をつなぐ。回答の根拠や検索失敗を調べ、pipeline変更前後の品質をsample単位で比較したい。

## VAI-15：modelの判断を説明する

feature寄与、saliency、counterfactual、部分入力への反応を調べる。説明手法の前提も表示し、単一の図をmodelの完全な理由として扱わない使い方にしたい。

## VAI-16：実験を比較するmodel registry

設定、metric、dataset/model hash、artifact、再現手順をまとめる。保存済みmodelと現在のVM stateを区別し、採用・保留・差戻しを実験の履歴として辿りたい。
