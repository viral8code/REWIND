# live GUI 入力と Task の保持

この例は入力 fixture で寿命・復元・キャンセルを確認するもの。実ウィンドウで HTTP / SQLite と組み合わせる例は `gui-async` を参照する。

```sh
rewind compile main.rw --allow-effects gui,tasks,external,live
rewind run main.rwc --allow-effects gui,tasks,external,live --gui-window-events events.json
```

同梱の `events.json` を使用する。一度取得した入力 Task の checkpoint を復元しても同じ日本語イベントが返り、新しい待機 Task は次の入力または fixture 終端を取得する。キャンセル済みの待機は次のイベントを消費しない。

完全な record / replay / inspect は利用できない。履歴を記録する用途は `nextEventAnyAsync` を使用する。GUI 待機の作成は application Task の `external live` 内で行い、await は外で行う。Task と checkpoint の最後の保持先がなくなれば観測結果を解放する。公開済みウィンドウ自体は待機 Task の解放では閉じない。
