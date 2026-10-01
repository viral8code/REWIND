# SDK 標準ライブラリを使う CLI

Linux x86_64、compiler/language 0.9.2。最初の一行を ASCII whitespace で分割し、signed64 の整数を読み、合計を出力する。入力欠落・解析失敗は status2、成功は0。加算 overflow と実行予算の超過は処理系の失敗となる。

```sh
rewind keygen /tmp/rewind-sdk.seed
# 表示された公開鍵を PUBLIC_KEY_HEX として使う。seed は配布しない。
rewind sdk-build --output /tmp/rewind-sdk-0.9.2 --key /tmp/rewind-sdk.seed
rewind sdk-install --root examples/v092 --sdk /tmp/rewind-sdk-0.9.2 --public-key PUBLIC_KEY_HEX
printf '2 3 -4\n' | rewind run --root examples/v092
rewind build --root examples/v092 --output /tmp/sum.json
printf '5 7\n' | rewind run-artifact /tmp/sum.json --root examples/v092 --allow-effects input,output
```

artifact に必要な std の code が入るため、最後の実行に source は不要。record/replay は `run --record TRACE` / `replay TRACE --root PROJECT` を使う。SDK 内 `share/rewind/examples/sum` を通常の writable project directory にコピーしてから導入してもよい。
