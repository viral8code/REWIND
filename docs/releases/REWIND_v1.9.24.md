# REWIND v1.9.24

## TCP の TLS 接続

`std.tcp.connectTls(host,port,deadlineMillis)` と `std.tcp.configuredTls(host,port,ca,deadlineMillis)` を追加する。結果は既存の `Task<Result<TcpSocket,TcpError>>`。Rustls / tokio-rustls の TLS 1.2 / 1.3、ring の暗号実装を使い、既存の read / write / shutdownWrite / close を利用できる。

`connectTls` は OS の trust store を使う。`configuredTls` の空 `Bytes` は同じ動作。空でない PEM bundle は明示的な trust roots として使い、64 KiB までに限定する。hostname と certificate の検証を無効化するオプションは追加しない。client certificate 認証や ALPN 設定は含めない。平文の `connect` と区別して選択する。

external region で factory を呼び、region 外で await する。permission、recorded / live、checkpoint に保持された既存 Task の結果、非復元の物理 socket は [v1.9.23 の TCP](REWIND_v1.9.23.md) と同じ。

## 失敗と費用

接続と TLS handshake に一つの deadline を適用する。hostname / CA の形式が不正な場合は `TcpTlsName` / `TcpTlsCa`、CA が上限を超えた場合は `TcpTlsLimit`。certificate / hostname の検証や TLS handshake の失敗は `TcpTls`。これらが `NotConnected` となる場合、TLS session は成立していないが、下位の TCP 接続や TLS handshake bytes が存在しなかったことまでは意味しない。application bytes の自動 retry は行わない。

write は全 plaintext bytes を transport に渡した後、TLS record の flush も deadline の範囲で待つ。成功は peer の処理完了を意味しない。`acceptedBytes` はローカル transport が受理した plaintext bytes の確認済み量であり、ciphertext の wire byte 数ではない。途中の cancel / timeout / flush failure は remote outcome を `Unknown` と扱う。

正常な TLS `close_notify` は read の EOF として扱う。TLS の不完全な切断は `TcpRead` と区別し、暗号化 stream の途切れを正常 EOF と偽らない。明示 close / scope exit は socket を即時解放し、graceful TLS shutdown の完了を保証しない。送信側終了には `shutdownWrite` を選ぶ。

plain / TLS を共通の bounded transport interface で扱い、socket の read と write は同時に待機できる。TLS config / session には socket / pending job ごとに追加 4 MiB の保守的な予約を行う。OS trust store の config は process 内で一つを共有する。custom CA ごとの無制限 cache は作らず、connection とともに解放する。abort 中の task は終了確認まで予約を維持する。完了を確認した handle の buffer 予約は、新しい通信を行わなくても予算会計から外す。

TCP の worker 予約を 16 MiB とし、二つの async worker と最大八つの blocking resolver、各 1 MiB の thread stack を上限管理する。数値の IP address は DNS を呼ばない。hostname の lookup は process 内で最大八つ、返す address は最大 64。OS の blocking DNS lookup は task の cancel だけでは中断できないため、blocking work 自体が終わるまで admission を保持し、繰り返す cancel によって DNS thread / queue を増やさない。満杯は `TcpBusy`、解決失敗 / 上限超過は `TcpResolve` / `TcpResolveLimit`。application bytes の retry と、接続前の address 選択を区別する。

既知の秘密値を host / 公開 CA / body に含める要求は fingerprint の前に拒否する。TLS で受信する plaintext も rolling secret matcher と context-change 検査を通す。handshake の certificate 情報や crypto のエラー詳細を public replay wire に混入させない。

## 検証と残り

実 TLS fixture による trust / hostname の検証、malformed CA の送信前拒否、バイナリ送受信、EOF、checkpoint の結果再利用、source-free の compiled 実行と disconnected replay を検証する。両 OS の extracted SDK で recorded / live と結果の寿命を確認し、TLS の API を SDK / baseline に含める。

HTTP server / router、GUI・HTTP・両 DB の統合の残り、native kernel の協調分割、一般 container の費用・capacity と v2.0 の受入条件は継続する。
