# SQLite の実行例

```sh
rewind run main.rw --allow-effects external,db,tasks
```

独立したmemory DBにtableを作り、値を束縛してINSERTし、cursorから最大64行のbounded batchを取得する。db.validateRowはcolumn名とDbValueをJSON objectに対応づけ、同じstd.schemaで必須field・範囲・unknown fieldを検証してからJsonを返す。scoreFromJsonは検証済みJSONから明示的にapplication recordを作る。出力は1、Alice、42。

DbValue::NullはJSON nullへ写すため、nullable schema fieldでだけ受け入れられる。列名の重複と行shape不一致はmapping errorになる。BytesとPrivate cellは暗黙にTextへ変換せず拒否する。DecimalのSQLite BLOBにはdbDecimal.cellを使い、必要なmapperでDecimal textをJSON Textへ明示的に渡す。PostgreSQL NUMERICがTextで返る場合はexact Decimal schema ruleを直接検証できる。failure diagnosticにはcell valueを含めない。

:memory:をroot内のfile名に変更すると、実fileに保存する。再実行時もDB fileは残るため、この例のCREATE TABLEは既存tableに対して失敗する。VMのrevert/publishはDBの保存を取り消さない。

APIと制約は[DB仕様](../../docs/REWIND_v1.7.md)を参照する。
