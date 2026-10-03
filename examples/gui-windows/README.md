# 複数ウィンドウと Undo

このディレクトリを writable な場所へコピーして実行する。

```sh
rewind run main.rw --allow-effects gui
```

Text と Counter の二つのネイティブ画面を表示する。Add で数値を増やし、Undo で初期モデルへ戻す。いずれかの画面を閉じると、両方の終了を publish する。Linux は X11 / core fonts、Windows は Win32 を使う。

画面への変更は `windows.present(id,&view); publish;` で確定する。`revert modelStart; windows.continueInput();` はモデルを戻し、すでに受信した入力を読み飛ばす。公開済み画面は revert だけでは変わらず、復元したモデルを再度 present / publish する。

```sh
rewind compile main.rw --allow-effects gui
rewind run main.rwc --allow-effects gui --gui-window-events events.json --record trace.json --record-mode compact
rewind replay trace.json --allow-effects gui
```

fixture では Add、Undo、Close を実行し、出力は `1` と `0`。fixture と replay では OS の画面を開かない。コンパイル後は source と入力 fixture を削除しても replay できる。既定の debug 記録も利用可能。

`nextEventAny()` は入力待ちの間 VM task を進めない。通信等の task と組み合わせる event loop では `pollAny()` と task.isDone() を利用する。明示的な VM task handoff API は後続版で追加する。[GUI guide](../../docs/gui.md)、[版の契約](../../docs/REWIND_v1.9.9.md)を参照。
