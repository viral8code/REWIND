# SQLite の実行例

```sh
rewind run main.rw --allow-effects external,db,tasks
```

独立した memory DB に table を作り、値を束縛して INSERT し、cursor で取得する。`cursor.columns`を`db.indexColumns`で一度検査し、明示mapper `mapScore`を`db.mapBatch`へ渡して、取得済みの最大64行を型付き`Score`へ変換する。失敗は0起点のbatch内row indexとmapper errorを返し、cursorを進めず元のDbBatchも保持する。出力は `1`、`Alice`、`42`。scope の外で cleanup を await してから終了する。

`:memory:` を root 内の file 名に変更すると、実 file に保存する。再実行時も DB file は残るため、この例の CREATE TABLE は既存 table に対して失敗する。VM の revert / publish は DB の保存を取り消さない。

API と制約は [DB 仕様](../../docs/REWIND_v1.7.md) を参照する。
