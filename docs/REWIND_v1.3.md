# REWIND v1.3.0

## 実装

- 標準ライブラリ `std.gui` を追加。Windows Win32/GDI と Linux X11 のネイティブウィンドウにラベル、ボタン、チェックボックス、矩形を描画し、クリック・キー・resize・close を REWIND のイベントループで処理する。
- View と未公開 scene は通常の Checkpoint 対象。present / close はステージし、publish で反映する。確定済み scene は操作 ledger で重複公開を防ぐ。
- 入力イベントは観測 journal に記録。revert は cursor を戻し、continueInput は配信済みイベントだけを読み飛ばす。CLI の `--gui-events` fixture と replay で、表示環境なしでも同じ GUI ロジックを検証できる。
- `gui` effect と main-task 限定を検査。Linux の GUI 依存は初回公開時に動的ロードし、CLI プログラムに X11 を要求しない。
- `Map` 型の struct field に対するメソッド呼び出しの引数・戻り型を 1.3 で検査する。
- 実行直後、ユーザーコードより前に予約 Checkpoint `begin` を自動作成。`revert begin;` は変数・heap・未公開 I/O・通常の Checkpoint・タスクを初期化し、現在位置から継続する。`resume begin;` は先頭から再実行する。main task の active branch 外で revert を使用する。変数名等への begin の使用、commit begin、drop begin は拒否する。
- publish 済み結果・観測記録・消費した予算は初期化しない。通常の巻き戻しと同じ契約とする。
- SDK に GUI API 文書、counter / Undo サンプル、入力 fixture を同梱。Linux / Windows SDK の両方を継続配布する。

## 利用

[GUI guide](gui.md)、[言語リファレンス](language-reference.md)、[SDK 導入](getting-started.md) を参照。

```sh
rewind run main.rw --allow-effects gui
rewindc main.rw --allow-effects gui
rewind main.rwc --allow-effects gui
```

## 保証範囲

固定座標の単一 canvas と基本コントロールを対象とする。自動レイアウト、editable textbox、IME 編集、メニュー、file dialog、複数ウィンドウ、アクセシビリティ連携、macOS、native Wayland は未対応。Linux は 24-bit 以上の X11 display と core fonts を要求する。文字表示は OS / fonts に依存する。OS や X11 server の障害、すでに公開した表示や出力は巻き戻せない。

v1.3.0 は Release 公開後に main へ統合済み。今後も codex/develop で開発・配布し、公開した版を main に統合する。
