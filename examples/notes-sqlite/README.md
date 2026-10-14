# SQLite のメモ帳例（コンソール版）

タイトルの入力検証、memory SQLite への保存、一覧、失敗した保存の rollback を、コンソール出力だけで示す最小の例です。`std.dbMigration` で作った migration を `std.dbTransaction` の transaction 内で適用し、保存は 2 件を 1 transaction で commit します。

```sh
rewind run main.rw --allow-effects external,db,tasks
```

期待出力（この 6 行で全部です）:

```text
title required
saved 2
1 first
2 second
rolled back
count 2
```

各行の意味:

1. 空タイトルは保存されず、DB にも触れずに `title required` と出ます。
2. `first` と `second` を 1 transaction で INSERT して commit できたら `saved 2` です。
3. `SELECT` の結果を列名で `Note{id,title}` へ写し、`1 first` と `2 second` を出します。
4. 2 件目の INSERT に不正な SQL を渡し、transaction を rollback します。`rolled back` の後、件数はまだ 2 件のままです（`count 2`）。

## VM revert と publish について

この例の DB 操作は VM の状態ではありません。VM の revert や `publish;` は SQLite の保存を取り消しません。commit 済みの 2 件は、VM がどこまで巻き戻っても残ります。取り消せるのは、VM の変数や画面の状態だけです。network、DB、native 入力を VM revert で戻せるように説明しないでください。

## 結果が不明なとき

commit や rollback の応答が届かなかった場合、`DbError.phase` が `Unknown` になります。この状態では自動で再試行せず、rollback も行いません。画面には「保存の結果は不明です」と表示し、利用者が DB の件数などで確認してから、次の操作を決めます。`NotSent` は送信前の失敗で、元の状態に戻ります。それ以外は `Failed` として rollback できます。

この例は、どの DB エラーでも `take` でパニックするだけです。結果不明の表示や再確認の流れは実装していません。実アプリでは、`Unknown` を失敗と同じ扱いにせず、この区別を必ず分けてください。

## 関連

- 手本 transaction: [tests/fixtures/db_transaction.rw](../../tests/fixtures/db_transaction.rw)
- migration の plan と適用: [tests/fixtures/db_migration.rw](../../tests/fixtures/db_migration.rw)
- 行の写像: [tests/fixtures/db_row_mapping.rw](../../tests/fixtures/db_row_mapping.rw)
- 検証テスト: [tests/db_row_mapping.rs](../../tests/db_row_mapping.rs)（`sqlite_notes_example_saves_lists_and_rolls_back_a_failed_save`）
