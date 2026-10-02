# REWIND v1.7.1 — PostgreSQL

v1.7.0 の SQLite、scope-owned 接続、DB transaction、外部観測記録に PostgreSQL adapter を追加する。SQL / transaction は VM の commit / revert / publish と独立し、確定済みの DB 書込みを VM revert で取消さない。

## API と認証

`std.db.credentials(alias,Secret<String>,caCertificate:Bytes)->Result<Unit,DbError>` は checkpoint 外に immutable な接続情報を登録する。alias は英数字、`_`、`-` の1..128 byte。最大16件、接続文字列と DER CA は各64 KiB、登録全体の保守的な保持予算は2 MiBである。異なる情報による alias の置換は `DbCredentialImmutable`。

`std.db.postgres(alias,timeoutMillis)->Task<Result<DbConnection,DbError>>` を `external` 内で呼び、領域の外で await する。接続文字列をソースへ埋め込まず、`Env.getSecret` と `--secret-env NAME` を使う。[実行例](../examples/postgres/README.md)は `true`、`42`、`Alice` を返す。

TCP host は1つ、port は最大1つ。Unix socket、hostaddr による host 名との分離、接続 options、複数 host の failover は提供しない。DNS で選んだ1 address に接続を1回試みる。自動 reconnect / write retry はない。独立した cancel 制御接続を使うことがある。

`sslmode=require` は TLS の暗号化、信頼する CA、host 名をすべて検証する。`caCertificate` は単一 DER root certificate、空 Bytes は OS の信頼ストアを使う。`prefer` の平文 fallback と証明書検証の無効化は認めない。明示的な `sslmode=disable` は loopback のみ許可し、解決した address も loopback であることを確認する。SCRAM-SHA-256 を実 server で検証する。channel binding の `require` は現 adapter で拒否する。

## SQL・型・cursor

既存の `prepare`、`executeStatement`、`queryStatement`、`executeMany`、`query`、`next` を使い、placeholder は `$1` 等。SQL への値の文字列連結は不要である。compiled statement cache は接続ごとに16件。

| PostgreSQL 型 | DbValue |
| --- | --- |
| boolean | Bool |
| smallint / integer / bigint | Int。parameter の範囲も検証 |
| real / double precision | Float。非有限 parameter は拒否 |
| text / varchar / char / name | Text |
| bytea | Bytes |
| SQL NULL | Null |

未提供の型は `DbType` とし、文字列や数値への暗黙の損失変換をしない。numeric、date、array、JSON 等を取得したい場合は SQL 側で対応する基本型へ明示的に cast する。専用型の変換は後続の数値・日時工程で追加する。

query はサーバー側の `NO SCROLL` cursor を使い、再クエリや LIMIT / OFFSET の繰返しで代用しない。next は1..64 row、1..4 MiBの wire byte 予算。サーバーからは最大8 rowずつ FETCH し、余りも bounded な buffer に保持する。大きな結果全体を List にしてから返すことはしない。cursor の column 名は順序と重複を維持する。

DB transaction の外で query した場合は adapter が内部の read-only transaction を作り、EOF / closeCursor で終了する。これは VM transaction ではない。明示的な DB transaction 内の query は、その transaction を使う。active cursor 中の別の接続操作は `DbBusy`。next の期限は元の query の期限も越えられない。

SQL の BEGIN / START / COMMIT / ROLLBACK / SAVEPOINT / RELEASE / PREPARE 等を直接渡して寿命管理を迂回することは拒否する。DB transaction は `beginTransaction` / `commitTransaction` / `rollbackTransaction` を使う。transaction が失敗状態になった際も物理世代を変え、以前の成功を未確定の成功として再利用しない。rollback / close が必要なことは Result から判断できる。

## 失敗・記録・解放

SQLSTATE は `DbError.sqlState:Option<String>`、SQLite の extended integer code は引続き sqlCode。PostgreSQL の生メッセージ、SQL、parameter、接続先を DB error に含めない。認証拒否は `DbAuthentication` と SQLSTATE、TLS 拒否は `DbTls`。切断・期限・cancel の結果は外部での適用範囲が不明になり得る。

期限 / cancel は制御接続で query の中断を試み、親接続も閉じる。close / scope cleanup は rollback を試み、`waitForCleanup` は完了と失敗を await できる。制御接続と rollback の待ちは各1秒に制限する。完了確認が得られない cleanup は `DbCleanup` として保持し、成功を捏造しない。

要求の fingerprint は opaque alias と公開入力を照合し、接続文字列・password・秘密 parameter の値や hash を含めない。既知の秘密を返す row / column は記録せず `DbSecretResult`。Bool、binary、数値も検査する。既知の短い scalar 秘密は公開 SQL 等にも一致し得るため、保守的に拒否される場合がある。この仕組みは任意の秘密変換を追跡する DB 全体の taint inference ではない。

replay は記録した結果だけを返し、DB 接続を開かない。秘密環境値は同じ名前で外部から再供給する。source と CA file を削除し、接続先を未使用 port に変えた compiled artifact の replay を検証する。

## 容量・検証

接続と解放待ちは SQLite / PostgreSQL を合わせて最大8、同時 native job は最大8。SQL64 KiB、256 column、1024 parameter、parameter byte1 MiB、executeMany1024 row / 合計1 MiBは共通。PostgreSQL の row は1 MiB、受信 frame は約1 MiBを越えると protocol decoder の大きな payload 確保前に拒否する。

接続は専用 actor と bounded command queue で直列化する。PostgreSQL 接続の native 保持費は192 MiBで保守的に計上し、終了待ちも予算から消さない。これは実使用量を常に192 MiBとするという意味ではない。データの取得は bounded で、測定に基づく予算・コピー費用の改善は1.9で行う。

実 PostgreSQL / SCRAM / TLS で、parameter の型・NULL・binary、statement 再利用、cursor、SQLSTATE、transaction と VM revert、部分 bulk failure、認証失敗、TLS 信頼拒否、期限、scope cleanup、source-free / disconnected replay を検証する。公開の条件は Linux / Windows の回帰と展開済み SDK の実 server 検証の成功である。
