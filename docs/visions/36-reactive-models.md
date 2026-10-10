# reactive graph・画面・modelの再計算

Signalや差分queryの構想を、modelの版とUIの寿命へ結び付ける。modelをrevertした時にも、一貫した値を見せ、無駄な再計算を説明できる仕組みを考える。

## VREACT-01：glitchのない更新単位

複数fieldを一つのmodel transactionで変更し、派生値を同じ版で計算してから通知する。

途中の一方だけが更新された状態をUIへ出さない。通知を待つことと、外部作用をpublishすることは別の境界として扱う。

## VREACT-02：read setから作る依存

派生値を評価した時に実際に読んだfield pathを記録する。分岐条件が変われば依存も変え、不要になったedgeを外す。

暗黙の依存追跡を使わない明示modeも欲しい。計算が追跡できない外部状態を参照する場合は、純粋な派生値の扱いから外す。

## VREACT-03：動的なgraphの循環診断

signalやqueryの依存に循環が生まれた時、経路と今回の変更を示す。単にstack overflowへ至る前に説明したい。

明示的なdelay、tick、fixed pointを使う循環とは区別する。静的に解けない循環を自動で無限に反復する構想にはしない。

## VREACT-04：画面に必要な値だけ評価

非表示panelや折り畳まれたtreeの派生計算を遅延させる。可視領域と利用者の要求が、再計算の需要になる。

監視用の不変条件等は表示の有無で省略しない。表示目的の計算と、modelが必須とする検証を別契約にする。

## VREACT-05：model rollbackの通知

通常編集のChangeSetと、以前のrootへ戻るResetToVersionを別eventとして流す。受け手は差分適用または再読込みを選ぶ。

Rollback eventは既に外へ出た通知を取り消すものではない。受け手が過去通知を保持する場合、その意味を対応する版と共に持たせる。

## VREACT-06：async derived value

重い派生計算をbackgroundで行い、Pending、ReadyAt、Stale、Failed等で返す。以前の値を表示しながら新しい計算を待てる。

現在の入力版と合わない結果を自動採用しない。stale表示のpolicyを、利用者向けの画面とmodel上の判断で別に選びたい。

## VREACT-07：subscriptionのowner

viewやpanelがsubscriptionを所有し、scope終了時に解除する。callback、capture、依存graphの生存期間を一緒に管理する。

履歴を持つsubscriptionと、現在値だけを見るsubscriptionを分ける。画面を閉じてもsnapshotをpinする明示ownerは保持される。

## VREACT-08：entity keyによるUI identity

Listの順序が変わっても、同じEntity IDのfocus、selection、編集中bufferを対応付ける。

data rootの変更とnative widgetのidentityを区別する。snapshotの復元で消えたentityへのfocusは、明示policyで解除または別対象へ移す。

## VREACT-09：deltaを運ぶsignal

変更済みの値だけでなく、old/new版とChangeSetを運ぶsignalを考える。集計やchartは差分から更新できる。

差分を取り逃した受け手には、現在rootへのresetを返す。古いdeltaを別のbaseへ適用しても整合するように装わない。

## VREACT-10：固定点計算のcontract

相互依存するruleや解析に、monotoneな更新と停止条件を明示したfixed-point nodeを用意する。

収束しない場合、iteration limitや振動した状態を返す。単なるreactive graphの循環を、証拠なしに収束可能と扱わない。

## VREACT-11：再計算の理由を説明する

どのfield、版、cache missが再計算を引き起こしたかをgraphで見たい。何も変わっていないのに画面が重い原因を調べられる。

記録levelを選び、通常実行のobserver overheadを抑える。費用は値の評価と説明の取得に分けて表示する。

## VREACT-12：pure graphとeffect sink

派生計算のgraphは値を返すだけにし、保存・送信・音等の操作は明示的なeffect sinkへ出す。

recomputeやrevertで純粋計算が再評価されても、同じ外部作用を繰り返さない構成を考える。sink側はIntent、receipt、重複判定等の独立contractを使う。
