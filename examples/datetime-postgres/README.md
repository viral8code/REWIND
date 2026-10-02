# 日時と PostgreSQL

`rewind run main.rw --allow-effects external,db,tasks,env,fileRead --secret-env REWIND_PG_DSN`。PostgreSQL の DSN を REWIND_PG_DSN として指定し、TLS CA を ca.der に配置してください。timestamptz のマイクロ秒精度を超える値は拒否します。DB への書込みは VM の revert 対象外です。
