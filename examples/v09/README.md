# v0.9 examples

```sh
rewind update --root examples/v09
rewind run --root examples/v09
rewind doctest examples/v09/manual.md --root examples/v09
rewind api-snapshot --root examples/v09 --output /tmp/v09-api.json
rewind repl --root examples/v09 --record /tmp/v09-session.json < examples/v09/repl.txt
rewind session-replay /tmp/v09-session.json --root examples/v09
rewind install --production --root examples/v09 --output /tmp/v09-production
rewind run --root /tmp/v09-production
```

record/install の出力は存在しない path を指定する。通常実行は 4 を出力する。REPL は assertion に失敗した入力だけを取り消し、n=4 を維持する。editor-timeline.json は LSP request の例で、uri を trace/inspection の絶対 path に置き換え、通常の Content-Length frame で送る。session transcript と通常の indexed trace は異なる形式。
