# HTTP の逐次受信

```sh
rewind run main.rw --allow-effects external,network,tasks -- https://example.com/
rewind compile main.rw --allow-effects external,network,tasks
rewind run main.rwc --allow-effects external,network,tasks --record trace.json -- https://example.com/
rewind replay trace.json --allow-effects external,network,tasks
```

status と受信した総 byte 数を出力する。UTF-8 変換や body 全体の結合は行わない。接続は match の所有スコープを抜ける際に閉じる。`take` はこのサンプルの失敗を panic に変える helper。通常のアプリケーションでは HttpError の code / phase / status を match して扱う。

このサンプルは累積 1 MiB、10 秒を上限にする。設定を増やす場合は native work と記録容量も確認する。[仕様](../../docs/REWIND_v1.6.1.md)を参照。
