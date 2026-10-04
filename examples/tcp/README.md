# TCP の送受信

`main.rw` の `PORT` を接続先のポート番号へ置き換える。相手は `ping` と送信側 EOF を受信したら `ok` を返して切断する。

```sh
rewind run main.rw --allow-effects external,network,tasks
rewind compile main.rw --allow-effects external,network,tasks
rewind run main.rwc --allow-effects external,network,tasks
```

この例は通常の記録付き外部操作を使う。pending Task を checkpoint に保持し、復元後も `ping` を再送しない。read は短い chunk を返し得るため、EOF まで繰り返す。平文 TCP であり、TLS や peer の処理完了を保証しない。長時間のアプリケーションでは `external live` と `live` effect を選択できる。新しい factory は新しい物理操作を実行するため、重複をアプリケーション側で扱う。
