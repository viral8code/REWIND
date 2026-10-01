# 外部操作の記録と再利用

```sh
rewind run main.rw --allow-effects external,clock --record trace.json
rewind replay trace.json --root . --allow-effects external,clock
rewind compile main.rw --allow-effects external,clock
rewind run main.rwc --allow-effects external,clock
```

最初の実行では UTC の milliseconds を観測し、二行に同じ値を出力する。revert 後は同じ操作位置の記録を使い、新たな時刻を観測しない。`external fresh` なら新規操作になる。再生では clock に触れず記録済みの結果を使う。

v1.5 の region は main task の active branch 外で使用する。領域内の checkpoint / publish、領域から抜ける return / `?` / break / continue、task switching は禁止される。外部操作の終了は未公開 output の確定ではなく、別途 publish が必要。
