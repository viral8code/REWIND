# task群・model更新・並行処理の契約

既存のtaskやchannelを置き換える宣言ではない。structured concurrency、版付きmodel、外部作用を一緒に扱うときの追加体験を考える。

## VCONCUR-01：権限が狭まるtask group

child taskへ渡す計算budget、接続能力、model領域を親から絞る。groupのsignatureで、共有する能力と移送するownerを読みたい。

子が親より強い能力を自動取得しない構成にする。記録用contextの継承と、操作権限の継承は別に指定する。

## VCONCUR-02：版付きの非同期結果

background計算が入力版を持つResultAt<T>を返す。完了時に、現在のmodelへ採用できるか、古い結果として残すかを選ぶ。

taskをcancelしただけで、その結果を既に他の処理が採用していないと保証しない。採用には別のmodel更新契約を使う。

## VCONCUR-03：model mailbox

一つのmodel ownerへChangeSetやcommandを送るmailbox。background taskはmodel全体を共有変更せず、更新案を提出する。

採用時の再検証、reject、rebase結果を返す。単一ownerの直列化と、複数modelを跨ぐ整合性を区別する。

## VCONCUR-04：group全体のsafe barrier

childの計算が指定したsafe pointへ揃った状態を観測し、必要なら純粋modelのsnapshotを取る。

戻せるtask、完了を待つnative call、recordした入力を使うtaskを一覧にする。一部のtaskだけが以前の版へ戻り、共有modelと食い違う状態を隠さない。

## VCONCUR-05：select後のloser policy

複数操作の最初の完了を取る時、選ばれなかった操作をcancel、drain、継続観測等のpolicyで扱う。

physical requestではcancel後も結果を照合する必要がある。競争に負けた結果を破棄したことと、操作が実行されなかったことを分ける。

## VCONCUR-06：失敗を束ねるtask結果

複数childのerror、cancel理由、成功値をExceptionGroupやGroupResultとして返す。最初のerrorだけで後続の重要な結果を失わない。

原因となった失敗と、親のcancelによる後続失敗を区別する。値・資源の所有権は各variantに対応させたい。

## VCONCUR-07：優先度の逆転を示す待機graph

低優先度taskの資源を高優先度taskが待つ状況を表示する。queue、lock、channel、native callを同じ待機graphで辿りたい。

優先度継承や公平性policyを選べる設計を考える。任意native codeのpreemptionをVMが保証する前提にはしない。

## VCONCUR-08：deterministic reduction

並列に計算した結果を、定義した順序または法則に従って集約する。入力の分割と物理完了順を変えても比較可能な結果を得たい。

浮動小数点等では順序・backendによる差を契約に含める。associativeと宣言した演算の確認にproperty testをつなぐ。

## VCONCUR-09：taskの資源領域を閉じる

groupが持つ一時buffer、cache、leaseをscopeと共に閉じる。childが返した値だけを外側のownerへ移せる。

snapshotが保持しているdata pageと、終了できるphysical leaseを分けて表示する。group終了で生きたsnapshotを破棄しない。

## VCONCUR-10：状態の違うsupervision

純粋taskの再計算、外部接続の再構築、結果不明operationの照合を別の再開strategyにする。

「例外なら全部restart」ではなく、対象の状態とownerが要求する手順を使う。以前のreceiptや入力版は再開後も照合できるようにしたい。

## VCONCUR-11：非同期の入力tape

taskへ届く値・順序・時刻を記録し、純粋modelの再実行へ供給する。物理待機から切り離した試験ができる。

記録後のchannel cursorはdataとして分岐できる。一方、実channelの消費やackは物理adapterで管理し、revertで再配信されたと扱わない。

## VCONCUR-12：終了理由を持つcancel scope

利用者の取消し、deadline、親の失敗、budget終了等を型付き理由にする。cleanupやreportは原因に応じた情報を返せる。

cancelの要求、受理、終了の三段階を観測したい。要求があるだけで子の計算と資源が終了済みという表示にしない。
