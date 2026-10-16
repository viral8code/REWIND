# REWIND 3.3.0

言語構文は3.0.0 / 3.1.0 / 3.2.0と同じです。

## 追加・修正

- **`guiTable.deleteRequest(table,event)`**：Delete keyが押され、行が選択されていて編集draftが開いていないときだけ、選択行の安定IDを返します。削除は行わず、アプリが受け入れたら`removeRow`を呼びます。
- **`std.dbTransaction`の修正**：接続が閉じられた（deadline・取消・切断）失敗`DbClosed`は、transactionを`Active`のまま残さず`Failed`にします。サーバー側でtransactionは終了済みで、commitを受け付けません。
- **実PostgreSQLでの失敗系の確認**（[tests/db_postgres_flows.rs](../tests/db_postgres_flows.rs)）：業務失敗のrollback、deadlineによる取消、commit前の接続断、COMMITを送った後に応答が来ない場合（`Unknown`、rollbackも再試行も不可、接続は再利用不可）。いずれも新しい接続から未commitの行が見えないことを確認します。

## 未完了

- tableの`EditRejected`経路のテスト（編集overlayが4 MiBを超える大きさのtableが必要で、既定の実行予算に収まらない）。

詳細は[v2.2計画](REWIND_v2.2-plan.md)を参照。
