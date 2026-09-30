# v0.9.1 offline application

JSON 設定 → schema 検査 → immutable State → 単一 state.json の更新。既存 state があれば読み込んで増分を保存する。引数 `--count VALUE`、許可した `REWIND_COUNT`、保存 state / 初期 config、fallback の順に優先する。環境/引数の VALUE は JSON の整数として解析する。失敗した入力は status 2、ファイル操作の業務失敗は 3。publish の競合/部分失敗は compiler の 72/73 で区別する。

```sh
rewind update --root examples/v091
rewind run --root examples/v091 --allow-env REWIND_COUNT -- --count 4
rewind run --root examples/v091 --allow-env REWIND_COUNT
rewind run --root examples/v091 --allow-env REWIND_COUNT --record /tmp/app-trace.json -- --count 6
rewind replay /tmp/app-trace.json --root examples/v091 --allow-env REWIND_COUNT
rewind run --root examples/v091 --diagnostic-format json --allow-env REWIND_COUNT -- --unknown value
rewind install --production --root examples/v091 --output /tmp/new-app-release
rewind run --root /tmp/new-app-release --allow-env REWIND_COUNT
rewind build --root /tmp/new-app-release --output /tmp/app-artifact.json
```

初回の出力は 5、次の出力は 6。replay は journal の観測を使い、Host の state.json を更新しない。既存 state の読取りを通じて publish 前の競合を検査する。チェックから置換までを保護する process 間 lock、power-loss の保証、複数ファイル transaction は未実装であり、実運用では一 writer を前提にする。

`[assets] config = "assets/config.json"` だけを immutable asset として lock に固定し、production install に含める。state.json は mutable data なので asset に指定しない。release は新しい directory を要求し、署名 seed・記録・cache はコピーしない。

```sh
rewind keygen /tmp/app-release.seed
# keygen の出力 PUBLIC_KEY を使用する
rewind sign-release /tmp/new-app-release/rewind.release.json --key /tmp/app-release.seed
rewind verify-release /tmp/new-app-release/rewind.release.json --public-key PUBLIC_KEY
rewind sign-artifact /tmp/app-artifact.json --key /tmp/app-release.seed
rewind run-artifact /tmp/app-artifact.json --root /tmp/new-app-release --verify-key PUBLIC_KEY --allow-effects args,env,fileRead,fileWrite,output --allow-env REWIND_COUNT
```

release の署名検査は asset・source・lock の内容も検査する。通常の run は release の署名を自動検査しないため、配布先では信頼した public key を使って verify-release してから実行する。artifact の署名は別 domain で、source-free 実行にも同じ asset の配置が必要。
