# 外部作用を値として組み立てる

実際の送信・DB更新を巻き戻す案ではありません。計算中は計画をdataとして作り、実行後は結果や照合根拠を値として受け取る、という自由構想です。

## VEFFECT-01：Intent<E>で操作案を表す

file更新、送信、保存等の意図を、typedなdataとして組み立てる。意図を作るだけでは実行せず、branch内で複数案を比較したい。

操作のpayloadと、接続やcredential等の実行能力を分ける。計画を保存しても実resourceや秘密値をそのままserializeしない。

## VEFFECT-02：検証済みのPrepared<E>

入力、schema、容量、必要条件を検査した操作案へ、検証結果を関連付ける。実行直前に変わり得る条件と、再検査不要な条件を区別したい。

Preparedは既に相手が処理した意味ではない。値を作る段階とphysical操作が始まる段階を型で表したい。

## VEFFECT-03：操作の根拠を持つReceipt<T>

結果にoperation ID、input/version、送信状態、相手の識別情報を添える。値だけがstateへ戻っても、実際の操作履歴と対応できるようにしたい。

receiptから再送するかどうかを勝手に決めない。結果不明なら、照合に必要な情報を取り出せると便利。

## VEFFECT-04：一度だけ使う適用ticket

同じ計画を誤って二度applyしないためのticketを持つ。VM内で値をrevertしても、実行済みticketの記録は戻らない方式にしたい。

これはremote全体のexactly-once保証ではない。結果不明の扱いは別に残し、相手が提供する重複防止契約と接続する。

## VEFFECT-05：read set付きの操作案

計画が参照した値やversionを持ち、適用時に条件を照合する。古い残高や設定を元に作った変更を、そのまま最新stateへ適用しないようにしたい。

外部sourceはCAS等の提供する能力に合わせる。条件確認と書込みを一体で行えないbackendでは、その限界を結果へ示す。

## VEFFECT-06：依存graphを持つ操作bundle

「A成功後にB」「CとDは独立」等を操作の依存graphとして表す。実行前に順序を見て、途中失敗で何が完了したかをqueryしたい。

bundleが一つでもphysicalにatomicとは限らない。完了・未開始・不明をoperation単位で返す。

## VEFFECT-07：外部結果を取り込むreducer

physical結果を、明示したmodel更新関数へ渡す。結果が来たときの現在versionと、要求を出したversionの違いを扱いたい。

古い結果を捨てる、比較する、再検証する等をpolicyへする。到着したからといってcurrent modelを無条件に上書きしない。

## VEFFECT-08：型付きの補償案

実行済み操作とreceiptから、取消・返金・訂正等の新しい計画を作る。補償が可能な条件と、補償後のreceiptを表したい。

元の操作をなかったことにするVM restoreとは分ける。補償自体の失敗や結果不明も履歴として残す。

## VEFFECT-09：操作planのdry-run adapter

同じIntentを、simulation backendへ渡して想定結果を得る。何を仮定したかを結果へ添え、実backendの成功保証とは区別したい。

実送信を禁止した環境で、UIやworkflowの分岐を試す。dry-runと本実行の能力を明示的に切り替える。

## VEFFECT-10：outboxの明示state

保存済みの送信予定、送信開始、照合待ち、完了等をdataとして管理する。runtime終了後も再開する場合は、永続storeとそのtransaction契約を使いたい。

VMのpending I/Oと、DBに確定したoutboxを同じqueueに見せない。どこへ保存した状態かを表示する。

## VEFFECT-11：外部結果の確認義務

結果不明や部分適用を、通常成功へ暗黙に変換できない型にする。確認・放棄・補償のいずれかを明示して、次の状態へ進むAPIを考えたい。

applicationが後で確認する場合も、その状態を値として保存する。error messageを表示しただけで処理済み扱いにしない。

## VEFFECT-12：能力を弱めた計画共有

元のresourceの全能力ではなく、特定の操作・対象・容量だけを許すcapabilityを渡す。moduleやpluginへ計画を任せても、使える範囲をsignatureから知りたい。

計画のdataと実行権限を組み合わせる場を明示する。文字列の接続先を書いただけで、任意の外部操作ができる仕組みにはしない。
