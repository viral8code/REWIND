# simulation・game・roboticsのmodel

scene、数値計算、入力、時間を組み合わせた自由構想です。計算内の世界は巻き戻せても、実deviceやremoteへ確定した操作は別に扱います。

## VWORLD-01：entity/componentのmodel

多数のobjectをcomponentの組合せで管理する。所有権、query、追加削除、checkpointの共有を表し、object graphとdata-oriented処理を使い分けたい。

## VWORLD-02：決まったtickで進むworld

simulationのstepと表示frameを分け、同じ入力列から同じworldを作る。速いmachineでも遅いmachineでも同じ進捗を再現し、任意tickからbranch比較したい。

## VWORLD-03：予測と照合のrollback netcode

remote入力の到着前に予測して計算し、確定入力と異なればsimulation stateを修正する。修正したlocal計算と、既に送ったmessageや表示したframeを区別して調べたい。

## VWORLD-04：衝突・剛体・変形のsimulation

shape、mass、constraint、contactをmodelとして扱う。衝突した理由やsolverのiterationを見て、条件変更が動きへ与える影響を比較したい。

## VWORLD-05：navmeshとsteering

移動可能領域、障害物、経路、局所回避をつなぐ。graph上の最短pathだけでなく、world内の移動や動的障害を扱いたい。

## VWORLD-06：behavior treeとdecision model

agentの条件・行動・優先順位を宣言する。選ばれた行動と選ばれなかった理由をtimelineで見て、strategy変更をbranchで比べたい。

## VWORLD-07：procedural world生成

seed、rule、noise、constraintからsceneや地形を作る。生成の途中を再現し、局所だけを作り直しながら同じidentityを保ちたい。

## VWORLD-08：chunk単位のworld streaming

地形やobjectを必要な範囲だけ取得・展開する。表示state、logical world、外部asset取得を分け、cacheとcheckpointの保持関係を見たい。

## VWORLD-09：空間audioのmodel

位置、距離、反射、遮蔽からaudioを計算する。sceneと再生graphを関連付け、simulation内の結果とspeakerへ出た音を区別して調整したい。

## VWORLD-10：skeletal animationとmotion blending

骨、pose、clip、blend、inverse kinematicsを扱う。modelとanimation stateをtyped dataとして保持し、frameごとの計算や拘束を調べたい。

## VWORLD-11：assetの変換pipeline

画像、mesh、音声等をruntime用の形式へ変換する。元assetと生成物のhash、設定、依存を保持し、必要な部分だけ作り直したい。

## VWORLD-12：board gameのrule engine

state、合法手、勝敗、chanceをpureなinterfaceで表す。探索、学習、対戦UIを同じruleに接続し、counterexampleから不正な手の許可を見つけたい。

## VWORLD-13：robotのkinematicsとtrajectory

joint、限界、座標変換、目標位置から動きを計算する。simulationで安全なtrajectoryを検討し、実deviceへ送るcommandとは別の段階にしたい。

## VWORLD-14：sensor fusionとstate推定

複数の時刻・精度を持つ観測を統合し、位置や状態を推定する。遅れて届いた観測の扱い、推定誤差、filter内部stateを履歴として調べたい。

## VWORLD-15：digital twinの照合

simulationと実測の違いからparameterやmodelを調整する。仮説branchの予測を実測と並べ、実世界が巻き戻ったと誤解しない表示にしたい。

## VWORLD-16：deadlineを意識した制御計算

周期、計算期限、遅延時のpolicyを宣言する。simulationで負荷や遅延を試し、実環境で保証できる範囲と計測した範囲を分けて確認したい。
