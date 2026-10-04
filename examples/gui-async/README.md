# GUI input while HTTP waits

```sh
rewind run main.rw --allow-effects gui,external,network,db -- http://127.0.0.1:8080/data "REWIND async"
```

HTTP の接続先は別途用意する。`state.sqlite` がない directory で実行する。日本語の値を parameter で保存し、VM の model を revert した後も DB の値が存在することを確認する。loading 画面を表示した後は HTTP と `nextEventAnyAsync` の完了を待ち合わせ、Cancel ボタンまたは閉鎖操作で HTTP Task を cancel する。HTTP が先に完了した場合は status または failure を表示する。最後に画面と scope-owned DB 資源を閉じる。

DB の保存・HTTP の送信・公開済み OS 画面は VM の revert で取り消されない。revert の後に別の操作を始める場所は `external fresh` を明示する。キャンセルは server が受け取った request の取消しを保証しない。

SDK の検証では応答を保留する実 server を使い、実 Win32 / X11 ウィンドウへ pointer input を送り、HTTP が保留中でも Cancel が処理されることを確認する。その後 server と DB を除いて、ソースのない artifact の debug / compact 記録を replay する。これは表示処理の待ち合わせと境界を示す最小例であり、フォーム編集・retry・多画面アプリは後続の統合で拡充する。
