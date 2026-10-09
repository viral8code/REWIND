# 協調的なforward計算とnative GUI入力

非ゼロの共有viewをmaterializeし、所有するTapeでparameter・sigmoid・square・meanを分割実行します。ウィンドウのクリックで進行中の計算を取り消します。途中のTapeは公開しません。

```sh
rewind run main.rw --allow-effects gui --native-work 10000000000 --history-memory 256MiB
```

GUI入力・取消のsource-free replayはdisplayを必要としません。初期入力のmaterializeは同期APIです。これは初期入力も含む全処理の応答性の完了宣言ではありません。
