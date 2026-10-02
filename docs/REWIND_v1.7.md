# REWIND v1.7 — DB

v1.7.0 は SQLite adapter、`std.db`、パラメータ束縛、prepared statement、逐次 cursor、DB transaction を提供する。PostgreSQL の実接続・TLS・認証は [v1.7.1](REWIND_v1.7.1.md) で追加する。SQLite だけで v1.7 の DB 計画全体を完了としない。

## 実行

```sh
rewind run main.rw --allow-effects external,db,tasks
rewind compile main.rw --allow-effects external,db,tasks
rewind run main.rwc --allow-effects external,db,tasks
```

[実行例](../examples/database/main.rw)は `1`、`Alice`、`42` を出力する。`db.sqlite(":memory:",false,5000)` は独立したメモリ DB、通常の path は実行 root 内の DB file。SQL は publish を待たずに実行される外部作用である。

呼出しは `external { ... }` 内で行い、返された hot `Task<Result<T,DbError>>` を領域の外で await する。await 自体は `Result<...,TaskError>` を返すため、Task の失敗と DB の失敗を別々に扱う。

## 型と SQL

- `DbConnection {backend:String}`、`DbStatement {columns:Frozen<List<String>>,parameters:Int}`、`DbCursor {columns:Frozen<List<String>>}` は移動できる scope-owned な資源。共有・freeze はできず、戻した VM token も閉じた物理接続を復活させない。
- `DbValue` は `Null`、`Bool(Bool)`、`Int(Int)`、`Float(Float)`、`Text(String)`、`Bytes(Bytes)`、`Private(String)`。SQLite の Bool parameter は integer 0/1 として束縛し、観測した integer は Int として返す。`boolean` getter は 0/1 だけを明示的に変換する。非有限の Float parameter は拒否する。
- `DbRow {values:Frozen<List<DbValue>>}`、`DbBatch {rows:Frozen<List<DbRow>>,done:Bool}`。column 名は cursor の immutable metadata に保持し、重複名を上書きしない。
- `DbError {code:String,phase:String,sqlCode:Option<Int>,sqlState:Option<String>}`。SQLite の extended code は sqlCode、sqlState は None。SQL・parameter・接続先を含む backend の生エラーメッセージは返さない。

SQLite placeholder は `?1`、`?2` 等を使い、SQL と `Frozen<List<DbValue>>` を別に渡す。`execute` は affected rows、`query` は cursor、`next` は batch を返す。`prepare` は SQL を接続に結び付け、`executeStatement` / `queryStatement` で再利用する。接続ごとの compiled statement cache は16件である。

`executeMany` は SQL を一度 prepare し、parameter row を順に束縛する。自動で transaction を作らない。途中で失敗した場合は前の row が適用済みかもしれないため、必要な原子性は明示的な DB transaction で確保する。

`cell`、`integer`、`floating`、`text`、`bytes`、`boolean` は column index を検証して Result を返す。`column` / `cellByName` は名前を完全一致で照合し、重複を `DbDuplicateColumn`、不在を `DbColumnMissing` とする。NULL は `DbValue::Null` であり、型付き getter で値を捏造しない。

## transaction と巻き戻し

`beginTransaction`、`commitTransaction`、`rollbackTransaction` は DB の操作で、VM の commit / revert / publish とは独立する。VM revert は確定済み DB 書込みを戻さない。

同じ task・操作位置・要求の再実行は記録を再利用し、INSERT 等を再送しない。異なる要求は `ExternalRequestMismatch`。新しい操作には `external fresh` を使う。DB transaction 内の成功には物理 transaction の世代を記録し、終了した transaction の成功を再利用しようとすると `DbTransactionExpired` の実行時診断にする。DB が無い offline replay は、記録済みの計算を再生する。

SQLite は接続ごとの直列利用で、cursor が active な間の他の操作は `DbBusy`。`closeCursor` で statement を解放して接続を再利用できる。EOF の batch は done=true で、その cursor に新しい next を送ると `DbClosed`。prepared statement は transaction を越えて再利用できるが、親接続の close で無効になる。

close・キャンセル・scope 終了は未確定 transaction の rollback を試みる。close は rollback と接続解放の結果を await できる。scope cleanup は VM を止めずに開始し、`waitForCleanup` で解放の完了と rollback / close の失敗を確認できる。解放待ちの worker と file lock は完了まで保持・計上する。キャンセルは接続を閉じ、既に適用された作用の取消しは保証しない。

## 上限と失敗

接続は最大8、同時 native job は最大8。SQL は64 KiB、column は256、parameter は1024、parameter byte は1 MiB。executeMany は最大1024 parameter row、合計1 MiB。SQLite value / row 上限は1 MiBである。

next は1..64 row、maxBytes は1..4 MiB。byte 計上には JSON の escape / base64 と値の管理費を含み、記録可能な batch だけを返す。全結果を常に List 化せず、statement と Rows は接続専用 worker 上に保持する。

期限は1..120000 ms。query の期限は後の next にも適用し、SQLite progress callback、cancel flag、interrupt を併用する。SQLite の busy wait は50 msで、ユーザー SQL から延長できない。scope cleanup は期限を過ぎた query guard を解除して rollback を試みる。

SQLite の native heap は process 全体で64 MiBの hard limit を設定する。thread stack、compiled SQL、parameter、結果、解放待ち資源も VM の予算で保守的に計上する。大きな checkpoint の保持費と、外部資源の保持費は別である。

`NotSent` は host へ操作を渡す前の拒否、`Unknown` は適用範囲を確定できない失敗、`ResponseReceived` は取得した結果の拒否。自動 reconnect / write retry はしない。外部観測台帳の容量不足や記録失敗は実行時診断となり、失敗した操作を自動再送しない。

## path・秘密値

DB 本体と -wal / -shm / -journal は root 制約で検証する。未 publish の仮想 file がある path は open を拒否し、使用中の DB / sidecar に仮想 file I/O を重ねない。ATTACH / DETACH、拡張ロード、接続設定を変更する PRAGMA は禁止する。

`privateParameter(alias,secret(DbValue::Text(...)))` 等で登録した値は immutable な公開 alias から束縛する。要求 fingerprint は alias のみを扱い、秘密の値や値の hash を入れない。文字列、binary、数値の既知の秘密を公開 parameter に混ぜることも拒否する。

既知の秘密を返す row / column は `DbSecretResult` とし、base64 の binary も検査する。後から秘密を登録した場合は trace export 時にも既存の row を検査する。alias は64件、登録値の総量は64 KiB、秘密パターンの総量も64 KiBである。この検査は DB 内の全データに秘密属性を自動付与する機能ではない。

## 検証

実 SQLite の memory / file DB で parameter、NULL / binary、statement 再利用、逐次 row、rollback、部分失敗、期限、closed token、file 競合を検証する。compiled artifact から source と DB file を削除した後の replay も検証する。PostgreSQL の検証は v1.7.1 の実 server で追加する。
