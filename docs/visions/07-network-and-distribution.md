# 通信・分散・外部連携

既存HTTP/TCP/DBとサービス構想の先にある自由構想です。別systemで確定した作用はVM revertでは戻らず、照合や補償は新しい外部操作として扱います。

## VNET-01：protocol schemaからtyped clientを作る

OpenAPI、Protocol Buffers等のschemaからclient・server側の型を生成する。通信errorとapplication error、optional/NULL、version差を型として見たい。

## VNET-02：typed RPCとlocal置換

同じinterfaceをlocal実装、test fixture、remote実装へ明示的に接続する。remote呼出しはeffectとdeadlineを持ち、local関数と同じ見た目でも通信境界を確認できると便利。

## VNET-03：message brokerへの接続

publish、subscribe、ack、再配送、dead-letterを型付きmessageで扱う。VMのpublishとbrokerへの送信を混同せず、message IDと処理結果を対応させたい。

## VNET-04：永続workflow

長時間の承認待ちや複数serviceの進捗を、保存可能なworkflow stateとして表す。processが終了しても進捗を再開でき、実行済みの外部操作は照合して二重に行わないようにしたい。

## VNET-05：sagaと補償の可視化

予約、決済、登録等の操作と、取消・返金等の補償を明示する。補償も失敗し得る新しい操作として表示し、VM revertによる完全な復元とは分けたい。

## VNET-06：webhookの受信と検証

signature、重複event、時刻、payload schemaを扱う。検証と業務処理を分け、受信eventをlocal fixtureへ変換してapplicationを試したい。

## VNET-07：双方向streaming RPC

messageの流れ、順序、backpressure、half-closeをtyped streamとして扱う。送信済み・受信済み・処理済みの位置を別に追い、切断後の再開契約を表したい。

## VNET-08：service discoveryと接続先policy

接続先の選択、health、credential世代、failoverを明示する。接続先が変わったことをtraceへ残し、結果不明の処理を新serverへ無条件に再送しない方式にしたい。

## VNET-09：circuit breakerとbulkhead

失敗が増えた依存先への呼出しを制限し、他の処理の資源を守る。開閉状態、試行、同時実行枠を表示して、止まった理由をapplicationから説明したい。

## VNET-10：rate limitとquotaの宣言

相手serviceや利用者ごとに、件数・byte数・同時実行枠を管理する。待機、拒否、予約、解放をtyped結果として扱い、wall timeとlogical予算を区別したい。

## VNET-11：暦・期限・scheduleのworkflow

定期実行、営業日、zone、期限切れを明示modelとして扱う。simulationの時計で試せる一方、実scheduleで発火済みのeventは別の履歴として確認したい。

## VNET-12：systemを跨ぐtraceの対応

request、message、DB、workerへ関連IDを渡し、各systemの観測をつなぐ。時計差を考慮して因果関係を表示し、REWIND内部のtimelineとも対応させたい。

## VNET-13：CRDTと協調編集

text、set、counter等を競合して編集するためのデータ型を提供する。local Undoと他者の変更を消す操作を区別し、同期後にも意図した編集履歴が残ると嬉しい。

## VNET-14：複数sourceのquery

local dataset、DB、remote APIを一つのquery定義で扱う。どこで処理するか、何を取得したか、失敗時にどこまで結果があるかをplanと一緒に見たい。

## VNET-15：配送失敗の再処理workbench

dead-letterや失敗eventを分類し、原因修正後の再処理をpreviewする。既に成功したside effectを照合し、利用者が対象と操作を選んで再実行したい。

## VNET-16：protocol state machineの表示

handshake、request、response、closeを状態遷移で記述する。recordから現在状態と違反した遷移を見て、不正な相手や途中切断の処理を調べたい。
