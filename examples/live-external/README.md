# 明示的な live 外部操作

SQLite の例。`rewind run main.rw --allow-effects external,db,tasks,live --native-work 5000000` で実行する。`rewind compile main.rw --allow-effects external,db,tasks,live` の後は `.rwc` のみでも実行できる。

`external live` は呼出しのたびに新規操作を送信する。checkpoint が保持する既存 Task の復元では結果を再利用する。この例では同じ INSERT Task を二度 await しても一行だけ増え、別途24回作成した Task と合わせて25行になる。外部 DB の内容は VM revert では戻らない。

完全な record / replay / inspect に対応しない。通常の `external` は引き続き記録付きで、同じ論理位置・要求では新規送信しない。配布文書の `REWIND_v1.9.21.md` を参照する。
