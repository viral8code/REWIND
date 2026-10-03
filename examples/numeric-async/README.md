# 協調的な数値処理

```sh
rewind run main.rw --native-work 100000000
```

native 内積の chunk と別の heartbeat task を進め、行列積を同期版と比較する。publish 後の revert でモデルを戻し、再計算した行列の値をもう一度出力する。出力は `9000`、`32`、`32`。

```sh
rewind compile main.rw
rewind run main.rwc --native-work 100000000 --record trace.json --record-mode compact
rewind replay trace.json
```

compile 後は source を削除しても実行・replay できる。replay は記録した計算予算を復元する。既定の debug 記録も使える。

`jobs.dot` / `jobs.matmul` は Task<Result<T,StdError>> を作る。`await` の外側 Result は TaskError、内側は数値の結果。入力 buffer は共有され、task が使う version は後の更新から独立する。chunk ごとに task を切り替え、キャンセルに応答する。`std.task.yieldNow()` は main の GUI polling loop にも使える。[契約](../../docs/REWIND_v1.9.10.md)を参照。
