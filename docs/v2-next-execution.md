# 追加構想：実行方式・並列性・計算モデル

状態：採用判断前。版番号は未割当です。[全体計画](ROADMAP_v2-next.md)へ追加する8候補です。現行の協調Task、native kernel、GC、記録/replay、artifactを出発点にします。速度改善だけを理由にcheckpointや予算を無効化しません。

## EXEC-01：SIMDとCPU特化kernel

まず既存build/native dependencyが行う最適化を確認します。追加候補はnumeric elementwise/reduction、Bytes比較、codec等の限定kernelです。汎用vector型を言語へ追加するかは別判断にします。

CPU featureの検出とfallback、alignment、tail処理、NaN/overflow、演算順、fused roundingを規定します。logical workは利用したISAの命令数に依存させません。高速CPUだけが予算を回避する契約にはしません。

**受入：** scalar参照解、stride/view、短い入力、tail、非対応CPUを確認する。誤差契約と取消chunkを保つ。改善が測定できないkernelは採用しない。

## EXEC-02：pureな並列計算

既存Taskの協調実行と、複数CPU coreで計算する方式を区別します。初期は既存native kernelの独立chunkを並列化する候補です。任意REWIND closureを外部threadへ移すことは別設計にします。

immutable入力の共有、出力の分割所有、固定chunk順、worker数上限、scratch予約、失敗の選択規則、cancel後のjoinを決めます。reduction順を固定する方式と、精度契約を変える高速modeは区別します。worker完了順をVM観測の順へ無条件に流しません。

**受入：** worker数を変えたときの値/誤差/record契約を確認する。取り消したworkerが共有pageを書き換えない。thread oversubscriptionとsmall inputの費用を測定する。

## EXEC-03：JITまたはAOTの限定導入

現行VMのdispatch/型付きIR/artifact経路をprofileしてから、hot functionをnative codeへ変換する価値を判断します。最初はpureな整数・数値loop等に範囲を限定する案です。

必要条件はsteps/native work、safe point、GC root、checkpoint境界、debug source位置、error/overflow、deoptimizationです。commit/revert/publish/awaitをまたぐ最適化は、境界を証明できるまで行いません。

machine code cacheはCPU/OS/ABI/compiler/languageをkeyにし、通常のportable artifactと分けます。cache改変、code memory上限、実行可能memoryのOS制約、cold-startを評価します。

**受入：** interpreterと同じ値・診断・予算失敗・記録結果。debug/profileで説明可能。native化によるsemantic driftや起動費用が大きければ保留する。

## EXEC-04：incremental compileと依存cache

大型project管理の追加とは分け、同じsourceを繰り返しcheck/compileする費用を減らす候補です。現行cache/resolverとcompiler予算を先に確認します。

cache keyはsource、公開signature、trait impl、generic bounds、effects、SDK署名、language、compiler設定を含めます。partial/生成codeでは全fragmentとgenerator入力の変更を反映します。private bodyだけの変更がどこまで再検査を必要とするかを定義します。

**受入：** clean buildと同じ診断・artifact。cache不正/中断で正常な結果へfallbackする。cache容量がbounded。未変更部分を何も検査せず信頼する設計にしない。

## EXEC-05：generator・async iterator

現行Iteratorはsnapshotとcursorを持ちます。追加候補は、逐次値を返すgeneratorと、非同期入力を次の値へ変換するasync iteratorです。

初期はlibrary state machineで成立する例と構文案を比較します。suspend時のlocal/borrow/capture、ownerを含むyield、終了・error・cancel、stack depthを規定します。external領域内でawaitしない現行契約を保ちます。

pure generatorのVM内cursorはcheckpointへ保持できる候補です。physical streamの受信をrevertで再実行するasync iteratorにはしません。記録済み入力の再読取りと新しい入力取得を分けます。

**受入：** range、parser、bounded streamの例。borrow逃避と終了後利用を拒否。consumer中断時にresourceを解放し、全件materializeしない。

## EXEC-06：channelとactorの型付き配送

まず現行Task連携で可能な操作を確認します。候補はbounded channel、send/receive、close、select、actorのmessage loopです。任意共有mutable stateを増やす機能にしません。

Shareなmessageとowner transferを別契約にします。channel stateがVM内なのかphysical workerとの境界なのかを分けます。revert時のqueue、取消済receiver、送信済ownerの扱いを定義できないままAPIを公開しません。

**受入：** 満杯/空queue、sender/receiver終了、取消、select tie、owner移動、deadlockを確認する。OS thread完了順を理由なくrecordの外へ出さない。queueのbytes/件数とtask資源を会計する。

## EXEC-07：reactiveなmodel計算

GUIの派生値や数値pipelineを、依存変更時だけ更新する案です。最初はpureなderived valueと明示的なdirty graphに絞ります。再計算のためにHTTP/DBを勝手に呼ぶwatcherを既定にしません。

依存cycle、subscription寿命、batch更新、更新順、errorの保持、checkpoint後の無効化を規定します。object addressではなくVM内の安定したidentity/versionを使います。全heapを追跡する常時instrumentationは避けます。

**受入：** 全再計算した参照結果と一致する。revert後に古いcacheを返さない。subscription解除とGCで依存graphが残り続けない。GUI公開は利用者が明示する。

## EXEC-08：portableな状態保存と再開

現行record/replay・model保存と、一般のapplication state保存を区別します。初期候補は明示schemaを持つVMデータのexport/importです。任意のcall stack/continuation/native handleをdiskへ保存する機能とはしません。

対応型、version、checksum/署名、参照共有/cycle、Secret、容量/深さを規定します。外部resourceは保存対象から除外するか再接続用の非機密設定だけを別に持ちます。再接続は新しいphysical操作です。

**受入：** 新processでデータを復元でき、旧schemaの移行または明示拒否がある。保存済み状態を読み込んでもSQL/出力を再送しない。保存時の部分File公開失敗を扱う。

## 大きな後続候補

GPU/acceleratorは別案です。host/device transfer、device memory、kernel起動、cancel、精度、record、driver/配布を計測してから判断します。今回の8候補へ自動的には含めません。

複数process間のVM state共有やdistributed revertも別範囲です。相手processで確定した作用をREWINDだけで巻き戻せる仕様にはしません。

## 調査の順序

EXEC-01/04は既存profile/cacheを調べてから、差分がある場合だけ選びます。EXEC-02はnative ownershipとmemory会計を先に固めます。EXEC-05/07はlibrary state machineで成立する例を優先します。

EXEC-03/06/08はVM、形式、continuation、資源寿命の設計が大きいため、独立したprototypeと互換性判断が必要です。採用は具体的な利用例・費用・受入条件に基づきます。計画文書の追加だけで処理系を変更しません。
