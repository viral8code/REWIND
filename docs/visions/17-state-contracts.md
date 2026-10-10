# 巻き戻しを使う関数・task・入力の契約

動的checkpoint、Snapshot<T>、checkpoint外の変数を組み合わせた自由構想です。以下の型名・syntaxは未実装です。既存branch、Result、task等の発展形を含み、現行版に基盤がないという主張ではありません。

## VSTATE-01：失敗時だけ戻すattempt block

blockがOkなら変更を残し、Errなら入口のVM stateへ戻してerrorを返す。呼出側でcommit/revert/dropを毎回書かずに、複数の変更を一つの失敗単位として扱いたい。

例えば二つのcollectionを更新して、途中の検査に失敗したら両方戻す。回復可能なErrの処理であり、実行予算を取り戻したり、fatal failureを無条件に通常Resultへ変換したりする仕組みとは分ける。

外部送信やDB commitまでatomicに取消す契約にはしない。pending I/Oを含めるか、checkpoint外stateへの書込みを許すかは明示する。入れ子のattemptでは、その入口まで戻る。

以下は説明用の擬似コード。

~~~text
let result = attempt {
    inventory.reserve(item, 1)?;
    balance.subtract(price)?;
    Ok(())
};

// Errなら両方のVM内変更を戻す。
// 実DB、送信済みmessage、消費済み予算は別の境界。
~~~

## VSTATE-02：採用できるTrial<T>

隔離した試行を実行し、戻り値とVM内の変更案をTrial<T>として受け取る。変更を比較・検査してからapplyするか、discardするかを選びたい。

既存branchの隔離を、利用者が採用する結果を選べる形へ広げる。二つの案を並べてから採用し、単に試行の値をcopyする方式と、state差分を適用する方式を区別する。

apply前に親stateが変わっていたら、競合を示す。古い試行の変更を新しいstateへ無条件に上書きせず、再検証か明示mergeを選ぶ。Trialの適用は二重に行わない。

## VSTATE-03：関数が巻き戻す範囲をsignatureへ表す

関数がstateを戻す可能性と範囲を、型/effectで知りたい。pure、通常mutation、特定modelのrollback、全VMのrevertを同じ操作として扱わない。

呼出後に使える値・binding・borrowが変わる場合、その契約をcallerへ伝える。checkpointを引数に取るhelperも、restore先のcontinuationやscopeが有効かを確認できると嬉しい。

## VSTATE-04：task群の一貫したsavepoint

関連taskのVM stateと内部queueを、一つのgroupとして保存する。producerだけを戻してconsumerが進んだままになる等の不整合を避け、simulationやparallel探索を再開したい。

group内で整合する停止点を選び、保存対象を説明する。physical worker、送信済みmessage、group外taskのstateまで巻き戻せるとは扱わず、境界を越えた操作は別の履歴として残す。

## VSTATE-05：巻き戻せる仮想入力source

入力列とcursorをVM内のdataとして持ち、revertで同じ入力を読み直せる。parser、対話UI、simulationを、現実の入力を再取得せずに試したい。

recordを読む機能の発展形として、利用者がInputTape等を組み立ててbranchごとに使う。real console/socket/deviceから消費した入力を物理的に返す機能とは区別し、仮想sourceへの差替えを明示する。

## VSTATE-06：条件に対して再開方法を選ぶrestart

回復可能な条件が起きたとき、「別の値を渡す」「この計算だけやり直す」「代替処理を選ぶ」をhandlerへ提示する。関数全体を再実行するretryより小さい単位で回復したい。

Common Lispのrestart等を参考に、選べる操作と引数をtypedにする。既存resumeのcheckpoint復元とは別の操作で、終了済みframeや公開済み外部作用を復活させない。初期の用途例はpureな入力検証・数値計算等を考える。

## VSTATE-07：版を指定できるEntityRef<T>

objectのidentityと、それを読むsnapshotの版を分ける。現在の版・過去の版で同じentityをqueryし、削除済みなら不在を通常の結果として受け取りたい。

素のheap addressを保持せず、Snapshot<T>やversion付きregistryを介して解決する。過去のentityを読めることと、現在のmutable ownerとして使えることは区別する。閉じたnative resourceにはこの方式を転用しない。

## VSTATE-08：rollbackを知らせるmodel event

stateが戻ったとき、その理由・対象・前後のversionをeventとして受け取る。checkpoint外のcacheや履歴catalogが、古いmodel版を参照したままにならないようにしたい。

onRollback等のhookは、外部の取消処理を勝手に実行するcallbackとは分ける。更新を終えたsafe pointで通知し、通知による再revertの循環やeventの保持量も説明できる仕組みにしたい。

## 特に組み合わせたいもの

- attempt + typed Result：失敗後も部分変更を残さないhelper。
- Trial<T> + snapshot diff：二つの設定・編集案を比較して採用する操作。
- checkpoint外state + 仮想入力：試行回数を残しつつ同じ入力で探索。
- EntityRef<T> + snapshot：複数版のtree/graphを同じidentityでquery。
- rollback event + version key：保持cacheの明示的な無効化。

関連する[動的checkpoint案](../v2-next-checkpoint-api.md)、[checkpoint外state案](../v2-next-retained-state.md)、[実行方式の構想](../v2-next-execution.md)を具体化する材料です。現在のruntime、syntax、版の計画は変更しません。
