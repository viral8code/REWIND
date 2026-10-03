# 数値 storage の共有と予算

`rewind compile main.rw` の後、次のように実行します。

```sh
rewind run main.rwc --history-memory 64MiB --native-work 100000000 --steps 10000000 --task-steps 10000000 --record trace.json --record-mode compact
rewind replay trace.json
```

出力は `1000000`、`64`、`0`。100万要素を task の引数で共有し、変更した値を64個の checkpoint に保存してから最初の値へ戻します。共有する page を重複計上せず、書換えた page / path の分は保持します。最後に全 checkpoint を捨てます。数値の user owner が残れば storage 自体は保持されます。実行後にソースを消しても artifact と replay で動きます。

このサイズの記録例は compact を選びます。debug 記録は値を含む検査 index の費用が別にかかります。history の上限は RSS の計測値ではありません。
