# 協調的な自動微分forward

`std.autodiffForwardAsync`はTapeを所有して計算し、成功時に`Tuple<Tape,Node>`を返します。数値ページは共有します。戻り値のTapeを先に`move result._0`で取り出し、Nodeは`result._1`から得ます。元のmoved bindingへTapeを再代入する書き方は使いません。

```sh
rewind run main.rw --native-work 100000000 --task-steps 1000000 --steps 10000000
```

この例は実行途中のcheckpoint、復元、取消を確認します。同期APIとNodeのidentityが一致し、debug・compact両形式でsource-free replayできます。保持したcheckpointには旧Tapeのページが残ります。
