# PostgreSQL

SDK のこのディレクトリを作業 root にコピーする。サーバーの CA certificate を DER 形式の `ca.der` として root に置き、接続文字列を `REWIND_PG_DSN` に設定する。公開 CA の PEM を変換するには `openssl x509 -in ca.pem -outform DER -out ca.der` を使える。

接続文字列の例は `host=db.example.com port=5432 user=app password=... dbname=app sslmode=require`。`require` は暗号化に加えて証明書と host 名を検証し、平文へ fallback しない。DSN をソース・artifact へ埋め込まず、`Env.getSecret` から取得する。

```sh
rewind run main.rw --allow-effects external,db,tasks,env,fileRead,output --secret-env REWIND_PG_DSN
rewind compile main.rw --allow-effects external,db,tasks,env,fileRead,output
rewind run main.rwc --allow-effects external,db,tasks,env,fileRead,output --secret-env REWIND_PG_DSN
```

出力は `true`、`42`、`Alice`。この例は接続・パラメータ束縛・逐次行読取り・scope cleanup を行う。

`--record trace.json` で記録できる。再生には同じ環境名を `--secret-env REWIND_PG_DSN` で外部から再供給する。秘密を欠落させた replay は保証しない。接続先が停止していても、CA file と source が無くても、compiled artifact と記録済みの観測から再生できる。DB には接続しない。
