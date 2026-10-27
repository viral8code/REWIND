# effect handler・継続・回復の表現

algebraic effectとrestartの構想を、REWINDのstate区分と組み合わせる。実装方法を固定せず、利用者が欲しい操作と、継続が持つ義務を考える。

## VHANDLER-01：型付きのeffect要求

ReadConfig、Choose、Report等を、引数と応答型を持つ要求として宣言する。関数はどの要求を発生させるかをsignatureへ表せる。

同じ計算を、対話・fixture・探索等のhandlerへ渡す。要求の名前だけで物理作用がrollback可能になる仕様にはしない。

## VHANDLER-02：一度だけ再開する継続

handlerが受け取るcontinuationをaffineなownerにし、resumeまたはdiscardを一度だけ選ぶ。

captureしたborrow、task、資源の寿命をcontinuationに結び付ける。普通のclosureと同じように無制限に保存できると仮定しない。

## VHANDLER-03：複製できる純粋継続

複数候補を試すため、純粋で複製可能なcaptureだけを持つ継続を分岐させたい。

VMが追跡するsnapshot可能stateを共有し、外部handleや一度だけ使うownerを持つ継続は複製しない。探索を短い構文へまとめる候補として考える。

## VHANDLER-04：handlerの委譲

handlerが一部の要求だけを処理し、残りを外側へ渡す。contextを追加するhandlerと、最終的に実行するhandlerを合成できる。

同名effectのinstanceを型付きIDで区別し、別の設定sourceや別の探索空間へ誤って渡さない設計を考える。

## VHANDLER-05：stateの扱いを選ぶhandler

local state、版付きmodel、checkpoint外のcounter等を別effect instanceとして提供する。

探索の候補を棄却した時に何が戻るかを、handlerのcontractから読める。同じget/setの名前でも、state領域の違いを隠さない。

## VHANDLER-06：回復方法を提示するrestart

失敗した処理がRetryWith、SkipRecord、UseDefault等の回復操作を提供する。呼出し側は内部例外を解析せず、型付き候補を選べる。

回復できる地点の寿命をrestart ownerへ持たせる。終了済みのscopeへ後から無制限に戻す仕組みにはしない。

## VHANDLER-07：実行前の作用の収集

handlerが実操作を行う代わりに、Intentのgraphを組み立てる。計算から計画を取り出し、previewや権限確認へ回したい。

実操作の応答を必要とする処理は、placeholderで勝手に続けず、応答後の継続や対話planとして表す。

## VHANDLER-08：回復budgetを外側に置く

retry回数、探索の仕事量、物理deadlineを最外側のhandlerで管理する。候補のrevertでbudgetが復活しない。

model内のsimulation時間と、実行を制限する物理budgetを別instanceにする。記録再生でどちらを仮想化したかも表示したい。

## VHANDLER-09：unwindとrollbackの後処理

continuationを捨てる時、local計算を戻す時、外部leaseを終了する時のhookを区別する。

一度だけ必要なphysical cleanupは実際のowner寿命に従う。過去のmodelへ戻ったという理由だけで、同じ外部closeを繰り返さない。

## VHANDLER-10：handler scopeのtask伝播

child taskがどのhandler instanceを引き継ぐかを宣言する。task-localな差替えと、group全体の共有policyを選べる。

親handlerの終了時に生きたchildが残らないstructured scopeを基本に考える。scopeから逃がす場合は、必要なownerを明示移送する。

## VHANDLER-11：要求と応答のtrace

handler chain、処理したinstance、応答の由来をtraceとして表示する。なぜfixtureでなく実adapterへ到達したかを調べたい。

記録する値はredaction policyを通し、機密の生値を既定で診断へ残さない。trace記録費用も選べるようにする。

## VHANDLER-12：handlerを特殊化して消す

静的に決まったhandlerを通常関数へ特殊化し、動的dispatchやcontinuation allocationを減らしたい。

型・作用・cleanupの意味を共通に保ち、動的handlerへ切り替えた場合も同じcontractで動作する。最適化後の展開を閲覧できると学習にも役立つ。
