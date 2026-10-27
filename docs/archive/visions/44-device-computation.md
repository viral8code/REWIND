# GPU・device計算の値と実行plan

GPUやdevice arrayの構想を、dataの版、transfer、実行完了、memory費用まで広げる。physical deviceの状態をVM全体のcheckpointへ無理に含めない設計を考える。

## VDEVICE-01：限定したkernel言語

typedな式とarray操作からkernelを生成する。許すeffect、control flow、型、allocationを通常のREWIND codeと区別する。

対応しない処理はcompiler診断として返す。任意関数がdeviceで動くように見せず、hostとの境界をsource上で確認したい。

## VDEVICE-02：address spaceを持つview

host、device local、shared、read-only等の領域を型へ表す。shape、stride、alignmentもkernel signatureへ渡す。

同じ長さのbytesでもmemory領域が違えば別のviewになる。pointerの数値が同じという理由で別deviceへ渡さない。

## VDEVICE-03：device dataの論理版

kernelの出力を新しいdata版として扱い、入力版をそのまま書き換えない契約を考える。

物理bufferを再利用できる場合も、古い論理版を保持するownerを確認する。snapshotへ残す方法はhost copy、device保持、再計算等から明示選択する。

## VDEVICE-04：fenceを持つ返値

launchの返値に完了fenceと結果ownerを持たせる。計算済みのdataとして読む前に完了を確認する。

launch成功、kernel完了、hostへの転送完了を別段階にする。要求をcancelしただけで実行中bufferを解放しない。

## VDEVICE-05：転送を含むcost plan

kernel時間だけでなく、copy、layout変換、page移動、同期をplanへ含める。

小さい入力ではhost計算の方が有利な場合を選べる。zero-copyという名称だけで、page移動や同期の費用がないと表示しない。

## VDEVICE-06：buffer再利用のowner

poolはfence完了とlive owner不在を確認して再利用する。利用者がpinした旧版のbufferは、単なるcacheとしてevictしない。

capacity、live、pending、history保持を別のmemory会計にする。deviceのallocation失敗からhostへ戻るpolicyも選びたい。

## VDEVICE-07：shapeごとのkernel特殊化

shape、dtype、tile、backend等に応じたkernelを生成し、compiled cacheへ残す。

特殊化の数とcode量にbudgetを付ける。runtime入力の小さな違いごとにcompileが走る場合、その理由を診断したい。

## VDEVICE-08：reductionの意味を選ぶ

順序固定、近似許容、exactに近い蓄積等を、device reductionのcontractとして選ぶ。

hardwareやthread数を変えた結果の差を測定する。速度優先modeを再現modeと同じbit結果の保証に読み替えない。

## VDEVICE-09：依存graphでlaunchを合成

kernelとtransferの依存をgraphへまとめ、不要な同期を減らす。利用者はdata依存とqueue間の待機を見られる。

graphの再実行は純粋data計算へ限定する。外部表示・sensor操作等を同じ再計算可能graphへ無条件に含めない。

## VDEVICE-10：検査用のkernel実行

小さな入力をhost reference、bounds-check版、device版で比較する。shape違反、raceの疑い、数値差を再現dataへ残す。

検査backendの対応範囲と費用を明示する。少数caseの一致から全kernelの正しさを証明したとは扱わない。

## VDEVICE-11：device lossの結果

device消失、memory不足、timeout等を、data ownerと実行状態を伴うerrorにする。

host snapshotや再計算planがある場合だけ復旧候補を出す。deviceが失ったbufferをrevertで再び有効にしない。

## VDEVICE-12：入力版を固定したautotuning

tile、kernel variant、batch、配置を同じ入力版で測り、結果policyを保存する。warm-up、転送、計測環境をreportへ残す。

候補の試行は純粋計算にする。新しい入力やdeviceへ適用する時、測定した範囲を外れていれば再評価するpolicyを選べる。
