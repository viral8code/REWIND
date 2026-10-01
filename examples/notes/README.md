# Native notes

`main.rw` を作業用フォルダへコピーし、次を実行します。

```sh
rewind run main.rw --allow-effects gui,fileRead,fileWrite
```

文字入力、Enter、選択（Shift + 矢印 / Ctrl+A）、Backspace / Delete、サイズ変更、Save、Undo all を利用できます。保存先は作業フォルダの `notes.txt`。Undo all は表示モデルを起動時へ戻しますが、保存済みファイルは戻しません。IME の変換は OS で行い、確定した文字を記録します。

fixture は `日本語😀` を入力して絵文字を消し、改行と `notes` を加えて resize し、保存して終了します。

```sh
rewind run main.rw --allow-effects gui,fileRead,fileWrite --gui-events events.json --record trace.json
rewind replay trace.json --root . --allow-effects gui,fileRead,fileWrite
```

Replay は実際の画面や保存ファイルを変更しません。保存済みファイルが存在する場合も、起動時の読取りには記録された観測を利用します。
