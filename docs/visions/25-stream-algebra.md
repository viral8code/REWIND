# streamとeventの合成

HTTP/CSV/JSON stream、Task、仮想入力、async iteratorの構想をつなぐ自由構想です。入力の取得と、取得済みdataの変換をoperationとして分けます。

## VSTREAM-01：終了状態を持つStream<T>

次の値だけでなく、正常終了、失敗、取消、打切りを表す。Noneだけでは理由を伝えにくいpipelineをtypedな結果として扱いたい。

terminal状態へ入った後の操作を契約へする。途中まで得た結果と、全件処理した結果の違いも保つ。

## VSTREAM-02：demandを返すconsumer

consumerが必要件数・byte数を要求し、producerがそれに合わせて進む。先読みとbuffer容量を明示して、遅いconsumerへ無限に溜めないpipelineにしたい。

要求を取り消した場合の処理も表す。logicalなdemandとphysicalなread開始を分けて表示する。

## VSTREAM-03：複数consumerへの配信

一つのsourceを複数の変換・表示へ分ける。全consumerへ届けるか、独立のcursorを持つか、遅いconsumerをどう扱うかを選びたい。

ownerを含む要素は無条件に複製しない。immutable共有、copy、単一consumerへのmoveを区別する。

## VSTREAM-04：複数sourceのfan-in

sourceを統合するとき、到着順、source順、event time等をpolicyとして選ぶ。どのsourceの値かを保持し、後から順序を説明したい。

同時にreadyなsourceの扱いも明示する。scheduler choiceやphysical到着をrecordと対応させる。

## VSTREAM-05：訂正eventを持つ集計

遅い入力によって、既に計算したaggregateを訂正する。旧結果を消すのではなく、retractionやreplacementをtyped eventとして返したい。

内部集計stateの修正と、外部へ公開済み結果の訂正通知を分ける。chartやreportにも同じ意味を渡す。

## VSTREAM-06：ReplayableとLiveの区分

保存済み入力から再読できるstreamと、新しいphysical入力を読むstreamを型や説明で区別する。branchへ渡せる入力をcallerが知りたい。

replayが本物のsocketを再び読むことにはならない。保存している範囲と、未取得の範囲を表示する。

## VSTREAM-07：処理完了を表すack

値を受け取ったことと、利用者の処理が成功したことを分ける。ack、再処理、失敗保留をconsumer側のoperationとして扱いたい。

内部queueのcursorと、broker等のphysical ackは別の境界。result receiptやoutboxの構想とも接続する。

## VSTREAM-08：取消が伝わるpipeline

consumer終了時に不要な上流task・buffer・cursorを整理する。取消要求、最後の値、cleanup完了を順序付きで確認したい。

他consumerが使うshared sourceを勝手に止めない。pipelineの所有関係から取消の範囲を決める。

## VSTREAM-09：cold/hot sourceの選択

要求されてから始めるsourceと、既に発生しているeventを受けるsourceを分ける。購読開始前の値を取得できるかをcontractへする。

hot sourceのbufferをrevertで読む場合も、新しいphysical入力の再取得とは区別する。購読を作るoperationの寿命を持たせる。

## VSTREAM-10：schema変更を含むstream

入力途中でformatやschemaが変わる場合に、変更eventとdataをtypedに受け取る。知らないversionを黙って旧型として解釈しないようにしたい。

変換adapterを途中から切り替える場合も、処理したversionを結果へ保持する。失敗sampleをfixtureへ切り出せると便利。

## VSTREAM-11：chunk単位の変換

要素ごとのcallbackと、native配列・Bytesのchunkを扱うoperationを選ぶ。scalar化やList化が不要な区間は、そのまま共有・変換したい。

partial chunk、残り入力、error位置を返す。まとめることによる追加費用と、cancelの応答粒度を説明する。

## VSTREAM-12：keyごとのstateful operation

key別の集計、直近値、windowをpipelineへ置く。state数、保持期間、evictionを利用者が指定できるようにしたい。

checkpointに保持するstateと、外部sourceの取得位置を分ける。固定したkey数なら長時間実行で保持量が増え続けない方式が欲しい。
