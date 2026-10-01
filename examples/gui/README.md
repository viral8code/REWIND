# Native counter with Undo

```sh
rewind run main.rw --allow-effects gui
```

Windows と Linux/X11 にウィンドウを開きます。Add、Undo all、Enable counting、Tab/Enter/Space、ウィンドウ終了を試せます。Undo はデータを巻き戻した後 `continueInput()` で処理済み入力を読み飛ばし、復元した scene を publish します。

表示環境のないテストでは次を使い、`1` が出力されることを確認できます。

```sh
rewind run main.rw --allow-effects gui --gui-events events.json --record trace.json
rewind replay trace.json --root . --allow-effects gui
```

詳しい API と制限は [GUI guide](../../docs/gui.md) を参照してください。
