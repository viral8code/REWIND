# REWIND v1.6.1

v1.6.0 の上限付き HTTP client に逐次送受信と接続の所有権を追加する。外部作用、TLS、credential、Task と replay の基本契約は [v1.6](REWIND_v1.6.md) を引き継ぐ。DB は未実装で、後続版で同じ資源の契約を利用する。

## API と所有権

`std.http` に次を追加する。いずれも呼出しは `external { ... }` 内、await は外で行い、`external,network,tasks` を許可する。

| 呼出し | Task の成功値 | 動作 |
| --- | --- | --- |
| `download(Request)` | `Result<HttpDownload,HttpError>` | headers を取得し、body を保持する接続を返す |
| `read(&mut connection,maxBytes)` | `Result<Option<Bytes>,HttpError>` | 次の chunk。EOF は None |
| `close(&mut connection)` | `Result<Unit,HttpError>` | download を閉じ、pending read をキャンセル |
| `upload(Request,maxUploadBytes)` | `Result<HttpUpload,HttpError>` | body producer を開く。Request.body は空 Bytes |
| `write(&mut connection,Bytes)` | `Result<Unit,HttpError>` | 上限付きキューへ chunk を渡す |
| `finish(&mut connection)` | `Result<HttpResponse,HttpError>` | body を終了し、上限付き応答を取得 |
| `closeUpload(&mut connection)` | `Result<Unit,HttpError>` | upload driver と pending operation をキャンセル |

`HttpDownload` の公開 field は status / Frozen headers、`HttpUpload` は応答容量 maxBytes。内部の論理 ID / ownership lease はソースから指定できない。型の constructor で作った値を接続として使うと NativeResourceInvalid。実接続は標準 API の成功値から取得する。

接続は Send、Share ではない。別 Task へ渡す場合も move する。freeze と暗黙のコピーを拒否し、generic 経由にも実行時の freeze 検査を行う。接続を含む Task の完了値は一度だけ取り出せる。再度の await は TaskResultTransferred。ignore / detach した接続結果は解放し、後からの取得は NativeResultDiscarded。

所有変数、所有引数、所有 capture はスコープ終了・return・キャンセル時に閉じる。return / move は所有情報を移し、元の cleanup で閉じない。`using connection=move owner;` も利用できる。借用された引数や通常の `match result` は所有者を変えず、所有者のスコープまで有効。`match move result` は接続を取り出して所有する。borrow は await をまたがせず、read / write の Task を先に取得してから await する。

## 容量・費用

- read は 1〜65,536 bytes、write は 0〜65,536 bytes。transfer 累積上限は 1 GiB。upload の応答上限は 4 MiB。
- download は 256 KiB の frame 上限と秘密値の lookahead を持つ。body 全体を List / JSON byte array に変換しない。大きすぎる transport frame は HttpFrameLimit。
- upload は最大 1 個の送信待ち chunk。キューが空くまで write Task を待機させる。immutable Bytes の backing storage を共有し、送信のための全体コピーを避ける。
- 各接続に同時 read、または write / finish は 1 個。重複は HttpDownloadBusy / HttpUploadBusy。Runtime ごとに download / upload は各 8 接続、native operation は 8 個。
- timeoutMillis は接続開始から最後の read / write / finish までの全期間を制限する。1〜120,000 ms。期限切れ・途中切断・容量超過で接続を失効させる。
- VM 外の buffer、matcher、応答の base64 化に必要な一時領域、キャンセル完了待ちも logical memory budget に含める。native work は入力・出力処理を計上する。RSS の厳密な上限を保証するものではない。
- 外部記録は引き続き 16 MiB、operation / poll は各 1,000,000 の上限。逐次転送の累積容量設定を増やしても、この記録上限は増えない。download の保存済み chunk は記録として保持するため、転送全体に比例する履歴費用がある。記録の spill / prefix 解放は v1.9 の計画で扱う。

write の成功はローカルキューへの受付であり、相手の処理完了を保証しない。送信結果は Unknown として扱う。finish の応答も、その API の意味に応じて確認する。close / cancel / revert で既に送った bytes を取り消さない。

## 記録と実接続の寿命

open / 各 read / 各 write / finish / 明示 close は別の記録操作。操作位置、接続 ID、chunk limit または byte fingerprint を照合する。同じ操作は保存済み結果または同じ pending operation を使い、元の request を再送しない。

checkpoint は token と所有情報を保存する。physical driver、closed 状態、ID / lease 発行位置は巻き戻さない。閉じた接続を含む checkpoint に revert しても、保存済み chunk は再利用できる。新たな read / write には HttpDownloadClosed / HttpUploadClosed を返す。checkpoint だけの参照は実接続を保持しない。restore で失われる pending Task はキャンセルする。Runtime 終了では driver をキャンセルし、deadline まで放置しない。

UTF-8 以外の Bytes を含む登録済み秘密値は incremental matcher で検査し、秘密値の途中の prefix を公開しないよう末尾を保留する。複数の transport chunk に分かれた秘密値も HttpSecretResponse。matcher 入力の合計は 64 KiB。接続開始後に秘密値集合を変えると、次の新規 read は HttpSecretContextChanged として閉じる。後から登録した秘密についても trace export 時に保存済み body / header / 連続 chunk を検査し、秘密を含む trace を拒否する。

## 検証

実サーバーで TLS、binary chunk、EOF、queued upload、finish、実接続の解放、キャンセル、secret split、後からの secret 登録、revert 後の記録再利用を検査する。標準 module 経由の所有値の return / move / 借用と、コンパイル済み artifact の実行・server 停止後 replay も検査する。両 OS の配布工程は展開した SDK の逐次 download / upload と source-free replay を実行する。
