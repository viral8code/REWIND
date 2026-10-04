# REWIND v1.9.23

## TCP の範囲

`std.tcp` に `connect`, `read`, `write`, `shutdownWrite`, `close` を追加する。Tokio の TCP transport を使い、平文の双方向 byte stream を扱う。HTTP / TLS の代替として証明書検証や暗号化を保証しない。HTTP server、TCP の TLS 設定はこの版に含めない。

いずれも `external,network,tasks` を必要とする。external region で Task を作成し、region の外で await する。通常の recorded 操作と `external live` の両方を使える。Live では追加の `live` permission が必要。

| API | Task の結果 | 条件 |
| --- | --- | --- |
| connect(host, port, deadlineMillis) | Result<TcpSocket,TcpError> | host 1..253 bytes、port 1..65535 |
| read(&mut socket, maxBytes, deadlineMillis) | Result<Option<Bytes>,TcpError> | 1..65536 bytes、EOF は None |
| write(&mut socket, body, deadlineMillis) | Result<Int,TcpError> | 一回 65536 bytes 以下、成功時は全 byte 数 |
| shutdownWrite(&mut socket, deadlineMillis) | Result<Unit,TcpError> | 送信側だけ終了し、read は維持 |
| close(&mut socket, deadlineMillis) | Result<Unit,TcpError> | 両側を閉じ、pending 操作も終了 |

deadline は各操作の開始から 1..120000 ms。read は一回の transport read であり、指定 byte 数まで満たす保証はない。必要な framing はアプリケーションが実装する。一つの socket に一つの pending read と一つの pending write を許可し、両方向は並行して動作できる。競合する操作は `TcpBusy`。同時 socket と native operation はそれぞれ最大 8。

## 物理作用と失敗

`TcpSocket` は affine な scope-owned handle。公開フィールドは `peer:String`。scope exit、明示 close、read/write の timeout、cancel、transport failure で物理 socket を閉じる。VM の checkpoint が socket を保持しても、物理的に閉じた接続は復元・再接続しない。

保持された既存 Task の結果は checkpoint の復元後も再利用する。記録付き実行は同じ観測地点の操作を再送せず、source-free / disconnected replay に結果を用いる。Live の新規 factory は新規物理操作を実行する。DNS 解決 / 接続先の address 選択は transport の動作であり、application bytes を自動再送・再接続する機能は追加しない。

`TcpError` は `code:String`, `phase:String`, `acceptedBytes:Int`。phase は `NotSent`, `NotConnected`, `Connected`, `Received`, `PartialWrite`, `Unknown`。主な code は `TcpAddress`, `TcpLimit`, `TcpDeadline`, `TcpBusy`, `TcpSocketLimit`, `TcpClosed`, `TcpWriteClosed`, `TcpCancelled`, `TcpConnect`, `TcpRead`, `TcpWrite`, `TcpShutdown`, `TcpMemoryLimit`, `TcpSecretResponse`, `TcpSecretContextChanged`。

成功した write の byte 数はローカル transport が受理した量であり、peer の処理完了を意味しない。部分送信の失敗は、観測済みの受理量を `acceptedBytes` に返す。特に `Unknown` のキャンセルは追加の in-flight bytes があり得るため、値は確認済み量の下限として扱う。read のキャンセルも、物理入力が消費されなかったことまでは保証しない。いずれも接続を閉じ、自動 retry はしない。元のキャンセルした Task は従来の `TaskError::Cancelled`、その前の checkpoint から再度 await した Task は保存済みの transport receipt を返す。

## 予算、秘密値、寿命

要求の fingerprint と response buffer、transport worker、socket、pending / abort 中の job、secret matcher の上限を物理操作前に予算検査する。秘密 pattern は既存 HTTP と同じ bounded automaton を再利用し、socket の read 間で rolling state を保持する。既知の秘密値を host / body に直接含める要求は fingerprint 作成前に拒否する。分割受信で秘密 pattern が完成した場合は、その chunk を返さず socket を閉じる。socket の作成後に秘密値の登録が変わった場合も `TcpSecretContextChanged` として閉じる。新規 read と、すでに pending な read の完了の両方を検査し、matcher を途中でリセットして分割 pattern を見逃さない。要求の guard も bounded automaton で走査し、pattern の構築と走査を予算に含める。

外部操作前の admission と独立に、VM の allocation / native work 上限超過は従来の fatal diagnostic になり得る。TLS の credential API は追加していない。

native host の予約費用は worker 8 MiB、socket ごとに 1 MiB と matcher の bounded estimate、job ごとの chunk / serialization と matcher 保持費用。OS 全体の厳密な RSS 計測値ではない。abort した job は終了確認まで費用と容量を保持し、完了した handle を逐次回収する。歴代 socket ID の tombstone は保持しない。Live の未受領 connect 結果は最後の Task / checkpoint lease とともに閉じ、受領した handle は scope の資源管理へ渡す。

`tcp` を SDK と API index に含め、source-free の送信 / 半閉鎖 / EOF / checkpoint と disconnected replay を両 OS で検証する。既存 GUI・HTTP・DB の契約は維持する。HTTP server、残る GUI / PostgreSQL の統合、長い native kernel、一般 container の費用・capacity と v2.0 の受入条件は継続する。
