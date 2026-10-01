# 最短距離を求めるCLI

compiler/language 0.9.3。最初の行はvertex数とedge数、後続は `from to weight`。vertexは0始まり、edgeは有向、sourceは0。重みは非負Int。各vertexの最短距離を出力し、到達しないvertexは `unreachable` と表示する。解析/範囲/容量/負辺/距離overflowはstatus2。

```sh
rewind sdk-install --root examples/v093 --sdk /path/to/sdk-0.9.3 --public-key PUBLIC_KEY_HEX
printf '4 3\n0 1 4\n0 2 1\n2 1 1\n' | rewind run --root examples/v093 --task-steps 2000000
rewind build --root examples/v093 --output /tmp/shortest.json
```

出力は `0` / `2` / `1` / `unreachable` の4行。input observationはappで明示し、scannerとgraph libraryはpure。各行は1 MiBまで。規模に応じてexecutionSteps、--task-steps、historyMemoryを指定する。固定の時間/メモリ制限で任意の規模が処理できることは保証しない。
