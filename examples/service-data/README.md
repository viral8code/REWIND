# GUI・HTTP サーバー・DB の組合せ

同じプログラムでネイティブのウィンドウを表示し、HTTP リクエストを
SQLite または PostgreSQL のトランザクションで処理します。
ウィンドウの「Check responsiveness」は処理中も入力できます。
画面の更新と `publish` はアプリケーション側のタスクが行い、サービス側は
容量1の channel で進行状況を通知します。

SQLite で起動する例です。引数は backend、port、ウィンドウのタイトル、
DB 行のキーです。最後のキーには実行ごとに新しい値を指定してください。

```sh
rewind run main.rw --allow-effects gui,external,network,db,tasks,env,fileRead,output -- sqlite 8080 "REWIND service" session-1
```

別の端末から順番に送信します。

```sh
curl -X POST http://127.0.0.1:8080/rollback --data "saved text"
curl -X POST http://127.0.0.1:8080/save --data "saved text"
curl -X POST http://127.0.0.1:8080/shutdown
```

最初は重複キーによる途中失敗を起こし、transaction を rollback して
`rolled back` を返します。次は transaction を commit し、`saved` を返します。
3番目のリクエストで listener を閉じて終了します。この終了リクエストの
接続は応答なしで閉じるため、curl は Empty reply を報告します。

DB は `state.sqlite` の `rewind_service_data` テーブルに保存されます。
画面の checkpoint を戻しても確定した行は残り、完了済みサービス task の
結果を再取得しても HTTP 応答や DB 書込みは再実行されません。
cursor は明示的に close し、その完了を待ってから接続を再利用します。

PostgreSQL の場合は、検証用 CA を `ca.der` に置き、接続文字列を
`REWIND_PG_DSN` 環境変数に設定します。

```sh
rewind run main.rw --allow-effects gui,external,network,db,tasks,env,fileRead,output --secret-env REWIND_PG_DSN -- postgres 8080 "REWIND service" session-2
```

`rewindc main.rw --allow-effects gui,external,network,db,tasks,env,fileRead,output`
でコンパイルした後も、`main.rwc` を同じ引数と effect 許可で実行できます。
配布試験では実際の GUI、両 DB、HTTP、途中失敗、資源解放、接続なしの
source-free replay を確認します。
