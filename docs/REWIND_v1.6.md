# REWIND v1.6.0

HTTP / HTTPS client と、外部通信の完了を待つ Task を追加する。外部システムへの送信は publish を待たずに発生し、VM の revert では取り消さない。通信と VM の記録再生は独立して扱う。

## API

`std.http` の `get(url,deadlineMillis,maxBytes)` と `request(method,url,body,deadlineMillis,maxBytes)` は即時に送信を開始する `Task<Result<HttpResponse,HttpError>>` を返す。呼出しは `external { ... }` 内で行い、await は領域の外で行う。`external,network,tasks` の許可を CLI / manifest でも明示する。

`HttpResponse` は status、複数値を保持する `Frozen<List<HttpHeader>>`、Bytes body。Header は name と Bytes value。UTF-8 / JSON 変換は `std.bytes.decode` / `std.json` 等で別途行う。4xx / 5xx と redirect は正常な HTTP response として返し、自動追跡・retry・decompression は行わない。

`configured(method,url,body,deadlineMillis,maxBytes,headers,ca)` は header と PEM CA を指定する。`send(Request(...))` は同じ設定に公開 credential alias を加える。`credential(alias,Secret<String>)` で Authorization の値を登録し、既存 alias の異なる値への置換を拒否する。private value は要求 fingerprint と trace に入れず、公開 alias を照合する。Authorization / Cookie / proxy authentication の生 header 指定は拒否する。登録済み秘密を含む response は `HttpSecretResponse` とし、base64 化した body / header からも秘密を検査する。

URL 内の userinfo・password・fragment と HTTP/HTTPS 以外の scheme は拒否する。`component(value)` は query value / path component の UTF-8 byte を percent encode する。URL 全体の encode には使わない。

## 巻き戻し・再生

checkpoint は操作 cursor と完了 poll の cursor を保持する。送信済み操作と結果は Runtime に保持し、同じ task の同じ位置・要求は既存の in-flight 操作または結果を再利用する。異なる task / 要求は `ExternalRequestMismatch`。新しい送信には `external fresh` を使う。fresh は配信済み cursor へ進み、replay tape の未配信末尾へは進まない。

trace は完了の配信順も記録する。root 内の `.rwc` からの記録は artifact の相対位置も保持し、元の `.rw` を削除した後も `.rwc` を使って replay できる。replay は記録だけを使い、接続・送信・DNS 解決をしない。未完了操作を含む trace export は拒否する。再生と crash recovery の exactly-once は別であり、プロセス再起動後の自動再送保証は提供しない。

## Task・GUI

外部送信は async task 内でも利用できる。ただし外部領域の中では suspension しない。worker は VM heap に触れず、VM は結果を受け取ってから値へ変換する。`task.isDone()` は非同期完了を poll し、待機せず Bool を返す。main task の publish は子 Task が動作中でも、その時点の output / file / GUI の差分を確定できる。未完了の通信はそのまま継続する。

HTTP の deadline は実時間の millisecond。従来の `Task.timeout` は論理 scheduler step の上限であり同じものではない。cancel は worker に通知し、acknowledgement まで native slot を保持する。送信前を atomic に確認できた場合だけ `NotSent`、それ以外の cancel は保守的に `Unknown` とする。

## 制限・失敗

- request / response body は各 4 MiB、response header は 32 KiB / 256 entries、URL は 8 KiB、CA は 64 KiB。deadline は 1〜120,000 ms。
- Runtime あたり同時 native operation は 8、custom CA の pooled client は 8、credential alias は 64。共有 worker pool は process 内で 2 I/O threads / 最大 2 blocking workers。
- native input / result の処理は累積 native work を消費する。記録 buffer と in-flight buffer は logical memory budget に計上し、送信前に容量を確認する。既定 work を超えるデータには `--native-work` 等を明示する。
- 外部記録の export は 16 MiB、操作 / poll は各 1,000,000 を上限とする。logical budget は RSS の厳密な計測ではない。
- timeout、切断、cancel は外部で未適用の証明にならない。HttpError の phase は `NotSent` / `Unknown` / `ResponseReceived`。HTTP error code と公開 status を返し、backend の URL / 認証情報を error text に含めない。
- reqwest / rustls が system root と hostname / certificate を検証する。明示 CA は信頼を追加し、検証を無効にしない。環境 proxy 設定を尊重する。接続を再利用し、hidden retry を無効にする。

この版は上限付き body を提供する。巨大 body の全保持を避ける public chunk stream、接続型資源の scope cleanup は [v1.6.1 草案](REWIND_v1.6.1.md) で実装・検証してから DB を追加する。

## 検証

ローカル実 HTTP / TLS server で binary GET / POST、header、body limit、認証、自己署名拒否 / 明示 CA の検証、cancel、revert、offline replay を試験する。server の受信回数で再送防止を確認する。GUI のイベント処理と別 Task が通信待機中に進むことも記録と replay の両方で検証する。標準 API snapshot、source-free artifact と署名 SDK の確認は Linux / Windows CI に含める。
