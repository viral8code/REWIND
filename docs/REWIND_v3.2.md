# REWIND 3.2.0

言語構文は3.0.0 / 3.1.0と同じです。3.2.0はライブラリとテストの追加版です。

## 追加

- **`guiTable.removeRow(table,id)`**：caller-supplied row IDのtableから行を削除します。その行の編集overlayと開いているdraftを落とし、選択はIDで追従し、sort順とfilterは保持します。費用はO(rows×columns)のfrozen読取りで、小さいtable向けです。
- **PostgreSQLの実backend確認**：`std.dbTransaction`のcommit・rollback・古い完了通知の拒否と、`std.dbMigration`の適用・再実行・編集検出・失敗時のDDL rollbackをLinux / WindowsのCIで確認しました（[tests/db_postgres_flows.rs](../tests/db_postgres_flows.rs)）。
- **SQLite**：同一process内で同じfileへの2本目の接続が拒否されることをテストで固定しました。

## 未完了

- cancel・接続断・commit結果不明の実PostgreSQLでの再現。
- GUI上の行削除操作（キー等）と、GUI入力からのevent連携の仕上げ。

詳細は[v2.2計画](REWIND_v2.2-plan.md)を参照。
