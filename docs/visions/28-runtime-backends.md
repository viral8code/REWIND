# 処理系backendと意味を保つ最適化

現行VMやnative kernelをすべて作り直す計画ではありません。JIT/AOTの構想を、型・checkpoint・予算・debug情報を保つ実行基盤へ広げる自由構想です。

## VBACKEND-01：typed IRを調べるviewer

sourceから型検査・effect解決・特殊化後の表現へ辿る。どのoperationがnativeへ移り、どこでcopyやsafe pointが入るかを見たい。

最適化前後を対応させる。IRを読める人だけでなく、source側へ説明を戻す表示も欲しい。

## VBACKEND-02：共通契約を持つbackend

interpreter、bytecode、native等が同じtyped operationを実行する。backendの差を、sourceの意味の差ではなく実行方式として扱いたい。

対応できないoperationは明示的にfallbackする。artifactが必要とするbackend能力も検査する。

## VBACKEND-03：safe pointからdeoptする

最適化の前提が外れたとき、値・owner・frameをmaterializeして通常実行へ戻る。失敗せずに処理を続けるbackendを考えたい。

checkpointや取消と同じ境界を利用する。物理addressだけを前提にして過去stateを読めなくしない。

## VBACKEND-04：意味に沿うwork counter

最適化でinstruction数が変わっても、予算・profileの説明をoperationへ対応させる。lookupや実際のnative workは過少に数えない。

backend固有の命令数と、languageが数える仕事量を分ける。利用者が二つの数字の目的を理解できるようにしたい。

## VBACKEND-05：契約付きoperation fusion

連続する純粋なoperationを一つのkernelへまとめる。中間値を省くとき、丸め・overflow・error位置・取消粒度の変化を示したい。

同じ意味を保つfusionと、明示的な高速modeを分ける。任意callbackの効果を勝手にnative workerへ移さない。

## VBACKEND-06：profile-guidedな特殊化

実際の型・shape・分岐から、よく使う経路を特殊化する。条件が変わったら通常経路へ戻り、特殊化の根拠をprofileで見たい。

sampleの値自体を不要にcode cacheへ残さない。source/SDK/CPU等の条件をcache keyへ持つ。

## VBACKEND-07：debug-friendlyな最適化

速さだけでなく、sourceの行や値を調べられる最適化modeを用意する。消えた変数も復元可能な範囲を説明したい。

「stepしたが値が読めない」という状態を減らす。再現用記録と性能測定のmodeを選べるようにする。

## VBACKEND-08：特殊化数とcode容量の予算

genericやconst parameterが大量のcodeを作る場合、共有化・dictionary方式・上限を選ぶ。compile時間と実行費用の交換条件を見たい。

上限超過を単に深い型errorへ潰さない。どの呼出しがcodeを増やしたかをsourceへ戻す。

## VBACKEND-09：native operationの費用model

shape、byte数、iteration等から、scratchやworkの見積りを持つ。実測と予算上の見積りを比較できるreportが欲しい。

machine速度をlogical workと同じ単位へ混ぜない。adapterごとの見積りの根拠と未計測部分を残す。

## VBACKEND-10：協調停止点を説明するcompiler

長いloopやkernelで、いつ別task・取消へ応答できるかを表示する。safe pointを入れる位置と、その費用を調べたい。

停止できないforeign callは別に表示する。Taskのcancel要求だけで、実行中のnative codeが停止したと約束しない。

## VBACKEND-11：寿命情報をallocationへ使う

型検査で分かる短い寿命から、stack、scratch、共有page等を選ぶ。戻り値やsnapshotへ逃げる場合は、必要なrepresentationへ移したい。

見た目の配置変更だけでownerやidentityを変えない。allocation削減の根拠をprofileとsourceで確認する。

## VBACKEND-12：checkpoint-awareなtail call

再帰のframeを減らすとき、保存中のcheckpointやcontinuationの扱いも維持する。呼出し履歴を省いてよい場合を区別したい。

debugやrestoreで必要な情報はmaterializeする。tail call最適化が、既存のscope/呼出し境界を黙って変更しない方式を考える。
