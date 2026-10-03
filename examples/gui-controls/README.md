# フォームと表の二つの window

```sh
rewind run main.rw --allow-effects gui
rewind compile main.rw --allow-effects gui
rewind run main.rwc --allow-effects gui
```

Name / Age / Active を入力して Submit を押すと型付き Json を表示する。Age は0..150。表は100行のうち4行だけを描画し、クリック・矢印・wheel で選択 / 移動できる。Escape で両モデルを最初の checkpoint に戻す。公開済み画面へ自動的に巻き戻しをかけず、復元したモデルを再描画して publish する。どちらかの window を閉じると両方を閉じる。

再現可能な入力例:

```sh
rewind run main.rwc --allow-effects gui --gui-window-events events.json --record trace.json --record-mode compact
rewind replay trace.json --allow-effects gui
```

出力は `{"active":false,"age":13,"name":"日本語"}`、`1`、`日本`。replay は元の .rw や入力 fixture、native GUI を必要としない。実 window を使うには Linux で X11 / DISPLAY、Windows で対話 desktop が必要。

この例のフォームと表は固定寸法。Resize への応答は各 dispatch が返す Resize を使い、表示行数と寸法をアプリケーション側で決める。
